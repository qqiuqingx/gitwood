//! Read-only, revision-pinned blame data. The complete annotation stays in Rust and is paged.
use super::{invalid, os, session::Lease, valid_repo_path, RepoPath};
use crate::git::{CancellationToken, GitError, ReadCommand, ReadOperation, RunOptions};
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
    lines: std::sync::Mutex<Vec<Option<AnnotatedLine>>>,
    progress: tokio::sync::watch::Receiver<AnnotationProgress>,
    cancellation: CancellationToken,
}

#[derive(Clone, Default)]
struct AnnotationProgress {
    done: bool,
    error: Option<String>,
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

struct BlameStream {
    annotation: Arc<Annotation>,
    group: Option<Group>,
    progress: tokio::sync::watch::Sender<AnnotationProgress>,
}

impl BlameStream {
    fn line(&mut self, line: &[u8]) -> Result<(), GitError> {
        if line.is_empty() {
            return Ok(());
        }
        if let Some(group) = parse_header(line) {
            self.flush()?;
            self.group = Some(group);
            return Ok(());
        }
        let group = self
            .group
            .as_mut()
            .ok_or_else(|| invalid("annotate.invalidRecord"))?;
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
        Ok(())
    }

    fn flush(&mut self) -> Result<(), GitError> {
        let Some(group) = self.group.take() else {
            return Ok(());
        };
        let begin = group
            .final_line
            .checked_sub(1)
            .ok_or_else(|| invalid("annotate.invalidRecord"))?;
        if group.count == 0
            || begin
                .checked_add(group.count)
                .is_none_or(|end| end > self.annotation.source.len())
        {
            return Err(invalid("annotate.invalidRecord"));
        }
        let mut lines = self.annotation.lines.lock().expect("annotation lines lock");
        let mut prototype = AnnotatedLine {
            line: begin,
            short_hash: group.hash.chars().take(8).collect(),
            hash: group.hash,
            author: group.author,
            author_time: group.time,
            text: self.annotation.source[begin].clone(),
            previous: group.previous,
            previous_path: group.previous_path.as_deref().map(RepoPath::new),
        };
        attach_previous_revision(&mut prototype, &self.annotation.history);
        for index in begin..begin + group.count {
            if lines[index].is_some() {
                return Err(invalid("annotate.invalidRecord"));
            }
            let mut line = prototype.clone();
            line.line = index;
            line.text.clone_from(&self.annotation.source[index]);
            lines[index] = Some(line);
        }
        drop(lines);
        self.progress.send_modify(|_| {});
        Ok(())
    }

    fn finish(mut self) -> Result<(), GitError> {
        self.flush()?;
        if self
            .annotation
            .lines
            .lock()
            .expect("annotation lines lock")
            .iter()
            .any(Option::is_none)
        {
            return Err(invalid("annotate.incompleteOutput"));
        }
        Ok(())
    }
}

fn spawn_blame(
    lease: &Lease,
    annotation: Arc<Annotation>,
    options: RunOptions,
    progress: tokio::sync::watch::Sender<AnnotationProgress>,
) {
    let runner = lease.runner.clone();
    tokio::spawn(async move {
        let args = [
            "--porcelain",
            "--incremental",
            annotation.revision.as_str(),
            "--",
        ]
        .into_iter()
        .map(Into::into)
        .chain([os(annotation.path.bytes.clone())])
        .collect::<Vec<_>>();
        let mut stream = BlameStream {
            annotation,
            group: None,
            progress: progress.clone(),
        };
        let result = runner
            .run_stream(
                ReadCommand::new(ReadOperation::Blame).args(args),
                &options,
                |line| stream.line(line),
            )
            .await
            .and_then(|_| stream.finish());
        progress.send_modify(|state| {
            state.done = true;
            state.error = result.err().map(|error| error.to_string());
        });
    });
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
        options
            .env
            .insert("GIT_LITERAL_PATHSPECS".into(), "1".into());
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
        let (progress_tx, progress_rx) = tokio::sync::watch::channel(AnnotationProgress::default());
        let annotation = Arc::new(Annotation {
            id,
            revision,
            path: path.clone(),
            source,
            history,
            lines: std::sync::Mutex::new(vec![None; summary.total]),
            progress: progress_rx,
            cancellation: options.cancellation.clone(),
        });
        let mut data = self.data.lock().await;
        if options.cancellation.is_cancelled() {
            return Err(GitError::Cancelled);
        }
        data.annotation = Some(annotation.clone());
        drop(data);
        spawn_blame(self, annotation, options, progress_tx);
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
        let value = {
            let data = self.data.lock().await;
            data.annotation
                .as_ref()
                .filter(|value| value.id == id)
                .cloned()
                .ok_or_else(|| invalid("annotate.stale"))?
        };
        if end > value.source.len() {
            return Err(invalid("annotate.invalidRange"));
        }
        let mut progress = value.progress.clone();
        loop {
            let ready = value.lines.lock().expect("annotation lines lock")[start..end]
                .iter()
                .all(Option::is_some);
            if ready {
                break;
            }
            let state = progress.borrow().clone();
            if state.done {
                if value.cancellation.is_cancelled() {
                    return Err(GitError::Cancelled);
                }
                if let Some(error) = state.error {
                    return Err(invalid(&error));
                }
                return Err(invalid("annotate.incompleteOutput"));
            }
            progress
                .changed()
                .await
                .map_err(|_| invalid("annotate.incompleteOutput"))?;
        }
        let rows = value.lines.lock().expect("annotation lines lock");
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
        let mut data = self.data.lock().await;
        if let Some(token) = data.annotation_cancel.take() {
            token.cancel();
        }
        data.annotation = None;
    }
}
