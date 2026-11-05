//! Conflict IPC domain boundary; no raw filesystem access is exposed to the UI.
use super::{session::Lease, RepoPath};
use crate::{
    git::GitError,
    ops::{
        conflict::{self, MergeContent, Side},
        CommitError,
    },
};
impl Lease {
    pub async fn conflict_content(&self, path: &RepoPath) -> Result<MergeContent, GitError> {
        conflict::read(&self.runner, &self.options, path).await
    }
    pub async fn resolve_conflict(
        &self,
        path: &RepoPath,
        expected: &str,
        side: Option<Side>,
        text: Option<&str>,
    ) -> Result<(), CommitError> {
        let guard = self.writes.start(&self.options)?;
        conflict::resolve(&self.runner, &guard.options, path, expected, side, text).await
    }
    pub async fn continue_update(
        &self,
        expected: &str,
        stash: Option<&str>,
        action: crate::ops::sequence::SequenceAction,
        message: Option<&str>,
    ) -> Result<crate::ops::remote::RemoteOutcome, crate::ops::remote::RemoteError> {
        let guard = self
            .writes
            .start(&self.options)
            .map_err(|e| super::invalid(&e.to_string()))?;
        crate::ops::remote::continue_update(
            &self.runner,
            &guard.options,
            expected,
            stash,
            action,
            message,
        )
        .await
    }
}
