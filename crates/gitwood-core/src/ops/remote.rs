//! User-initiated remote transport. Targets are configured remote names, never URLs.
use crate::{
    git::{GitError, GitOutput, GitRunner, ReadCommand, ReadOperation, RunOptions, WriteCommand},
    repo::{self, RepoPath},
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteError {
    pub key: String,
    pub detail: String,
    pub guidance: &'static str,
}
impl From<GitError> for RemoteError {
    fn from(e: GitError) -> Self {
        let text = redact(&e.to_string());
        let key = match &e {
            GitError::Cancelled => "remote.cancelled",
            GitError::Timeout(_) => "remote.timeout",
            _ if text.contains("Host key verification failed")
                || text.contains("REMOTE HOST IDENTIFICATION HAS CHANGED") =>
            {
                "remote.hostKey"
            }
            _ if text.contains("Authentication failed")
                || text.contains("Permission denied")
                || text.contains("could not read Username") =>
            {
                "remote.authentication"
            }
            _ if text.contains("Could not resolve")
                || text.contains("Connection refused")
                || text.contains("Network is unreachable")
                || text.contains("does not appear to be a git repository") =>
            {
                "remote.unreachable"
            }
            _ if text.contains("timed out") => "remote.timeout",
            _ => "remote.failed",
        };
        // Authentication problems are fixed in the credential helper or SSH agent, not in
        // a terminal; other failures still point at the Git configuration and output.
        let guidance = match key {
            "remote.authentication" | "remote.hostKey" => "remote.checkCredentials",
            _ => "remote.tryFetchInTerminal",
        };
        Self {
            key: key.into(),
            detail: text,
            guidance,
        }
    }
}
pub use crate::git::redact;
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushOptions {
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub force_with_lease: bool,
    pub tags: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UpdateStrategy {
    Merge,
    Rebase,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushPreview {
    pub remote: String,
    pub branch: String,
    pub commits: Vec<String>,
    /// Subject line for each entry of `commits` (same order and length).
    pub subjects: Vec<String>,
    pub total: usize,
    pub start: usize,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefResult {
    pub flag: char,
    pub source: String,
    pub target: String,
    pub summary: String,
    pub rejection: Option<PushRejection>,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PushRejection {
    NeedsUpdate,
    RemoteRule,
    StaleLease,
    Other,
}
fn rejection(flag: char, reason: &str) -> Option<PushRejection> {
    if flag != '!' {
        return None;
    }
    Some(if reason.contains("(stale info)") {
        PushRejection::StaleLease
    } else if reason.starts_with("[remote rejected]") {
        PushRejection::RemoteRule
    } else if reason.starts_with("[rejected]")
        && (reason.contains("(fetch first)") || reason.contains("(non-fast-forward)"))
    {
        PushRejection::NeedsUpdate
    } else {
        PushRejection::Other
    })
}
#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RemoteOutcome {
    Completed,
    Interrupted {
        stash: Option<String>,
        error: RemoteError,
    },
    Push {
        success: bool,
        refs: Vec<RefResult>,
    },
    Conflicts {
        files: Vec<RepoPath>,
        stash: Option<String>,
    },
    StashRecovery {
        stash: String,
        files: Vec<RepoPath>,
    },
}
async fn cmd(
    r: &GitRunner,
    o: &RunOptions,
    name: &str,
    args: &[&str],
) -> Result<GitOutput, GitError> {
    r.run(WriteCommand::new(name).args(args.iter().copied()), o)
        .await
}
async fn config(r: &GitRunner, o: &RunOptions, key: &str) -> Result<Option<String>, GitError> {
    match cmd(r, o, "config", &["--get", key]).await {
        Ok(v) => Ok(Some(String::from_utf8_lossy(&v.stdout).trim().into())),
        Err(GitError::Exit { code: Some(1), .. }) => Ok(None),
        Err(e) => Err(e),
    }
}
async fn configured_update_strategy(
    r: &GitRunner,
    o: &RunOptions,
    branch: &str,
) -> Result<UpdateStrategy, GitError> {
    let method = match config(r, o, &format!("branch.{branch}.rebase")).await? {
        Some(value) => value,
        None => config(r, o, "pull.rebase")
            .await?
            .unwrap_or_else(|| "false".into()),
    };
    Ok(
        if matches!(
            method.as_str(),
            "true" | "yes" | "on" | "1" | "merges" | "interactive" | "preserve"
        ) {
            UpdateStrategy::Rebase
        } else {
            UpdateStrategy::Merge
        },
    )
}
pub async fn default_update_strategy(
    r: &GitRunner,
    o: &RunOptions,
) -> Result<UpdateStrategy, GitError> {
    let state = repo::status::read(r, o).await?;
    let branch = state
        .branch
        .head
        .ok_or_else(|| repo::invalid("remote.detachedHead"))?;
    configured_update_strategy(r, o, &branch).await
}
async fn remote_name(
    r: &GitRunner,
    o: &RunOptions,
    requested: Option<&str>,
) -> Result<String, GitError> {
    let output = cmd(r, o, "remote", &[]).await?;
    let names: Vec<_> = std::str::from_utf8(&output.stdout)
        .map_err(|_| repo::invalid("remote.invalidName"))?
        .lines()
        .collect();
    let name = requested
        .or_else(|| names.contains(&"origin").then_some("origin"))
        .or_else(|| (names.len() == 1).then(|| names[0]))
        .ok_or_else(|| repo::invalid("remote.chooseTarget"))?;
    if name.starts_with('-') || !names.contains(&name) {
        return Err(repo::invalid("remote.invalidName"));
    }
    Ok(name.into())
}
async fn transport_options(r: &GitRunner, o: &RunOptions) -> Result<RunOptions, GitError> {
    let mut options = o.clone();
    options.env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
    let inherited = std::env::var_os("GIT_SSH_COMMAND").is_some()
        || std::env::var_os("GIT_SSH").is_some()
        || options
            .env
            .contains_key(std::ffi::OsStr::new("GIT_SSH_COMMAND"))
        || options.env.contains_key(std::ffi::OsStr::new("GIT_SSH"));
    // BatchMode would disable the SSH prompt the askpass helper exists to answer.
    if !inherited && config(r, o, "core.sshCommand").await?.is_none() && o.askpass.is_none() {
        options
            .env
            .insert("GIT_SSH_COMMAND".into(), "ssh -o BatchMode=yes".into());
    }
    Ok(options)
}
async fn check(r: &GitRunner, o: &RunOptions, expected: &str) -> Result<(), GitError> {
    let repository = repo::Repository::open(r, o).await?;
    if repository.git_dir.join("index.lock").exists() {
        return Err(repo::invalid("remote.indexLocked"));
    }
    if super::snapshot(r, o).await?.fingerprint != expected {
        return Err(repo::invalid("remote.concurrentChange"));
    }
    Ok(())
}
pub async fn preview(
    r: &GitRunner,
    o: &RunOptions,
    p: &PushOptions,
) -> Result<PushPreview, RemoteError> {
    let refs = repo::refs::read(r, o).await?;
    let head = refs
        .head
        .head
        .ok_or_else(|| repo::invalid("remote.detachedHead"))?;
    let configured = config(r, o, &format!("branch.{head}.remote")).await?;
    let remote = remote_name(r, o, p.remote.as_deref().or(configured.as_deref())).await?;
    let upstream = config(r, o, &format!("branch.{head}.merge")).await?;
    let branch = p
        .branch
        .clone()
        .or_else(|| upstream.and_then(|s| s.strip_prefix("refs/heads/").map(str::to_owned)))
        .unwrap_or(head);
    if branch.starts_with('-') {
        return Err(repo::invalid("remote.invalidBranch").into());
    }
    cmd(r, o, "check-ref-format", &["--branch", &branch]).await?;
    let target = format!("refs/remotes/{remote}/{branch}");
    let mut args = vec!["--format=%H%x00%s".to_owned(), "HEAD".into()];
    if refs.remotes.values().flatten().any(|b| b.name == target) {
        args.push(format!("^{target}"));
    } else {
        args.extend(["--not".into(), "--remotes".into()]);
    }
    let out = r
        .run(ReadCommand::new(ReadOperation::Log).args(args), o)
        .await?;
    let (commits, subjects): (Vec<_>, Vec<_>) = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (hash, subject) = line.split_once('\0').unwrap_or((line, ""));
            (hash.to_owned(), subject.to_owned())
        })
        .unzip();
    Ok(PushPreview {
        remote,
        branch,
        total: commits.len(),
        start: 0,
        commits,
        subjects,
    })
}
pub async fn fetch(
    r: &GitRunner,
    o: &RunOptions,
    remote: Option<&str>,
    all: bool,
    expected: &str,
) -> Result<RemoteOutcome, RemoteError> {
    let _gate = super::WRITES.lock().await;
    check(r, o, expected).await?;
    let options = transport_options(r, o).await?;
    let target = if all {
        "--all".into()
    } else {
        let state = repo::status::read(r, o).await?;
        let configured = if let Some(head) = state.branch.head {
            config(r, o, &format!("branch.{head}.remote")).await?
        } else {
            None
        };
        remote_name(r, o, remote.or(configured.as_deref())).await?
    };
    cmd(r, &options, "fetch", &["--prune", "--progress", &target]).await?;
    check(r, o, expected).await?;
    Ok(RemoteOutcome::Completed)
}
pub async fn push(
    r: &GitRunner,
    o: &RunOptions,
    p: &PushOptions,
    expected: &str,
) -> Result<RemoteOutcome, RemoteError> {
    let _gate = super::WRITES.lock().await;
    check(r, o, expected).await?;
    let target = preview(r, o, p).await?;
    let options = transport_options(r, o).await?;
    let refspec = format!("HEAD:refs/heads/{}", target.branch);
    let mut args = vec!["--porcelain", "--progress", "--set-upstream"];
    if p.force_with_lease {
        args.push("--force-with-lease");
    }
    if p.tags {
        args.push("--tags");
    }
    args.extend([target.remote.as_str(), refspec.as_str()]);
    let output = r
        .run_report(WriteCommand::new("push").args(args), &options)
        .await?;
    let refs: Vec<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let flag = fields.next()?.chars().next()?;
            if ![' ', '+', '-', '*', '=', '!'].contains(&flag) {
                return None;
            }
            let (source, target) = fields.next()?.split_once(':')?;
            Some(RefResult {
                flag,
                source: source.into(),
                target: target.into(),
                summary: redact(fields.clone().next().unwrap_or_default()),
                rejection: rejection(flag, fields.next().unwrap_or_default()),
            })
        })
        .collect();
    if output.code != Some(0) && refs.is_empty() {
        return Err(GitError::Exit {
            code: output.code,
            stderr: output.stderr,
        }
        .into());
    }
    check(r, o, expected).await?;
    Ok(RemoteOutcome::Push {
        success: output.code == Some(0),
        refs,
    })
}

async fn stash_id(r: &GitRunner, o: &RunOptions) -> Result<Option<String>, GitError> {
    match r
        .run(
            ReadCommand::new(ReadOperation::RevParse).args(["--verify", "-q", "refs/stash"]),
            o,
        )
        .await
    {
        Ok(v) => Ok(Some(String::from_utf8_lossy(&v.stdout).trim().into())),
        Err(GitError::Exit { code: Some(1), .. }) => Ok(None),
        Err(e) => Err(e),
    }
}
async fn restore(r: &GitRunner, o: &RunOptions, stash: &str) -> Result<RemoteOutcome, RemoteError> {
    if super::branch::stash_position(r, o, stash).await?.is_none()
        || cmd(r, o, "stash", &["apply", stash]).await.is_err()
    {
        return Ok(RemoteOutcome::StashRecovery {
            stash: stash.into(),
            files: repo::status::read(r, o)
                .await?
                .conflicts
                .into_iter()
                .map(|e| e.path)
                .collect(),
        });
    }
    super::branch::drop_saved_stash(r, o, stash).await?;
    Ok(RemoteOutcome::Completed)
}
/// Update leaves conflicts in place. The returned stash ID belongs to the caller's
/// recovery state and can be supplied when aborting the merge/rebase.
pub async fn update(
    r: &GitRunner,
    o: &RunOptions,
    expected: &str,
    strategy: Option<UpdateStrategy>,
) -> Result<RemoteOutcome, RemoteError> {
    let _gate = super::WRITES.lock().await;
    check(r, o, expected).await?;
    let repository = repo::Repository::open(r, o).await?;
    if [
        "MERGE_HEAD",
        "rebase-merge",
        "rebase-apply",
        "CHERRY_PICK_HEAD",
    ]
    .iter()
    .any(|name| repository.git_dir.join(name).exists())
    {
        return Err(repo::invalid("remote.operationInProgress").into());
    }
    let state = repo::status::read(r, o).await?;
    if !state.conflicts.is_empty() {
        return Err(repo::invalid("remote.resolveConflictsFirst").into());
    }
    let head = state
        .branch
        .head
        .ok_or_else(|| repo::invalid("remote.detachedHead"))?;
    let remote = config(r, o, &format!("branch.{head}.remote"))
        .await?
        .ok_or_else(|| repo::invalid("remote.upstreamRequired"))?;
    let remote = remote_name(r, o, Some(&remote)).await?;
    let merge = config(r, o, &format!("branch.{head}.merge"))
        .await?
        .ok_or_else(|| repo::invalid("remote.upstreamRequired"))?;
    let branch = merge
        .strip_prefix("refs/heads/")
        .ok_or_else(|| repo::invalid("remote.invalidBranch"))?;
    let upstream = format!("refs/remotes/{remote}/{branch}");
    let options = transport_options(r, o).await?;
    cmd(r, &options, "fetch", &["--prune", "--progress", &remote]).await?;
    // Fetch may take minutes; reject edits observed while it was running.
    check(r, o, expected).await?;
    let strategy = match strategy {
        Some(strategy) => strategy,
        None => configured_update_strategy(r, o, &head).await?,
    };
    let before = stash_id(r, o).await?;
    let saved = cmd(r, o, "stash", &["push", "-m", "Gitwood Update Project"]).await;
    let mut recovery = o.clone();
    recovery.cancellation = crate::git::CancellationToken::new();
    let stash = stash_id(r, &recovery)
        .await?
        .filter(|id| Some(id) != before.as_ref());
    if let Err(error) = saved {
        if stash.is_some() {
            return Ok(RemoteOutcome::Interrupted {
                stash,
                error: error.into(),
            });
        }
        return Err(error.into());
    }
    let result = if strategy == UpdateStrategy::Rebase {
        cmd(r, &options, "rebase", &[&upstream]).await
    } else {
        cmd(r, &options, "merge", &["--no-edit", &upstream]).await
    };
    let mut recovery = o.clone();
    recovery.cancellation = crate::git::CancellationToken::new();
    let files: Vec<_> = repo::status::read(r, &recovery)
        .await?
        .conflicts
        .into_iter()
        .map(|e| e.path)
        .collect();
    if let Err(error) = result.as_ref() {
        if ["MERGE_HEAD", "rebase-merge", "rebase-apply"]
            .iter()
            .any(|name| repository.git_dir.join(name).exists())
            && files.is_empty()
        {
            return Ok(RemoteOutcome::Interrupted {
                stash,
                error: RemoteError {
                    key: "remote.interrupted".into(),
                    detail: redact(&error.to_string()),
                    guidance: "remote.abortUpdate",
                },
            });
        }
    }
    if !files.is_empty() {
        return Ok(RemoteOutcome::Conflicts { files, stash });
    }
    if let Some(stash) = stash {
        let restored = restore(r, &recovery, &stash).await?;
        if !matches!(restored, RemoteOutcome::Completed) {
            return Ok(restored);
        }
    }
    result?;
    Ok(RemoteOutcome::Completed)
}
pub async fn abort_update(
    r: &GitRunner,
    o: &RunOptions,
    expected: &str,
    stash: Option<&str>,
) -> Result<RemoteOutcome, RemoteError> {
    let _gate = super::WRITES.lock().await;
    check(r, o, expected).await?;
    if let Some(stash) = stash {
        if super::branch::stash_position(r, o, stash).await?.is_none() {
            return Err(repo::invalid("remote.stashChanged").into());
        }
    }
    let repository = repo::Repository::open(r, o).await?;
    if repository.git_dir.join("rebase-merge").exists()
        || repository.git_dir.join("rebase-apply").exists()
    {
        cmd(r, o, "rebase", &["--abort"]).await?;
    } else {
        cmd(r, o, "merge", &["--abort"]).await?;
    }
    if let Some(stash) = stash {
        if ![40, 64].contains(&stash.len()) || !stash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(repo::invalid("remote.invalidStash").into());
        }
        return restore(r, o, stash).await;
    }
    Ok(RemoteOutcome::Completed)
}
