//! Session APIs for stash and file-scoped workspace actions.
use super::session::Lease;
use crate::{
    git::GitError,
    ops::{self, CommitError},
    repo::RepoPath,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashPage {
    pub entries: Vec<ops::stash::StashEntry>,
    pub start: usize,
    pub total: usize,
    pub list_token: String,
}

impl Lease {
    pub async fn stash_page(&self, start: usize, end: usize) -> Result<StashPage, GitError> {
        let (entries, total, list_token) =
            ops::stash::page(&self.runner, &self.options, start, end).await?;
        self.ensure_current()?;
        Ok(StashPage {
            entries,
            start,
            total,
            list_token,
        })
    }

    pub async fn stash_diff(&self, entry: &ops::stash::StashEntry) -> Result<Vec<u8>, GitError> {
        let diff = ops::stash::diff(&self.runner, &self.options, entry).await?;
        self.ensure_current()?;
        Ok(diff)
    }

    pub async fn stash_write(
        &self,
        expected: &str,
        action: &ops::stash::StashAction,
    ) -> Result<ops::stash::StashOutcome, CommitError> {
        let guard = self.writes.start(&self.options)?;
        ops::stash::execute(&self.runner, &guard.options, expected, action).await
    }

    pub async fn stash_create(
        &self,
        expected: &str,
        message: &str,
        keep_index: bool,
        include_untracked: bool,
    ) -> Result<(), CommitError> {
        let guard = self.writes.start(&self.options)?;
        ops::stash::create(
            &self.runner,
            &guard.options,
            expected,
            message,
            keep_index,
            include_untracked,
        )
        .await
    }

    pub async fn changes_add(&self, paths: &[RepoPath], expected: &str) -> Result<(), CommitError> {
        let guard = self.writes.start(&self.options)?;
        ops::changes::add(&self.runner, &guard.options, paths, expected).await
    }

    pub async fn changes_delete(
        &self,
        paths: &[RepoPath],
        expected: &str,
    ) -> Result<(), CommitError> {
        let guard = self.writes.start(&self.options)?;
        ops::changes::delete(&self.runner, &guard.options, paths, expected).await
    }
}
