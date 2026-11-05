//! Read-only, revision-pinned blame data. The complete annotation stays in Rust and is paged.
use super::{invalid, os, session::Lease, valid_repo_path, RepoPath};
use crate::git::{GitError, ReadCommand, ReadOperation};
use serde::Serialize;
use std::{
    ffi::OsString,
    hash::{Hash, Hasher},
    sync::Arc,
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotatedLine {
    pub line: usize,
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub author_time: i64,
    pub text: String,
    pub previous: Option<String>,
    pub previous_path: Option<RepoPath>,
}

pub(super) struct Annotation {
    pub id: String,
    revision: String,
    path: RepoPath,
    source: Vec<String>,
    history: Vec<super::file_history::FileRevision>,
    lines: tokio::sync::Mutex<Vec<Option<AnnotatedLine>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationSummary {
    pub id: String,
    pub revision: String,
    pub total: usize,
    pub oldest: i64,
    pub newest: i64,
}

#[derive(Serialize)]
pub struct AnnotationPage {
    pub total: usize,
    pub start: usize,
    pub lines: Vec<AnnotatedLine>,
}

#[derive(Default)]
struct Group {
    hash: String,
    final_line: usize,
    count: usize,
    author: String,
    time: i64,
    previous: Option<String>,
    previous_path: Option<Vec<u8>>,
}

fn parse_incremental(bytes: &[u8]) -> Result<Vec<Group>, GitError> {
    let mut groups = Vec::<Group>::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        if let Some(group) = parse_header(line) {
            groups.push(group);
            continue;
        }
        let Some(group) = groups.last_mut() else {
            return Err(invalid("annotate.invalidRecord"));
        };
        if let Some(value) = line.strip_prefix(b"author ") {
            group.author = String::from_utf8_lossy(value).into_owned();
        } else if let Some(value) = line.strip_prefix(b"author-time ") {
            group.time = String::from_utf8_lossy(value)
                .parse()
                .map_err(|_| invalid("annotate.invalidRecord"))?;
        } else if let Some(value) = line.strip_prefix(b"previous ") {
            let mut fields = value.splitn(2, |byte| *byte == b' ');
            group.previous = fields
                .next()
                .map(|v| String::from_utf8_lossy(v).into_owned());
            group.previous_path = fields.next().map(ToOwned::to_owned);
        }
    }
    if groups.iter().any(|group| group.count == 0) {
        return Err(invalid("annotate.invalidRecord"));
    }
    Ok(groups)
}

fn parse_header(line: &[u8]) -> Option<Group> {
    let fields: Vec<_> = line.split(|byte| *byte == b' ').collect();
    if fields.len() != 4 || fields[0].len() < 40 || !fields[0].iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    Some(Group {
        hash: String::from_utf8_lossy(fields[0]).into_owned(),
        final_line: parse_number(fields[2]).ok()?,
        count: parse_number(fields[3]).ok()?,
        ..Group::default()
    })
}

fn parse_number(value: &[u8]) -> Result<usize, GitError> {
    String::from_utf8_lossy(value)
        .parse()
        .map_err(|_| invalid("annotate.invalidRecord"))
}

fn object_path(revision: &str, path: &[u8]) -> OsString {
    let mut bytes = revision.as_bytes().to_vec();
    bytes.push(b':');
    bytes.extend_from_slice(path);
    os(bytes)
}

fn content_lines(content: &[u8]) -> Result<Vec<String>, GitError> {
    let text = std::str::from_utf8(content).map_err(|_| invalid("annotate.binaryFile"))?;
    if content.contains(&0) {
        return Err(invalid("annotate.binaryFile"));
    }
    Ok(text
        .split_terminator('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
        .collect())
}

fn fill_lines(
    groups: Vec<Group>,
    source: &[String],
    start: usize,
    values: &mut [Option<AnnotatedLine>],
    history: &[super::file_history::FileRevision],
) -> Result<(), GitError> {
    for group in groups {
        let begin = group
            .final_line
            .checked_sub(1)
            .ok_or_else(|| invalid("annotate.invalidRecord"))?;
        for offset in 0..group.count {
            let index = begin + offset;
            let Some(slot) = index.checked_sub(start).and_then(|i| values.get_mut(i)) else {
                continue;
            };
            let mut line = AnnotatedLine {
                line: index,
                short_hash: group.hash.chars().take(8).collect(),
                hash: group.hash.clone(),
                author: group.author.clone(),
                author_time: group.time,
                text: source[index].clone(),
                previous: group.previous.clone(),
                previous_path: group.previous_path.as_deref().map(RepoPath::new),
            };
            attach_previous_revision(&mut line, history);
            *slot = Some(line);
        }
    }
    if values.iter().any(Option::is_none) {
        return Err(invalid("annotate.incompleteOutput"));
    }
    Ok(())
}

async fn fill_page(
    lease: &Lease,
    annotation: &Annotation,
    start: usize,
    end: usize,
    values: &mut [Option<AnnotatedLine>],
    options: &crate::git::RunOptions,
) -> Result<(), GitError> {
    let range = format!("-L{},{}", start + 1, end);
    let args = [
        "--porcelain",
        "--incremental",
        range.as_str(),
        annotation.revision.as_str(),
        "--",
    ]
    .into_iter()
    .map(Into::into)
    .chain([os(annotation.path.bytes.clone())])
    .collect::<Vec<OsString>>();
    let output = lease
        .runner
        .run(ReadCommand::new(ReadOperation::Blame).args(args), options)
        .await?;
    fill_lines(
        parse_incremental(&output.stdout)?,
        &annotation.source,
        start,
        values,
        &annotation.history,
    )
}

async fn history_rows(
    lease: &Lease,
    path: &RepoPath,
    revision: &str,
    options: &crate::git::RunOptions,
) -> Result<Vec<super::file_history::FileRevision>, GitError> {
    Ok(lease
        .file_history_load_with_options(path, revision, options)
        .await?
        .rows)
}

fn attach_previous_revision(
    line: &mut AnnotatedLine,
    history: &[super::file_history::FileRevision],
) {
    if let Some(index) = history.iter().position(|row| row.hash == line.hash) {
        if let Some(previous) = history.get(index + 1) {
            line.previous = Some(previous.hash.clone());
            line.previous_path = Some(previous.path.clone());
        }
    }
}

impl Lease {
    pub async fn annotate_open(
        &self,
        path: &RepoPath,
        target: &str,
    ) -> Result<AnnotationSummary, GitError> {
        self.ensure_current()?;
        if !valid_repo_path(path) || path.bytes.contains(&0) {
            return Err(invalid("annotate.invalidPath"));
        }
        let token = self.options.cancellation.child_token();
        {
            let mut data = self.data.lock().await;
            if let Some(previous) = data.annotation_cancel.replace(token.clone()) {
                previous.cancel();
            }
            data.annotation = None;
        }
        let mut options = self.options.clone();
        options.cancellation = token;
        let revision = crate::ops::integrate::pin(&self.runner, &options, target).await?;
        let history = history_rows(self, path, &revision, &options).await?;
        let content = self
            .runner
            .run(
                ReadCommand::new(ReadOperation::CatFile)
                    .args(["blob".into(), object_path(&revision, &path.bytes)]),
                &options,
            )
            .await?;
        let source = content_lines(&content.stdout)?;
        self.ensure_current()?;
        let oldest = history
            .iter()
            .map(|line| line.author_time)
            .min()
            .unwrap_or(0);
        let newest = history
            .iter()
            .map(|line| line.author_time)
            .max()
            .unwrap_or(0);
        let mut path_hash = std::collections::hash_map::DefaultHasher::new();
        path.bytes.hash(&mut path_hash);
        let id = format!("{}:{revision}:{:x}", self.generation, path_hash.finish());
        let summary = AnnotationSummary {
            id: id.clone(),
            revision: revision.clone(),
            total: source.len(),
            oldest,
            newest,
        };
        self.data.lock().await.annotation = Some(Arc::new(Annotation {
            id,
            revision,
            path: path.clone(),
            source,
            history,
            lines: tokio::sync::Mutex::new(vec![None; summary.total]),
        }));
        Ok(summary)
    }

    pub async fn annotate_page(
        &self,
        id: &str,
        start: usize,
        end: usize,
    ) -> Result<AnnotationPage, GitError> {
        if end < start || end - start > 200 {
            return Err(invalid("page exceeds 200 lines"));
        }
        let (value, token) = {
            let data = self.data.lock().await;
            let value = data
                .annotation
                .as_ref()
                .filter(|value| value.id == id)
                .cloned()
                .ok_or_else(|| invalid("annotate.stale"))?;
            let token = data
                .annotation_cancel
                .clone()
                .ok_or_else(|| invalid("annotate.stale"))?;
            (value, token)
        };
        if end > value.source.len() {
            return Err(invalid("annotate.invalidRange"));
        }
        let mut rows = value.lines.lock().await;
        if rows[start..end].iter().any(Option::is_none) {
            let mut options = self.options.clone();
            options.cancellation = token;
            options
                .env
                .insert("GIT_LITERAL_PATHSPECS".into(), "1".into());
            fill_page(self, &value, start, end, &mut rows[start..end], &options).await?;
        }
        let lines = rows[start..end]
            .iter()
            .cloned()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| invalid("annotate.incompleteOutput"))?;
        Ok(AnnotationPage {
            total: rows.len(),
            start,
            lines,
        })
    }

    pub async fn annotate_cancel(&self) {
        if let Some(token) = self.data.lock().await.annotation_cancel.take() {
            token.cancel();
        }
        self.data.lock().await.annotation = None;
    }
}
