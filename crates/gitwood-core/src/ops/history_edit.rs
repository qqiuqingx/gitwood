//! Preconditions shared by actions that edit the current branch history (Undo Commit, and in
//! v0.2 Revert, Reset, Cherry-pick): working-tree root, no index lock, the expected HEAD, and
//! no merge / rebase / cherry-pick / revert in progress. Each action adds its own commit checks.
use super::{context::Context, index::IndexTransaction, CommitError};
use crate::{
    git::{GitRunner, RunOptions},
    repo::operation::{self, Operation},
};

pub async fn operation_state(
    runner: &GitRunner,
    options: &RunOptions,
) -> Result<Option<Operation>, CommitError> {
    let context = Context {
        runner,
        options: options.clone(),
    };
    current(&context).await
}

async fn current(context: &Context<'_>) -> Result<Option<Operation>, CommitError> {
    Ok(operation::read(&context.git_dir().await?)?)
}

/// Rejects when another operation is in progress (bisect excepted).
pub(super) async fn require_no_operation(context: &Context<'_>) -> Result<(), CommitError> {
    match current(context).await? {
        Some(state) if state.blocks_history_edit() => Err(CommitError::InvalidSelection(
            "cannot edit history during rebase, merge, cherry-pick or revert".into(),
        )),
        _ => Ok(()),
    }
}

/// Holds the index lock for the caller and verifies HEAD and operation state.
/// `expected` is the HEAD the user saw; a different HEAD is a concurrent change.
pub(super) async fn require_editable(
    context: &Context<'_>,
    expected: &str,
) -> Result<IndexTransaction, CommitError> {
    crate::history::details::validate_hash(expected)?;
    context.require_root().await?;
    let tx = IndexTransaction::acquire(context.git_path("index").await?)?;
    if context.head().await?.as_deref() != Some(expected) {
        return Err(CommitError::ConcurrentChange);
    }
    require_no_operation(context).await?;
    Ok(tx)
}
