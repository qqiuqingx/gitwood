//! Git stash operations. Stash references are always resolved against the list
//! displayed to the user before an index based operation is allowed to proceed.
use super::{snapshot, CommitError, WRITES};
use crate::{
    git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions, WriteCommand},
    repo::{self, status},
};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    pub selector: String,
    pub oid: String,
    pub subject: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StashOutcome {
    Completed,
    Conflicts { files: Vec<repo::RepoPath> },
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StashAction {
    Apply {
        entry: StashEntry,
        reinstate_index: bool,
    },
    Pop {
        entry: StashEntry,
        reinstate_index: bool,
    },
    Drop {
        entry: StashEntry,
    },
    Clear {
        list_token: String,
    },
    Branch {
        entry: StashEntry,
        name: String,
    },
}

async fn list(runner: &GitRunner, options: &RunOptions) -> Result<Vec<StashEntry>, GitError> {
    let output = runner
        .run(
            ReadCommand::new(ReadOperation::StashList).args(["list", "--format=%gd%x00%H%x00%gs"]),
            options,
        )
        .await?;
    let mut entries = Vec::new();
    for record in output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|r| !r.is_empty())
    {
        let mut fields = record.splitn(3, |byte| *byte == 0);
        let selector = fields.next().unwrap_or_default();
        let oid = fields.next().unwrap_or_default();
        let subject = fields.next().unwrap_or_default();
        if !selector.starts_with(b"stash@{") || !oid.iter().all(u8::is_ascii_hexdigit) {
            return Err(repo::invalid("stash.invalidList"));
        }
        entries.push(StashEntry {
            selector: String::from_utf8_lossy(selector).into_owned(),
            oid: String::from_utf8_lossy(oid).into_owned(),
            subject: String::from_utf8_lossy(subject).into_owned(),
        });
    }
    Ok(entries)
}

async fn check_index_lock(runner: &GitRunner, options: &RunOptions) -> Result<(), GitError> {
    let repository = repo::Repository::open(runner, options).await?;
    if repository.git_dir.join("index.lock").exists() {
        return Err(repo::invalid("stash.indexLocked"));
    }
    Ok(())
}

pub async fn page(
    runner: &GitRunner,
    options: &RunOptions,
    start: usize,
    end: usize,
) -> Result<(Vec<StashEntry>, usize, String), GitError> {
    if end < start || end - start > 200 {
        return Err(repo::invalid("stash.pageLimit"));
    }
    let entries = list(runner, options).await?;
    let total = entries.len();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    entries.hash(&mut hasher);
    let token = format!("{:016x}", hasher.finish());
    let page = entries
        .iter()
        .skip(start)
        .take(end - start)
        .cloned()
        .collect();
    Ok((page, total, token))
}

pub async fn diff(
    runner: &GitRunner,
    options: &RunOptions,
    entry: &StashEntry,
) -> Result<Vec<u8>, GitError> {
    let current = list(runner, options).await?;
    validate_entry(&current, entry)?;
    Ok(runner
        .run(
            ReadCommand::new(ReadOperation::StashShow).args([
                "show",
                "--patch",
                "--binary",
                "--include-untracked",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--ignore-submodules=all",
                entry.oid.as_str(),
            ]),
            options,
        )
        .await?
        .stdout)
}

fn validate_entry(entries: &[StashEntry], selected: &StashEntry) -> Result<(), GitError> {
    if !selected.oid.bytes().all(|b| b.is_ascii_hexdigit())
        || !selected.selector.starts_with("stash@{")
        || !selected.selector.ends_with('}')
    {
        return Err(repo::invalid("stash.invalidReference"));
    }
    if entries
        .iter()
        .any(|entry| entry.selector == selected.selector && entry.oid == selected.oid)
    {
        Ok(())
    } else {
        Err(repo::invalid("stash.changed"))
    }
}

pub async fn execute(
    runner: &GitRunner,
    options: &RunOptions,
    expected: &str,
    action: &StashAction,
) -> Result<StashOutcome, CommitError> {
    let _guard = tokio::select! { _ = options.cancellation.cancelled() => return Err(GitError::Cancelled.into()), guard = WRITES.lock() => guard };
    let before = snapshot(runner, options).await?;
    if before.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    let entries = list(runner, options).await?;
    match action {
        StashAction::Clear { list_token } => {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            entries.hash(&mut hasher);
            if *list_token != format!("{:016x}", hasher.finish()) {
                return Err(repo::invalid("stash.changed").into());
            }
            if entries.is_empty() {
                return Err(repo::invalid("stash.empty").into());
            }
            ensure_expected(runner, options, expected).await?;
            runner
                .run(WriteCommand::new("stash").args(["clear"]), options)
                .await?;
            Ok(StashOutcome::Completed)
        }
        StashAction::Drop { entry } => {
            validate_entry(&entries, entry)?;
            ensure_expected(runner, options, expected).await?;
            runner
                .run(
                    WriteCommand::new("stash").args(["drop", &entry.selector]),
                    options,
                )
                .await?;
            Ok(StashOutcome::Completed)
        }
        StashAction::Apply {
            entry,
            reinstate_index,
        }
        | StashAction::Pop {
            entry,
            reinstate_index,
        } => {
            check_index_lock(runner, options).await?;
            ensure_can_apply(runner, options).await?;
            validate_entry(&entries, entry)?;
            if before.fingerprint != expected {
                return Err(CommitError::ConcurrentChange);
            }
            ensure_expected(runner, options, expected).await?;
            let pop = matches!(action, StashAction::Pop { .. });
            let mut args = vec![if pop { "pop" } else { "apply" }];
            if *reinstate_index {
                args.push("--index");
            }
            args.push(&entry.selector);
            let result = runner
                .run(WriteCommand::new("stash").args(args), options)
                .await;
            match result {
                Ok(_) => Ok(StashOutcome::Completed),
                Err(error @ GitError::Exit { .. }) => {
                    let state = status::read(runner, options).await?;
                    if state.conflicts.is_empty() {
                        return Err(error.into());
                    }
                    let files = super::conflict::files(runner, options, &state.conflicts)
                        .await?
                        .into_iter()
                        .map(|file| file.path)
                        .collect();
                    Ok(StashOutcome::Conflicts { files })
                }
                Err(error) => Err(error.into()),
            }
        }
        StashAction::Branch { entry, name } => {
            check_index_lock(runner, options).await?;
            ensure_can_apply(runner, options).await?;
            validate_entry(&entries, entry)?;
            if name.starts_with('-') || name.trim().is_empty() {
                return Err(repo::invalid("stash.invalidBranch").into());
            }
            runner
                .run(
                    ReadCommand::new(ReadOperation::CheckRefFormat).args(["--branch", name]),
                    options,
                )
                .await?;
            if before.fingerprint != expected {
                return Err(CommitError::ConcurrentChange);
            }
            ensure_expected(runner, options, expected).await?;
            runner
                .run(
                    WriteCommand::new("stash").args(["branch", name, &entry.selector]),
                    options,
                )
                .await?;
            Ok(StashOutcome::Completed)
        }
    }
}

async fn ensure_can_apply(runner: &GitRunner, options: &RunOptions) -> Result<(), CommitError> {
    let state = status::read(runner, options).await?;
    if !state.conflicts.is_empty() || super::operation_state(runner, options).await?.is_some() {
        return Err(repo::invalid("stash.operationInProgress").into());
    }
    Ok(())
}

async fn ensure_expected(
    runner: &GitRunner,
    options: &RunOptions,
    expected: &str,
) -> Result<(), CommitError> {
    check_index_lock(runner, options).await?;
    if snapshot(runner, options).await?.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    Ok(())
}

pub async fn create(
    runner: &GitRunner,
    options: &RunOptions,
    expected: &str,
    message: &str,
    keep_index: bool,
    include_untracked: bool,
) -> Result<(), CommitError> {
    let _guard = tokio::select! { _ = options.cancellation.cancelled() => return Err(GitError::Cancelled.into()), guard = WRITES.lock() => guard };
    check_index_lock(runner, options).await?;
    let before = snapshot(runner, options).await?;
    if before.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    let state = status::read(runner, options).await?;
    if !state.conflicts.is_empty() || super::operation_state(runner, options).await?.is_some() {
        return Err(repo::invalid("stash.operationInProgress").into());
    }
    if state.staged.is_empty() && state.unstaged.is_empty() && state.untracked.is_empty() {
        return Err(repo::invalid("stash.noChanges").into());
    }
    let before_list = list(runner, options).await?;
    let mut args: Vec<OsString> = vec!["push".into()];
    if !message.trim().is_empty() {
        args.extend(["--message".into(), message.trim().into()]);
    }
    if keep_index {
        args.push("--keep-index".into());
    }
    if include_untracked {
        args.push("--include-untracked".into());
    }
    args.push("--".into());
    ensure_expected(runner, options, expected).await?;
    runner
        .run(WriteCommand::new("stash").args(args), options)
        .await?;
    if list(runner, options).await? == before_list {
        return Err(repo::invalid("stash.noChanges").into());
    }
    Ok(())
}
