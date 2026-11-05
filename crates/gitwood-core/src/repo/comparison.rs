//! Minimal reusable two-revision / revision-to-worktree comparison. Cache stays in Rust.
use super::{
    file_tree::{FileTree, FileTreePage},
    invalid, os,
    preview::Preview,
    session::Lease,
    RepoPath,
};
use crate::{
    diff::ContentSource,
    git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions},
    history::details::{self, Commit, FileChange},
    ops,
};
use serde::Serialize;
use std::{collections::HashSet, path::PathBuf, sync::Arc};
pub(super) struct Comparison {
    id: String,
    left: String,
    right: Option<String>,
    fingerprint: String,
    files: Vec<FileChange>,
    tree: FileTree,
    commits: Vec<(String, bool)>,
}
#[derive(Serialize)]
pub struct CommitPage {
    pub commits: Vec<Commit>,
    pub current: Vec<bool>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub left: String,
    pub right: Option<String>,
    pub commits: usize,
}
impl Comparison {
    pub(super) fn summary(&self) -> Summary {
        Summary {
            id: self.id.clone(),
            left: self.left.clone(),
            right: self.right.clone(),
            commits: self.commits.len(),
        }
    }
}
async fn read(
    r: &GitRunner,
    o: &RunOptions,
    op: ReadOperation,
    args: Vec<String>,
) -> Result<Vec<u8>, GitError> {
    Ok(r.run(ReadCommand::new(op).args(args), o).await?.stdout)
}
pub(super) async fn open(
    r: &GitRunner,
    o: &RunOptions,
    selected: &str,
    working: bool,
    right_selected: Option<&str>,
) -> Result<Comparison, GitError> {
    let before = ops::snapshot(r, o).await?.fingerprint;
    let left = ops::integrate::pin(r, o, selected).await?;
    let right = if working {
        None
    } else {
        Some(ops::integrate::pin(r, o, right_selected.unwrap_or("HEAD")).await?)
    };
    let mut args = vec![
        "--name-status".into(),
        "-z".into(),
        "-M".into(),
        left.clone(),
    ];
    args.extend(right.clone());
    args.push("--".into());
    let files = parse_files(&read(r, o, ReadOperation::Diff, args).await?)?;
    let mut commits = vec![];
    if let Some(right) = &right {
        let range = format!("{left}...{right}");
        commits = String::from_utf8_lossy(
            &read(
                r,
                o,
                ReadOperation::Log,
                vec![
                    "--format=%m %H".into(),
                    "--left-right".into(),
                    "--topo-order".into(),
                    range,
                    "--".into(),
                ],
            )
            .await?,
        )
        .lines()
        .filter_map(|line| {
            line.split_once(' ')
                .map(|(side, hash)| (hash.to_owned(), side == ">"))
        })
        .collect();
    }
    if ops::snapshot(r, o).await?.fingerprint != before {
        return Err(invalid("remote.concurrentChange"));
    }
    Ok(Comparison {
        id: format!("{left}:{}:{before}", right.as_deref().unwrap_or("worktree")),
        tree: FileTree::new(&files),
        left,
        right,
        fingerprint: before,
        files,
        commits,
    })
}
fn parse_files(bytes: &[u8]) -> Result<Vec<FileChange>, GitError> {
    let mut fields = bytes.split(|b| *b == 0);
    let mut files = vec![];
    while let Some(status) = fields.next().filter(|s| !s.is_empty()) {
        let first = fields.next().ok_or_else(|| invalid("missing path"))?;
        let (path, original) = if matches!(status[0], b'R' | b'C') {
            (
                fields.next().ok_or_else(|| invalid("missing rename"))?,
                Some(RepoPath::new(first)),
            )
        } else {
            (first, None)
        };
        files.push(FileChange {
            status: super::text(status),
            path: RepoPath::new(path),
            original,
        });
    }
    Ok(files)
}
impl Lease {
    async fn comparison(&self, id: &str) -> Result<Arc<Comparison>, GitError> {
        self.ensure_current()?;
        self.data
            .lock()
            .await
            .comparison
            .clone()
            .filter(|c| c.id == id)
            .ok_or_else(|| invalid("comparison.stale"))
    }
    pub async fn comparison_files(
        &self,
        id: &str,
        expanded: &HashSet<String>,
        start: usize,
        end: usize,
    ) -> Result<FileTreePage, GitError> {
        if end < start || end - start > 200 {
            return Err(invalid("page exceeds 200 files"));
        }
        Ok(self
            .comparison(id)
            .await?
            .tree
            .page(expanded, start, end, &[]))
    }
    pub async fn comparison_commits(
        &self,
        id: &str,
        start: usize,
        end: usize,
    ) -> Result<CommitPage, GitError> {
        if end < start || end - start > 200 {
            return Err(invalid("page exceeds 200 commits"));
        }
        let c = self.comparison(id).await?;
        let hashes = c
            .commits
            .get(start..end.min(c.commits.len()))
            .ok_or_else(|| invalid("comparison.invalidRange"))?;
        let current = hashes.iter().map(|(_, current)| *current).collect();
        let hashes: Vec<_> = hashes.iter().map(|(hash, _)| hash.clone()).collect();
        Ok(CommitPage {
            commits: details::page(&self.runner, &self.options, &hashes, false).await?,
            current,
        })
    }
    pub async fn comparison_diff(&self, id: &str, path: &RepoPath) -> Result<Preview, GitError> {
        let c = self.comparison(id).await?;
        if c.right.is_none()
            && ops::snapshot(&self.runner, &self.options)
                .await?
                .fingerprint
                != c.fingerprint
        {
            return Err(invalid("remote.concurrentChange"));
        }
        let file = c
            .files
            .iter()
            .find(|f| f.path.bytes == path.bytes)
            .ok_or_else(|| invalid("comparison.invalidPath"))?;
        let path = PathBuf::from(os(file.path.bytes.clone()));
        let original = PathBuf::from(os(file
            .original
            .as_ref()
            .unwrap_or(&file.path)
            .bytes
            .clone()));
        let left = (!file.status.starts_with('A')).then_some(ContentSource::Commit {
            revision: &c.left,
            path: &original,
        });
        let right = (!file.status.starts_with('D')).then_some(match c.right.as_deref() {
            Some(revision) => ContentSource::Commit {
                revision,
                path: &path,
            },
            None => ContentSource::Worktree { path: &path },
        });
        let result = self
            .preview(
                left,
                right,
                &c.left,
                c.right.as_deref().unwrap_or("Working tree"),
            )
            .await?;
        if c.right.is_none()
            && ops::snapshot(&self.runner, &self.options)
                .await?
                .fingerprint
                != c.fingerprint
        {
            return Err(invalid("remote.concurrentChange"));
        }
        Ok(result)
    }
}
