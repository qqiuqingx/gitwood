//! Paged history for one path. Commit metadata and rename path transitions stay in Rust.
use super::{invalid, session::Lease, valid_repo_path, RepoPath};
use crate::git::{GitError, ReadCommand, ReadOperation, RunOptions};
use serde::Serialize;
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRevision {
    pub hash: String,
    pub parents: Vec<String>,
    pub author_time: i64,
    pub author: String,
    pub subject: String,
    pub path: RepoPath,
}

pub(super) struct FileHistory {
    pub id: String,
    pub rows: Vec<FileRevision>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistorySummary {
    pub id: String,
    pub total: usize,
}

#[derive(Serialize)]
pub struct FileHistoryPage {
    pub total: usize,
    pub start: usize,
    pub rows: Vec<FileRevision>,
}

fn update_path(statuses: &[u8], path: &mut Vec<u8>) -> Result<(), GitError> {
    let mut fields = statuses
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    let status = fields.next().unwrap_or_default();
    if status.starts_with(b"R") || status.starts_with(b"C") {
        let previous = fields
            .next()
            .ok_or_else(|| invalid("fileHistory.invalidRename"))?;
        fields
            .next()
            .ok_or_else(|| invalid("fileHistory.invalidRename"))?;
        *path = previous.to_vec();
    } else if status.starts_with(b"A") || status.starts_with(b"D") || status.starts_with(b"M") {
        let _ = fields.next();
    } else if !status.is_empty() {
        return Err(invalid("fileHistory.invalidStatus"));
    }
    Ok(())
}

fn split_record(record: &[u8], path: &mut Vec<u8>) -> Result<FileRevision, GitError> {
    let fields: Vec<_> = record.split(|byte| *byte == 0).collect();
    if fields.len() < 5 {
        return Err(invalid("fileHistory.invalidRecord"));
    }
    let hash = String::from_utf8_lossy(fields[0]).into_owned();
    if hash.len() < 40 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid("fileHistory.invalidHash"));
    }
    let metadata_end = record
        .windows(2)
        .position(|bytes| bytes == b"\n\n")
        .map(|index| index + 2)
        .or_else(|| {
            record
                .iter()
                .position(|byte| *byte == b'\n')
                .map(|index| index + 1)
        })
        .unwrap_or(record.len());
    let statuses = &record[metadata_end..];
    let row_path = path.clone();
    update_path(statuses, path)?;
    let parents = String::from_utf8_lossy(fields[1])
        .split_ascii_whitespace()
        .map(str::to_owned)
        .collect();
    let author_time = String::from_utf8_lossy(fields[2])
        .parse()
        .map_err(|_| invalid("fileHistory.invalidDate"))?;
    Ok(FileRevision {
        hash,
        parents,
        author_time,
        author: String::from_utf8_lossy(fields[3]).into_owned(),
        subject: String::from_utf8_lossy(fields[4])
            .trim_start_matches('\n')
            .to_owned(),
        path: RepoPath::new(&row_path),
    })
}

fn parse_log(bytes: &[u8], path: &[u8]) -> Result<Vec<FileRevision>, GitError> {
    let mut current_path = path.to_vec();
    bytes
        .split(|byte| *byte == 0x1e)
        .filter(|record| !record.is_empty())
        .map(|record| split_record(record, &mut current_path))
        .collect()
}

impl Lease {
    pub async fn file_history_open(
        &self,
        path: &RepoPath,
        target: &str,
    ) -> Result<FileHistorySummary, GitError> {
        self.file_history_open_with_options(path, target, &self.options)
            .await
    }

    pub(super) async fn file_history_open_with_options(
        &self,
        path: &RepoPath,
        target: &str,
        options: &RunOptions,
    ) -> Result<FileHistorySummary, GitError> {
        self.ensure_current()?;
        if !valid_repo_path(path) || path.bytes.contains(&0) {
            return Err(invalid("fileHistory.invalidPath"));
        }
        let mut options = options.clone();
        options
            .env
            .insert("GIT_LITERAL_PATHSPECS".into(), "1".into());
        let revision = crate::ops::integrate::pin(&self.runner, &options, target).await?;
        let path_arg = super::os(path.bytes.clone());
        let args = [
            "--follow",
            "-M",
            "--name-status",
            "-z",
            "--format=%x1e%H%x00%P%x00%at%x00%an%x00%s",
        ]
        .into_iter()
        .map(Into::into)
        .chain([revision.into(), "--".into(), path_arg])
        .collect::<Vec<std::ffi::OsString>>();
        let output = self
            .runner
            .run(ReadCommand::new(ReadOperation::Log).args(args), &options)
            .await?;
        let rows = parse_log(&output.stdout, &path.bytes)?;
        let mut path_hash = std::collections::hash_map::DefaultHasher::new();
        path.bytes.hash(&mut path_hash);
        let id = format!(
            "{}:{}:{:x}",
            self.generation,
            rows.first().map_or("empty", |row| row.hash.as_str()),
            path_hash.finish()
        );
        let summary = FileHistorySummary {
            id: id.clone(),
            total: rows.len(),
        };
        self.data.lock().await.file_history = Some(Arc::new(FileHistory { id, rows }));
        Ok(summary)
    }

    pub async fn file_history_page(
        &self,
        id: &str,
        start: usize,
        end: usize,
    ) -> Result<FileHistoryPage, GitError> {
        if end < start || end - start > 200 {
            return Err(invalid("page exceeds 200 revisions"));
        }
        let data = self.data.lock().await;
        let history = data
            .file_history
            .as_ref()
            .filter(|history| history.id == id)
            .ok_or_else(|| invalid("fileHistory.stale"))?;
        let rows = history
            .rows
            .get(start..end.min(history.rows.len()))
            .ok_or_else(|| invalid("fileHistory.invalidRange"))?
            .to_vec();
        Ok(FileHistoryPage {
            total: history.rows.len(),
            start,
            rows,
        })
    }
}
