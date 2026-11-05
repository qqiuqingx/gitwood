//! Read-only, paged browsing of one immutable commit tree. The complete index and the
//! selected revision stay in the Rust session; the UI receives only a visible page.
use super::{
    file_tree::{FileTree, FileTreePage},
    invalid,
    session::Lease,
    RepoPath,
};
use crate::{
    diff::ContentSource,
    git::{GitError, ReadCommand, ReadOperation},
    history::details::FileChange,
    ops,
};
use serde::Serialize;
use std::{collections::HashSet, path::PathBuf, sync::Arc};

pub(super) struct HistoryTree {
    pub id: String,
    pub revision: String,
    pub files: Vec<FileChange>,
    pub tree: FileTree,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryTreeSummary {
    pub id: String,
    pub revision: String,
    pub files: usize,
}

fn parse_tree(bytes: &[u8]) -> Result<Vec<FileChange>, GitError> {
    let mut files = Vec::new();
    for record in bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| invalid("history.invalidTreeRecord"))?;
        let (metadata, path) = (&record[..tab], &record[tab + 1..]);
        let fields: Vec<_> = metadata.split(|byte| *byte == b' ').collect();
        if fields.len() != 3 {
            return Err(invalid("history.invalidTreeRecord"));
        }
        if fields[1] == b"blob" || fields[0] == b"160000" {
            files.push(FileChange {
                status: if fields[0] == b"160000" { "S" } else { "M" }.into(),
                path: RepoPath::new(path),
                original: None,
            });
        }
    }
    Ok(files)
}

impl Lease {
    pub async fn history_tree_open(&self, revision: &str) -> Result<HistoryTreeSummary, GitError> {
        self.ensure_current()?;
        let pinned = ops::integrate::pin(&self.runner, &self.options, revision).await?;
        let out = self
            .runner
            .run(
                ReadCommand::new(ReadOperation::LsTree).args(["-r", "-z", "--full-tree", &pinned]),
                &self.options,
            )
            .await?;
        let files = parse_tree(&out.stdout)?;
        let tree = FileTree::new(&files);
        let id = format!("{pinned}:{}", self.generation);
        let summary = HistoryTreeSummary {
            id: id.clone(),
            revision: pinned.clone(),
            files: files.len(),
        };
        let mut data = self.data.lock().await;
        self.ensure_current()?;
        data.history_tree = Some(Arc::new(HistoryTree {
            id,
            revision: pinned,
            files,
            tree,
        }));
        Ok(summary)
    }
    pub async fn history_tree_files(
        &self,
        id: &str,
        expanded: &HashSet<String>,
        start: usize,
        end: usize,
    ) -> Result<FileTreePage, GitError> {
        if end < start || end - start > 200 {
            return Err(invalid("page exceeds 200 files"));
        }
        let data = self.data.lock().await;
        let tree = data
            .history_tree
            .as_ref()
            .filter(|tree| tree.id == id)
            .ok_or_else(|| invalid("history.staleTree"))?;
        Ok(tree.tree.page(expanded, start, end, &[]))
    }
    pub async fn history_tree_diff(
        &self,
        id: &str,
        path: &RepoPath,
    ) -> Result<super::preview::Preview, GitError> {
        let data = self.data.lock().await;
        let tree = data
            .history_tree
            .as_ref()
            .filter(|tree| tree.id == id)
            .ok_or_else(|| invalid("history.staleTree"))?;
        let file = tree
            .files
            .iter()
            .find(|file| file.path.bytes == path.bytes)
            .ok_or_else(|| invalid("history.invalidPath"))?;
        let path = PathBuf::from(super::os(file.path.bytes.clone()));
        self.preview(
            None,
            Some(ContentSource::Commit {
                revision: &tree.revision,
                path: &path,
            }),
            "Empty",
            &tree.revision,
        )
        .await
    }
}
