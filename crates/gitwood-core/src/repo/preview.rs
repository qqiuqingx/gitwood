//! Current-file previews. Sources are resolved from Git state, never from display paths.
use super::{invalid, os, session::Lease, status, RepoPath};
use crate::{
    diff::*,
    git::{GitError, ReadCommand, ReadOperation},
    history::details,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Area {
    Staged,
    Unstaged,
    Untracked,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub left: Content,
    pub right: Content,
    pub result: Option<DiffResult>,
    pub left_label: String,
    pub right_label: String,
    pub conflicted: bool,
}
#[derive(Serialize)]
pub struct ContainingBranches {
    pub branches: Vec<String>,
    pub total: usize,
    pub protected: bool,
}
impl Lease {
    pub async fn containing_branches(&self, hash: &str) -> Result<ContainingBranches, GitError> {
        let names = containing_branch_names(&self.runner, &self.options, hash).await?;
        Ok(ContainingBranches {
            protected: names.iter().any(|name| protected_remote_branch(name)),
            total: names.len(),
            branches: names.into_iter().take(200).collect(),
        })
    }
    pub async fn worktree_diff(&self, path: &RepoPath, area: Area) -> Result<Preview, GitError> {
        let state = status::read(&self.runner, &self.options).await?;
        let empty = || classify(vec![], ReadLimits::default());
        if state.conflicts.iter().any(|e| e.path.bytes == path.bytes) {
            return Ok(Preview {
                left: empty(),
                right: empty(),
                result: None,
                left_label: "Index".into(),
                right_label: "Working tree".into(),
                conflicted: true,
            });
        }
        let entries = match area {
            Area::Staged => &state.staged,
            Area::Unstaged => &state.unstaged,
            Area::Untracked => &state.untracked,
        };
        let entry = entries
            .iter()
            .find(|e| e.path.bytes == path.bytes)
            .ok_or_else(|| invalid("file no longer in selected area"))?;
        let path = PathBuf::from(os(entry.path.bytes.clone()));
        let original = PathBuf::from(os(entry
            .original
            .as_ref()
            .unwrap_or(&entry.path)
            .bytes
            .clone()));
        let code = match area {
            Area::Staged => entry.xy.as_bytes()[0],
            Area::Unstaged => entry.xy.as_bytes()[1],
            Area::Untracked => b'A',
        };
        let left = match area {
            Area::Untracked => None,
            Area::Staged if code == b'A' => None,
            Area::Staged => Some(ContentSource::Commit {
                revision: "HEAD",
                path: &original,
            }),
            _ => Some(ContentSource::Index { path: &path }),
        };
        let right = if code == b'D' {
            None
        } else {
            Some(match area {
                Area::Staged => ContentSource::Index { path: &path },
                _ => ContentSource::Worktree { path: &path },
            })
        };
        self.preview(
            left,
            right,
            if matches!(area, Area::Staged) {
                "HEAD"
            } else {
                "Staged"
            },
            if matches!(area, Area::Staged) {
                "Staged"
            } else {
                "Working tree"
            },
        )
        .await
    }
    pub async fn commit_diff(&self, hash: &str, path: &RepoPath) -> Result<Preview, GitError> {
        details::validate_hash(hash)?;
        let files = self.files(hash).await?;
        let file = files
            .iter()
            .find(|f| f.path.bytes == path.bytes)
            .ok_or_else(|| invalid("file not in commit"))?;
        let commit = self.details(hash).await?;
        let path = PathBuf::from(os(file.path.bytes.clone()));
        let original = PathBuf::from(os(file
            .original
            .as_ref()
            .unwrap_or(&file.path)
            .bytes
            .clone()));
        let parent = commit.parents.first();
        let left = if file.status.starts_with('A') {
            None
        } else {
            parent.map(|revision| ContentSource::Commit {
                revision,
                path: &original,
            })
        };
        let right = if file.status.starts_with('D') {
            None
        } else {
            Some(ContentSource::Commit {
                revision: hash,
                path: &path,
            })
        };
        self.preview(left, right, parent.map_or("Empty", String::as_str), hash)
            .await
    }
    pub(super) async fn preview(
        &self,
        left: Option<ContentSource<'_>>,
        right: Option<ContentSource<'_>>,
        left_label: &str,
        right_label: &str,
    ) -> Result<Preview, GitError> {
        async fn read(l: &Lease, s: Option<ContentSource<'_>>) -> Result<Content, GitError> {
            match s {
                Some(s) => read_content(&l.runner, &l.options, s, ReadLimits::default())
                    .await
                    .map_err(|e| invalid(&e.to_string())),
                None => Ok(classify(vec![], ReadLimits::default())),
            }
        }
        let mut left = read(self, left).await?;
        let mut right = read(self, right).await?;
        let result = if let (Content::Text { text: a, .. }, Content::Text { text: b, .. }) =
            (&left, &right)
        {
            Some(compare(a, b, CompareOptions::default()))
        } else {
            None
        };
        // A non-text preview transfers no repository text from either side.
        if result.is_none() {
            if matches!(left, Content::Text { .. }) {
                left = classify(vec![], ReadLimits::default());
            }
            if matches!(right, Content::Text { .. }) {
                right = classify(vec![], ReadLimits::default());
            }
        }
        Ok(Preview {
            left,
            right,
            result,
            left_label: left_label.into(),
            right_label: right_label.into(),
            conflicted: false,
        })
    }
}

pub(crate) async fn containing_branch_names(
    runner: &crate::git::GitRunner,
    options: &crate::git::RunOptions,
    hash: &str,
) -> Result<Vec<String>, GitError> {
    details::validate_hash(hash)?;
    let output = runner
        .run(
            ReadCommand::new(ReadOperation::ForEachRef).args([
                "--contains",
                hash,
                "--format=%(refname)",
                "refs/heads",
                "refs/remotes",
            ]),
            options,
        )
        .await?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(String::from)
        .collect())
}

pub(crate) fn protected_remote_branch(name: &str) -> bool {
    name.strip_prefix("refs/remotes/")
        .and_then(|n| n.split_once('/'))
        .is_some_and(|(_, branch)| matches!(branch, "main" | "master"))
}
