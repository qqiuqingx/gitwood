//! History actions initiated from the log. Every mutation is serialized, pinned to the
//! snapshot shown by the UI, and uses Git arguments rather than a shell.
use super::{context::Context, history_edit, remote::redact, snapshot, CommitError, WRITES};
use crate::{
    git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions, WriteCommand},
    repo::{self, status, RepoPath, Repository},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
    Keep,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EmptyCherryPick {
    Skip,
    CreateEmpty,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HistoryWrite {
    CherryPick {
        commits: Vec<String>,
        record_origin: bool,
        empty: EmptyCherryPick,
    },
    Revert {
        commits: Vec<String>,
    },
    Reset {
        target: String,
        mode: ResetMode,
    },
    CheckoutRevision {
        target: String,
        mode: super::branch::CheckoutMode,
    },
    CreateTag {
        name: String,
        target: String,
        message: Option<String>,
    },
    DeleteTag {
        name: String,
    },
    DeleteRemoteTag {
        remote: String,
        name: String,
    },
    PushTag {
        remote: String,
        name: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum HistoryOutcome {
    Completed,
    Conflicts { files: Vec<RepoPath> },
    Stopped { detail: String },
    LocalChanges { files: Vec<RepoPath> },
    StashRecovery { stash: String, detail: String },
}

async fn write(
    r: &GitRunner,
    o: &RunOptions,
    name: &str,
    args: &[String],
) -> Result<Vec<u8>, GitError> {
    Ok(r.run(WriteCommand::new(name).args(args), o).await?.stdout)
}
async fn read(
    r: &GitRunner,
    o: &RunOptions,
    op: ReadOperation,
    args: &[String],
) -> Result<Vec<u8>, GitError> {
    Ok(r.run(ReadCommand::new(op).args(args), o).await?.stdout)
}
async fn pin(r: &GitRunner, o: &RunOptions, value: &str) -> Result<String, GitError> {
    crate::history::details::validate_hash(value)?;
    let out = read(
        r,
        o,
        ReadOperation::RevParse,
        &[
            "--verify".into(),
            "--end-of-options".into(),
            format!("{value}^{{commit}}"),
        ],
    )
    .await?;
    Ok(String::from_utf8_lossy(&out).trim().to_owned())
}
async fn parents(r: &GitRunner, o: &RunOptions, hash: &str) -> Result<Vec<String>, GitError> {
    let out = read(
        r,
        o,
        ReadOperation::Log,
        &[
            "--format=%P".into(),
            "-n".into(),
            "1".into(),
            hash.into(),
            "--".into(),
        ],
    )
    .await?;
    Ok(String::from_utf8_lossy(&out)
        .split_whitespace()
        .map(str::to_owned)
        .collect())
}
pub async fn selection_has_merge(
    r: &GitRunner,
    o: &RunOptions,
    hashes: &[String],
) -> Result<bool, GitError> {
    for hash in hashes {
        let pinned = pin(r, o, hash).await?;
        if parents(r, o, &pinned).await?.len() > 1 {
            return Ok(true);
        }
    }
    Ok(false)
}
async fn before_write(r: &GitRunner, o: &RunOptions, expected: &str) -> Result<(), CommitError> {
    let repository = Repository::open(r, o).await?;
    if repository.git_dir.join("index.lock").exists() {
        return Err(repo::invalid("history.indexLocked").into());
    }
    ensure_snapshot(r, o, expected).await?;
    let state = status::read(r, o).await?;
    let head = state
        .branch
        .oid
        .filter(|oid| oid != "(initial)")
        .ok_or_else(|| repo::invalid("history.noHead"))?;
    let context = Context {
        runner: r,
        options: o.clone(),
    };
    history_edit::check_editable(&context, &head).await?;
    Ok(())
}
async fn ensure_snapshot(r: &GitRunner, o: &RunOptions, expected: &str) -> Result<(), CommitError> {
    if snapshot(r, o).await?.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    Ok(())
}
async fn ensure_no_staged_changes(r: &GitRunner, o: &RunOptions) -> Result<(), CommitError> {
    if !status::read(r, o).await?.staged.is_empty() {
        return Err(repo::invalid("history.stagedChanges").into());
    }
    Ok(())
}
fn conflict_result(files: Vec<RepoPath>) -> HistoryOutcome {
    HistoryOutcome::Conflicts { files }
}
async fn current_conflicts(r: &GitRunner, o: &RunOptions) -> Result<Vec<RepoPath>, GitError> {
    Ok(status::read(r, o)
        .await?
        .conflicts
        .into_iter()
        .map(|entry| entry.path)
        .collect())
}
async fn published(r: &GitRunner, o: &RunOptions, hash: &str) -> Result<bool, GitError> {
    let refs = read(
        r,
        o,
        ReadOperation::ForEachRef,
        &[
            "--contains".into(),
            hash.into(),
            "--format=%(refname)".into(),
            "refs/remotes".into(),
        ],
    )
    .await?;
    Ok(!refs.is_empty())
}

pub async fn execute(
    r: &GitRunner,
    o: &RunOptions,
    request: &HistoryWrite,
    expected: &str,
) -> Result<HistoryOutcome, CommitError> {
    let _gate = tokio::select! { _ = o.cancellation.cancelled() => return Err(GitError::Cancelled.into()), guard = WRITES.lock() => guard };
    before_write(r, o, expected).await?;
    match request {
        HistoryWrite::CherryPick {
            commits,
            record_origin,
            empty,
        } => cherry_pick(r, o, commits, *record_origin, *empty, expected).await,
        HistoryWrite::Revert { commits } => revert(r, o, commits, expected).await,
        HistoryWrite::Reset { target, mode } => reset(r, o, target, *mode, expected).await,
        HistoryWrite::CheckoutRevision { target, mode } => {
            checkout_revision(r, o, target, *mode, expected).await
        }
        HistoryWrite::CreateTag {
            name,
            target,
            message,
        } => create_tag(r, o, name, target, message.as_deref(), expected).await,
        HistoryWrite::DeleteTag { name } => delete_tag(r, o, name, expected).await,
        HistoryWrite::DeleteRemoteTag { remote, name } => {
            delete_remote_tag(r, o, remote, name, expected).await
        }
        HistoryWrite::PushTag { remote, name } => push_tag(r, o, remote, name, expected).await,
    }
}

async fn cherry_pick(
    r: &GitRunner,
    o: &RunOptions,
    commits: &[String],
    origin: bool,
    empty: EmptyCherryPick,
    expected: &str,
) -> Result<HistoryOutcome, CommitError> {
    if commits.is_empty() {
        return Err(repo::invalid("history.noCommits").into());
    }
    let mut pinned = Vec::with_capacity(commits.len());
    for commit in commits {
        pinned.push(pin(r, o, commit).await?);
    }
    let mut dated = Vec::with_capacity(pinned.len());
    for (index, hash) in pinned.into_iter().enumerate() {
        let out = read(
            r,
            o,
            ReadOperation::Log,
            &[
                "--format=%ct".into(),
                "-n".into(),
                "1".into(),
                hash.clone(),
                "--".into(),
            ],
        )
        .await?;
        let time = String::from_utf8_lossy(&out)
            .trim()
            .parse::<i64>()
            .unwrap_or_default();
        let commit_parents = parents(r, o, &hash).await?;
        dated.push((time, index, hash, commit_parents));
    }
    let ordered = cherry_order(dated);
    let mut expected = expected.to_owned();
    for hash in ordered {
        ensure_snapshot(r, o, &expected).await?;
        ensure_no_staged_changes(r, o).await?;
        let mut args = vec!["--no-edit".to_owned()];
        if origin && published(r, o, &hash).await? {
            args.push("-x".into());
        }
        args.extend(["--".into(), hash]);
        match write(r, o, "cherry-pick", &args).await {
            Ok(_) => expected = snapshot(r, o).await?.fingerprint,
            Err(error) => {
                let files = current_conflicts(r, o).await?;
                if !files.is_empty() {
                    return Ok(conflict_result(files));
                }
                let detail = redact(&error.to_string());
                if detail.contains("previous cherry-pick is now empty")
                    || detail.contains("nothing to commit")
                {
                    expected = snapshot(r, o).await?.fingerprint;
                    ensure_snapshot(r, o, &expected).await?;
                    match empty {
                        EmptyCherryPick::Skip => {
                            write(r, o, "cherry-pick", &["--skip".into()]).await?;
                        }
                        EmptyCherryPick::CreateEmpty => {
                            write(
                                r,
                                o,
                                "commit",
                                &[
                                    "--allow-empty".into(),
                                    "-C".into(),
                                    "CHERRY_PICK_HEAD".into(),
                                ],
                            )
                            .await?;
                        }
                    }
                    expected = snapshot(r, o).await?.fingerprint;
                } else {
                    return Ok(HistoryOutcome::Stopped { detail });
                }
            }
        }
    }
    Ok(HistoryOutcome::Completed)
}

/// Pick the oldest currently applicable commit, retaining graph safety when commit dates
/// are skewed. Ready commits with equal timestamps retain the user's log selection order.
fn cherry_order(commits: Vec<(i64, usize, String, Vec<String>)>) -> Vec<String> {
    let indexes: HashMap<_, _> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| (c.2.clone(), i))
        .collect();
    let mut children = vec![Vec::new(); commits.len()];
    let mut indegree = vec![0usize; commits.len()];
    for (child_index, commit) in commits.iter().enumerate() {
        for parent in &commit.3 {
            if let Some(&parent_index) = indexes.get(parent) {
                children[parent_index].push(child_index);
                indegree[child_index] += 1;
            }
        }
    }
    let mut ready: Vec<_> = indegree
        .iter()
        .enumerate()
        .filter_map(|(i, count)| (*count == 0).then_some(i))
        .collect();
    let mut result = Vec::with_capacity(commits.len());
    while !ready.is_empty() {
        ready.sort_by_key(|&index| (commits[index].0, commits[index].1));
        let next = ready.remove(0);
        result.push(commits[next].2.clone());
        for &child in &children[next] {
            indegree[child] -= 1;
            if indegree[child] == 0 {
                ready.push(child);
            }
        }
    }
    // Commit graphs are acyclic; this is defensive and preserves every validated hash.
    if result.len() != commits.len() {
        return commits.into_iter().map(|commit| commit.2).collect();
    }
    result
}

async fn revert(
    r: &GitRunner,
    o: &RunOptions,
    commits: &[String],
    expected: &str,
) -> Result<HistoryOutcome, CommitError> {
    if commits.is_empty() {
        return Err(repo::invalid("history.noCommits").into());
    }
    let mut pinned = vec![];
    for commit in commits {
        let hash = pin(r, o, commit).await?;
        if parents(r, o, &hash).await?.len() > 1 {
            return Err(repo::invalid("history.revertMergeCommit").into());
        }
        pinned.push(hash);
    }
    let mut expected = expected.to_owned();
    for hash in pinned {
        ensure_snapshot(r, o, &expected).await?;
        ensure_no_staged_changes(r, o).await?;
        match write(r, o, "revert", &["--no-edit".into(), "--".into(), hash]).await {
            Ok(_) => expected = snapshot(r, o).await?.fingerprint,
            Err(error) => {
                let files = current_conflicts(r, o).await?;
                if !files.is_empty() {
                    return Ok(conflict_result(files));
                }
                return Ok(HistoryOutcome::Stopped {
                    detail: redact(&error.to_string()),
                });
            }
        }
    }
    Ok(HistoryOutcome::Completed)
}

async fn reset(
    r: &GitRunner,
    o: &RunOptions,
    target: &str,
    mode: ResetMode,
    expected: &str,
) -> Result<HistoryOutcome, CommitError> {
    let context = Context {
        runner: r,
        options: o.clone(),
    };
    let head = context
        .head()
        .await?
        .ok_or_else(|| repo::invalid("history.noHead"))?;
    history_edit::check_editable(&context, &head).await?;
    ensure_snapshot(r, o, expected).await?;
    let target = pin(r, o, target).await?;
    ensure_snapshot(r, o, expected).await?;
    let flag = match mode {
        ResetMode::Soft => "--soft",
        ResetMode::Mixed => "--mixed",
        ResetMode::Hard => "--hard",
        ResetMode::Keep => "--keep",
    };
    match write(
        r,
        o,
        "reset",
        &[flag.into(), "--end-of-options".into(), target],
    )
    .await
    {
        Ok(_) => Ok(HistoryOutcome::Completed),
        Err(error) if matches!(mode, ResetMode::Keep) => Ok(HistoryOutcome::Stopped {
            detail: redact(&error.to_string()),
        }),
        Err(error) => Err(error.into()),
    }
}

async fn checkout_revision(
    r: &GitRunner,
    o: &RunOptions,
    target: &str,
    mode: super::branch::CheckoutMode,
    expected: &str,
) -> Result<HistoryOutcome, CommitError> {
    ensure_snapshot(r, o, expected).await?;
    let target = pin(r, o, target).await?;
    ensure_snapshot(r, o, expected).await?;
    match super::branch::checkout_with_mode(r, o, vec!["--detach".into(), target], mode, false)
        .await?
    {
        super::branch::BranchOutcome::Completed => Ok(HistoryOutcome::Completed),
        super::branch::BranchOutcome::LocalChanges { files } => {
            Ok(HistoryOutcome::LocalChanges { files })
        }
        super::branch::BranchOutcome::StashRecovery { stash, detail, .. } => {
            Ok(HistoryOutcome::StashRecovery {
                stash,
                detail: redact(&detail),
            })
        }
        super::branch::BranchOutcome::NotMerged { name } => {
            Ok(HistoryOutcome::Stopped { detail: name })
        }
    }
}

async fn validate_tag(r: &GitRunner, o: &RunOptions, name: &str) -> Result<(), GitError> {
    if name.starts_with('-') || name.starts_with('@') || name.is_empty() {
        return Err(repo::invalid("history.invalidTagName"));
    }
    write(r, o, "check-ref-format", &[format!("refs/tags/{name}")]).await?;
    Ok(())
}
async fn create_tag(
    r: &GitRunner,
    o: &RunOptions,
    name: &str,
    target: &str,
    message: Option<&str>,
    expected: &str,
) -> Result<HistoryOutcome, CommitError> {
    validate_tag(r, o, name).await?;
    let target = pin(r, o, target).await?;
    ensure_snapshot(r, o, expected).await?;
    if message.is_some_and(str::is_empty) {
        return Err(repo::invalid("history.emptyTagMessage").into());
    }
    let mut args = vec![];
    if let Some(message) = message {
        args.extend(["-a".into(), "-m".into(), message.into()]);
    }
    args.extend(["--".into(), name.into(), target]);
    write(r, o, "tag", &args).await?;
    Ok(HistoryOutcome::Completed)
}
async fn delete_tag(
    r: &GitRunner,
    o: &RunOptions,
    name: &str,
    expected: &str,
) -> Result<HistoryOutcome, CommitError> {
    validate_tag(r, o, name).await?;
    ensure_snapshot(r, o, expected).await?;
    write(r, o, "tag", &["-d".into(), "--".into(), name.into()]).await?;
    Ok(HistoryOutcome::Completed)
}
async fn delete_remote_tag(
    r: &GitRunner,
    o: &RunOptions,
    remote: &str,
    name: &str,
    expected: &str,
) -> Result<HistoryOutcome, CommitError> {
    validate_tag(r, o, name).await?;
    super::remote::remote_name(r, o, Some(remote)).await?;
    ensure_snapshot(r, o, expected).await?;
    let output = write(
        r,
        &super::remote::transport_options(r, o).await?,
        "push",
        &[
            "--porcelain".into(),
            remote.into(),
            format!(":refs/tags/{name}"),
        ],
    )
    .await?;
    if !String::from_utf8_lossy(&output).contains(&format!("refs/tags/{name}")) {
        return Err(repo::invalid("history.remoteTagMissing").into());
    }
    Ok(HistoryOutcome::Completed)
}

async fn push_tag(
    r: &GitRunner,
    o: &RunOptions,
    remote: &str,
    name: &str,
    expected: &str,
) -> Result<HistoryOutcome, CommitError> {
    validate_tag(r, o, name).await?;
    super::remote::remote_name(r, o, Some(remote)).await?;
    let refs = repo::refs::read(r, o).await?;
    if !refs
        .tags
        .iter()
        .any(|reference| reference.name == format!("refs/tags/{name}"))
    {
        return Err(repo::invalid("history.tagMissing").into());
    }
    ensure_snapshot(r, o, expected).await?;
    let refspec = format!("refs/tags/{name}:refs/tags/{name}");
    write(
        r,
        &super::remote::transport_options(r, o).await?,
        "push",
        &[remote.into(), refspec],
    )
    .await?;
    Ok(HistoryOutcome::Completed)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildCommits {
    pub children: Vec<String>,
}
pub async fn children(
    r: &GitRunner,
    o: &RunOptions,
    target: &str,
) -> Result<ChildCommits, GitError> {
    let target = pin(r, o, target).await?;
    let out = read(
        r,
        o,
        ReadOperation::Log,
        &[
            "--format=%H %P".into(),
            "--all".into(),
            "--topo-order".into(),
            "--".into(),
        ],
    )
    .await?;
    let mut children: Vec<_> = String::from_utf8_lossy(&out)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let child = fields.next()?;
            fields
                .any(|parent| parent == target)
                .then(|| child.to_owned())
        })
        .collect();
    children.sort();
    Ok(ChildCommits { children })
}
