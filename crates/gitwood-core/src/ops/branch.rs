//! Explicit branch writes, sharing the commit/rollback write gate.
use crate::{
    git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions, WriteCommand},
    repo::{refs, status, RepoPath, Repository},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BranchOperation {
    Create {
        name: String,
        start: Option<String>,
        checkout: bool,
    },
    Checkout {
        name: String,
        mode: CheckoutMode,
    },
    CheckoutRemote {
        remote: String,
        branch: String,
        mode: CheckoutMode,
    },
    Rename {
        name: String,
        new_name: String,
    },
    Delete {
        name: String,
        force: bool,
    },
    SetUpstream {
        name: String,
        upstream: Option<String>,
    },
}
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckoutMode {
    #[default]
    Normal,
    Smart,
    Force,
}
#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BranchOutcome {
    Completed,
    LocalChanges {
        files: Vec<RepoPath>,
    },
    NotMerged {
        name: String,
    },
    StashRecovery {
        stash: String,
        files: Vec<RepoPath>,
        detail: String,
    },
}

async fn command(
    r: &GitRunner,
    o: &RunOptions,
    name: &str,
    args: &[&str],
) -> Result<Vec<u8>, GitError> {
    Ok(r.run(WriteCommand::new(name).args(args.iter().copied()), o)
        .await?
        .stdout)
}
pub(super) async fn validate(r: &GitRunner, o: &RunOptions, name: &str) -> Result<(), GitError> {
    // Reject revision shorthand as well as options; check-ref-format expands @{-n}.
    if name.starts_with('-') || name.starts_with('@') || name.is_empty() {
        return Err(crate::repo::invalid("branch.invalidName"));
    }
    command(r, o, "check-ref-format", &["--branch", name]).await?;
    Ok(())
}
async fn check_lock(r: &GitRunner, o: &RunOptions) -> Result<(), GitError> {
    let repository = Repository::open(r, o).await?;
    if repository.git_dir.join("index.lock").exists() {
        return Err(crate::repo::invalid("branch.indexLocked"));
    }
    Ok(())
}
async fn stash_id(r: &GitRunner, o: &RunOptions) -> Result<Option<String>, GitError> {
    match r
        .run(
            ReadCommand::new(ReadOperation::RevParse).args(["--verify", "-q", "refs/stash"]),
            o,
        )
        .await
    {
        Ok(out) => Ok(Some(String::from_utf8_lossy(&out.stdout).trim().into())),
        Err(GitError::Exit { code: Some(1), .. }) => Ok(None),
        Err(e) => Err(e),
    }
}
/// A retained stash is always named by immutable object ID. Never drop another
/// tool's stash if it has inserted one while the operation was running.
async fn restore(r: &GitRunner, o: &RunOptions, stash: &str) -> Result<BranchOutcome, GitError> {
    let result = if stash_position(r, o, stash).await?.is_some() {
        command(r, o, "stash", &["apply", stash]).await
    } else {
        Err(crate::repo::invalid("branch.stashChanged"))
    };
    if let Err(e) = result {
        let files = status::read(r, o)
            .await?
            .conflicts
            .into_iter()
            .map(|e| e.path)
            .collect();
        return Ok(BranchOutcome::StashRecovery {
            stash: stash.into(),
            files,
            detail: e.to_string(),
        });
    }
    drop_saved_stash(r, o, stash).await?;
    Ok(BranchOutcome::Completed)
}

/// Shared Normal / Smart / Force checkout path for branches and detached revisions.
pub(super) async fn checkout_with_mode(
    r: &GitRunner,
    o: &RunOptions,
    mut args: Vec<String>,
    mode: CheckoutMode,
    branch_ref: bool,
) -> Result<BranchOutcome, GitError> {
    check_lock(r, o).await?;
    let mut stash = None;
    if matches!(mode, CheckoutMode::Force) {
        args.insert(0, "--force".into());
    }
    if matches!(mode, CheckoutMode::Smart) {
        let before = stash_id(r, o).await?;
        let saved = command(r, o, "stash", &["push", "-m", "Gitwood Smart Checkout"]).await;
        let mut recovery = o.clone();
        recovery.cancellation = crate::git::CancellationToken::new();
        stash = stash_id(r, &recovery)
            .await?
            .filter(|id| Some(id) != before.as_ref());
        if let Err(error) = saved {
            if let Some(stash) = stash {
                return Ok(BranchOutcome::StashRecovery {
                    stash,
                    files: vec![],
                    detail: error.to_string(),
                });
            }
            return Err(error);
        }
    }
    if branch_ref {
        args.push("--".into());
    }
    let result = r.run(WriteCommand::new("checkout").args(args), o).await;
    if let Some(stash) = stash {
        let mut recovery = o.clone();
        recovery.cancellation = crate::git::CancellationToken::new();
        let restored = restore(r, &recovery, &stash).await?;
        if !matches!(restored, BranchOutcome::Completed) {
            return Ok(restored);
        }
    }
    check_lock(r, o).await?;
    match result {
        Ok(_) => Ok(BranchOutcome::Completed),
        Err(GitError::Exit { stderr, .. })
            if stderr.contains("would be overwritten by checkout") =>
        {
            let state = status::read(r, o).await?;
            let files = state
                .staged
                .into_iter()
                .chain(state.unstaged)
                .chain(state.untracked)
                .filter(|entry| {
                    stderr
                        .lines()
                        .filter_map(|line| line.strip_prefix('\t'))
                        .any(|line| unquote(line) == entry.path.bytes)
                })
                .map(|entry| entry.path)
                .collect();
            Ok(BranchOutcome::LocalChanges { files })
        }
        Err(error) => Err(error),
    }
}

pub async fn branch(
    r: &GitRunner,
    o: &RunOptions,
    operation: &BranchOperation,
    expected: &str,
) -> Result<BranchOutcome, GitError> {
    let _gate = super::WRITES.lock().await;
    check_lock(r, o).await?;
    if super::snapshot(r, o).await?.fingerprint != expected {
        return Err(crate::repo::invalid("branch.concurrentChange"));
    }
    let references = refs::read(r, o).await?;
    let local = |name: &str| {
        references
            .local
            .iter()
            .any(|b| b.name == format!("refs/heads/{name}"))
    };
    let mut args: Vec<String> = vec![];
    let mut mode = CheckoutMode::Normal;
    let name = match operation {
        BranchOperation::Create {
            name,
            start,
            checkout,
        } => {
            validate(r, o, name).await?;
            let start = start.as_deref().unwrap_or("HEAD");
            let pinned = r
                .run(
                    ReadCommand::new(ReadOperation::RevParse).args([
                        "--verify",
                        "--end-of-options",
                        &format!("{start}^{{commit}}"),
                    ]),
                    o,
                )
                .await?;
            let pinned = String::from_utf8_lossy(&pinned.stdout).trim().to_owned();
            if *checkout {
                args.extend(["-b".into(), name.clone(), pinned]);
                "checkout"
            } else {
                args.extend([name.clone(), pinned]);
                "branch"
            }
        }
        BranchOperation::Checkout {
            name,
            mode: selected,
        } => {
            validate(r, o, name).await?;
            if !local(name) {
                return Err(crate::repo::invalid("branch.localRequired"));
            }
            mode = *selected;
            args.push(name.clone());
            "checkout"
        }
        BranchOperation::CheckoutRemote {
            remote,
            branch,
            mode: selected,
        } => {
            validate(r, o, branch).await?;
            let target = format!("refs/remotes/{remote}/{branch}");
            if !references
                .remotes
                .values()
                .flatten()
                .any(|r| r.name == target)
            {
                return Err(crate::repo::invalid("branch.remoteRequired"));
            }
            mode = *selected;
            args.extend(["--track".into(), "-b".into(), branch.clone(), target]);
            "checkout"
        }
        BranchOperation::Rename { name, new_name } => {
            validate(r, o, name).await?;
            validate(r, o, new_name).await?;
            args.extend(["-m".into(), name.clone(), new_name.clone()]);
            "branch"
        }
        BranchOperation::Delete { name, force } => {
            validate(r, o, name).await?;
            args.extend([if *force { "-D" } else { "-d" }.into(), name.clone()]);
            "branch"
        }
        BranchOperation::SetUpstream { name, upstream } => {
            validate(r, o, name).await?;
            if let Some(upstream) = upstream {
                if !references
                    .local
                    .iter()
                    .chain(references.remotes.values().flatten())
                    .any(|b| b.name == *upstream)
                {
                    return Err(crate::repo::invalid("branch.upstreamRequired"));
                }
                args.extend(["--set-upstream-to".into(), upstream.clone(), name.clone()]);
            } else {
                args.extend(["--unset-upstream".into(), name.clone()]);
            }
            "branch"
        }
    };
    check_lock(r, o).await?;
    if super::snapshot(r, o).await?.fingerprint != expected {
        return Err(crate::repo::invalid("branch.concurrentChange"));
    }
    if name == "checkout" {
        return checkout_with_mode(r, o, args, mode, true).await;
    }
    let result = command(
        r,
        o,
        name,
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
    .await;
    check_lock(r, o).await?;
    match result {
        Ok(_) => Ok(BranchOutcome::Completed),
        Err(GitError::Exit { stderr, .. }) if stderr.contains("not fully merged") => {
            let BranchOperation::Delete { name, .. } = operation else {
                unreachable!()
            };
            Ok(BranchOutcome::NotMerged { name: name.clone() })
        }
        Err(e) => Err(e),
    }
}

fn unquote(line: &str) -> Vec<u8> {
    let Some(inner) = line.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return line.as_bytes().to_vec();
    };
    let mut out = Vec::new();
    let mut bytes = inner.bytes().peekable();
    while let Some(byte) = bytes.next() {
        if byte != b'\\' {
            out.push(byte);
            continue;
        }
        let Some(escaped) = bytes.next() else {
            break;
        };
        if (b'0'..=b'7').contains(&escaped) {
            let mut value = u16::from(escaped - b'0');
            for _ in 0..2 {
                if let Some(next) = bytes.peek().copied().filter(|b| (b'0'..=b'7').contains(b)) {
                    bytes.next();
                    value = value * 8 + u16::from(next - b'0');
                } else {
                    break;
                }
            }
            out.push(value as u8);
        } else {
            out.push(match escaped {
                b'a' => 7,
                b'b' => 8,
                b't' => 9,
                b'n' => 10,
                b'v' => 11,
                b'f' => 12,
                b'r' => 13,
                other => other,
            });
        }
    }
    out
}

/// Resolve the current reflog position twice; never reuse an index across apply.
pub(super) async fn stash_position(
    r: &GitRunner,
    o: &RunOptions,
    stash: &str,
) -> Result<Option<usize>, GitError> {
    let list = command(r, o, "stash", &["list", "--format=%H"]).await?;
    Ok(String::from_utf8_lossy(&list)
        .lines()
        .position(|id| id == stash))
}
pub(super) async fn drop_saved_stash(
    r: &GitRunner,
    o: &RunOptions,
    stash: &str,
) -> Result<(), GitError> {
    if let Some(index) = stash_position(r, o, stash).await? {
        if stash_position(r, o, stash).await? == Some(index) {
            command(r, o, "stash", &["drop", &format!("stash@{{{index}}}")]).await?;
        }
    }
    Ok(())
}
