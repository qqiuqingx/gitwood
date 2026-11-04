//! Continue / abort / skip the merge, rebase, am, cherry-pick or revert in progress.
//!
//! `--continue` never opens an editor: Git skips launching an editor named `:`, so the
//! prepared message (`MERGE_MSG`, or the one supplied here) is used as is, no shell is
//! started and no user configuration is changed. Hooks still run.
use super::remote::{redact, RemoteError};
use crate::{
    git::{CancellationToken, GitError, GitRunner, RunOptions, WriteCommand},
    repo::{
        self,
        operation::{self, Operation, OperationKind},
        RepoPath,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SequenceAction {
    Continue,
    Abort,
    Skip,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SequenceOutcome {
    /// The operation finished or was aborted; nothing is in progress any more.
    Completed,
    /// Stopped again on conflicts (for example the next commit of a rebase).
    Conflicts {
        operation: Operation,
        files: Vec<RepoPath>,
    },
    /// Still in progress without conflicts: an `edit` stop, an empty commit, or a hook
    /// failure. `detail` is Git's own output.
    Stopped {
        operation: Operation,
        detail: String,
    },
}

fn error(key: &str, detail: impl Into<String>) -> RemoteError {
    RemoteError {
        key: key.into(),
        detail: detail.into(),
        guidance: "sequence.inspectOutput",
    }
}

fn arguments(kind: OperationKind, action: SequenceAction) -> Option<(&'static str, &'static str)> {
    let flag = match action {
        SequenceAction::Continue => "--continue",
        SequenceAction::Abort => "--abort",
        SequenceAction::Skip => "--skip",
    };
    match (kind, action) {
        (OperationKind::Merge, SequenceAction::Skip) => None,
        (OperationKind::Merge, _) => Some(("merge", flag)),
        (OperationKind::Rebase, _) => Some(("rebase", flag)),
        (OperationKind::ApplyMailbox, _) => Some(("am", flag)),
        (OperationKind::CherryPick, _) => Some(("cherry-pick", flag)),
        (OperationKind::Revert, _) => Some(("revert", flag)),
        (OperationKind::Bisect, SequenceAction::Abort) => Some(("bisect", "reset")),
        (OperationKind::Bisect, _) => None,
    }
}

/// `expected` is the status fingerprint the user acted on. `message` replaces the prepared
/// commit message of a merge / cherry-pick / revert before continuing.
pub async fn run(
    r: &GitRunner,
    o: &RunOptions,
    expected: &str,
    action: SequenceAction,
    message: Option<&str>,
) -> Result<SequenceOutcome, RemoteError> {
    let _gate = tokio::select! {
        _ = o.cancellation.cancelled() => return Err(GitError::Cancelled.into()),
        guard = super::WRITES.lock() => guard,
    };
    super::remote::check(r, o, expected).await?;
    run_locked(r, o, action, message).await
}

/// Caller holds `WRITES` and has verified the expected snapshot.
pub(super) async fn run_locked(
    r: &GitRunner,
    o: &RunOptions,
    action: SequenceAction,
    message: Option<&str>,
) -> Result<SequenceOutcome, RemoteError> {
    let repository = repo::Repository::open(r, o).await?;
    let state = operation::read(&repository.git_dir)
        .map_err(|e| error("sequence.failed", e.to_string()))?
        .ok_or_else(|| error("sequence.none", ""))?;
    let (command, flag) =
        arguments(state.kind, action).ok_or_else(|| error("sequence.unsupported", ""))?;
    if action == SequenceAction::Continue {
        if !repo::status::read(r, o).await?.conflicts.is_empty() {
            return Err(error("sequence.unresolved", ""));
        }
        if let Some(message) = message {
            if !matches!(
                state.kind,
                OperationKind::Merge | OperationKind::CherryPick | OperationKind::Revert
            ) {
                return Err(error("sequence.messageUnsupported", ""));
            }
            std::fs::write(repository.git_dir.join("MERGE_MSG"), message)
                .map_err(|e| error("sequence.failed", e.to_string()))?;
        }
    } else if message.is_some() {
        return Err(error("sequence.messageUnsupported", ""));
    }
    let mut options = o.clone();
    options.env.insert("GIT_EDITOR".into(), ":".into());
    let result = r
        .run(WriteCommand::new(command).args([flag]), &options)
        .await;
    if let Err(GitError::Cancelled) = result {
        return Err(GitError::Cancelled.into());
    }
    // Report the state Git left behind even if the request was cancelled meanwhile.
    let mut recovery = o.clone();
    recovery.cancellation = CancellationToken::new();
    let after = operation::read(&repository.git_dir)
        .map_err(|e| error("sequence.failed", e.to_string()))?;
    let Some(operation) = after else {
        return match result {
            Ok(_) => Ok(SequenceOutcome::Completed),
            Err(e) => Err(failed(e)),
        };
    };
    let files: Vec<_> = repo::status::read(r, &recovery)
        .await?
        .conflicts
        .into_iter()
        .map(|e| e.path)
        .collect();
    if !files.is_empty() {
        return Ok(SequenceOutcome::Conflicts { operation, files });
    }
    match result {
        Ok(_) if action == SequenceAction::Abort => Err(error(
            "sequence.stillInProgress",
            "Git reported success but the operation is still in progress.",
        )),
        Ok(output) => Ok(SequenceOutcome::Stopped {
            operation,
            detail: redact(&output.stderr),
        }),
        Err(e) => Ok(SequenceOutcome::Stopped {
            operation,
            detail: redact(&e.to_string()),
        }),
    }
}

fn failed(e: GitError) -> RemoteError {
    match e {
        GitError::Cancelled | GitError::Timeout(_) => e.into(),
        e => error("sequence.failed", redact(&e.to_string())),
    }
}
