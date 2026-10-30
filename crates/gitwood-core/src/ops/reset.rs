//! Undo the current non-root, non-merge commit without touching index or worktree.
use super::{context::Context, index::IndexTransaction, snapshot, CommitError, WRITES};
use crate::{
    git::{GitError, GitRunner, ReadOperation, RunOptions, WriteCommand},
    repo::preview::{containing_branch_names, protected_remote_branch},
};

pub async fn undo_in_progress(
    runner: &GitRunner,
    options: &RunOptions,
) -> Result<bool, CommitError> {
    let context = Context {
        runner,
        options: options.clone(),
    };
    for name in [
        "rebase-merge",
        "rebase-apply",
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "sequencer",
    ] {
        if context.git_path(name).await?.try_exists()? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub async fn undo_commit(
    runner: &GitRunner,
    options: &RunOptions,
    expected: &str,
) -> Result<String, CommitError> {
    crate::history::details::validate_hash(expected)?;
    let _gate = tokio::select! { _ = options.cancellation.cancelled() => return Err(GitError::Cancelled.into()), guard = WRITES.lock() => guard };
    let context = Context {
        runner,
        options: options.clone(),
    };
    context.require_root().await?;
    let tx = IndexTransaction::acquire(context.git_path("index").await?)?;
    if context.head().await?.as_deref() != Some(expected) {
        return Err(CommitError::ConcurrentChange);
    }
    if undo_in_progress(runner, options).await? {
        return Err(CommitError::InvalidSelection(
            "cannot undo during rebase, merge, cherry-pick or revert".into(),
        ));
    }
    let parents = context
        .read(
            ReadOperation::Log,
            &["--format=%H %P", "-n", "1", expected, "--"],
        )
        .await?;
    let text = String::from_utf8_lossy(&parents);
    let ids: Vec<_> = text.split_whitespace().collect();
    if ids.len() != 2 {
        return Err(CommitError::InvalidSelection(
            "undo requires a non-root, non-merge commit".into(),
        ));
    }
    if containing_branch_names(runner, options, expected)
        .await?
        .iter()
        .any(|name| protected_remote_branch(name))
    {
        return Err(CommitError::InvalidSelection(
            "commit is pushed to a protected remote branch (main/master)".into(),
        ));
    }
    let message = context
        .read(ReadOperation::Log, &["-1", "--format=%B", expected, "--"])
        .await?;
    // --format adds one record terminator; preserve the complete message itself.
    let message =
        String::from_utf8_lossy(message.strip_suffix(b"\n").unwrap_or(&message)).into_owned();
    let before = snapshot(runner, options).await?;
    let worktree = super::snapshot::worktree_at(options, &before.paths)?;
    tx.check_unchanged()?;
    if context.head().await?.as_deref() != Some(expected)
        || undo_in_progress(runner, options).await?
        || snapshot(runner, options).await? != before
    {
        return Err(CommitError::ConcurrentChange);
    }
    runner
        .run(
            WriteCommand::new("reset").args(["--soft", ids[1], "--"]),
            options,
        )
        .await?;
    // Soft reset never rewrites index/worktree. Detect external index/ref changes.
    tx.check_unchanged()?;
    if context.head().await?.as_deref() != Some(ids[1])
        || super::snapshot::worktree_at(options, &before.paths)? != worktree
    {
        return Err(CommitError::ConcurrentChange);
    }
    Ok(message)
}
