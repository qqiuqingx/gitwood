//! Explicit integration requests. All targets are pinned before touching local changes.
use super::remote::{self, RemoteError, RemoteOutcome};
use crate::{
    git::{
        CancellationToken, GitError, GitRunner, ReadCommand, ReadOperation, RunOptions,
        WriteCommand,
    },
    repo::{self, operation, Repository},
};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MergeOptions {
    pub no_ff: bool,
    pub ff_only: bool,
    pub squash: bool,
    pub no_commit: bool,
    pub message: Option<String>,
}
impl MergeOptions {
    pub fn arguments(&self) -> Result<Vec<String>, GitError> {
        let count = u8::from(self.no_ff) + u8::from(self.ff_only) + u8::from(self.squash);
        if count > 1 || (self.message.is_some() && (self.ff_only || self.squash || self.no_commit))
        {
            return Err(repo::invalid("integration.incompatibleOptions"));
        }
        if self.message.as_ref().is_some_and(|s| s.trim().is_empty()) {
            return Err(repo::invalid("integration.messageRequired"));
        }
        let mut args = vec!["--no-edit".into(), "--no-autostash".into()];
        for (enabled, flag) in [
            (self.no_ff, "--no-ff"),
            (self.ff_only, "--ff-only"),
            (self.squash, "--squash"),
            (self.no_commit, "--no-commit"),
        ] {
            if enabled {
                args.push(flag.into());
            }
        }
        if let Some(message) = &self.message {
            args.extend(["-m".into(), message.clone()]);
        }
        Ok(args)
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Integration {
    Merge {
        target: String,
        options: MergeOptions,
    },
    Rebase {
        target: String,
        onto: Option<String>,
    },
    CheckoutRebase {
        branch: String,
        local_name: Option<String>,
    },
    Pull {
        remote: String,
        branches: Vec<String>,
        rebase: bool,
        options: MergeOptions,
    },
}
struct Plan {
    command: &'static str,
    args: Vec<String>,
    targets: Vec<(String, String)>,
    checkout: Option<Vec<String>>,
    branch: Option<String>,
}
async fn command(
    r: &GitRunner,
    o: &RunOptions,
    name: &str,
    args: &[String],
) -> Result<(), GitError> {
    r.run(WriteCommand::new(name).args(args.iter().cloned()), o)
        .await?;
    Ok(())
}
/// Resolve only refs or commit expressions, never options. Recheck named targets before writes.
pub(crate) async fn pin(r: &GitRunner, o: &RunOptions, target: &str) -> Result<String, GitError> {
    if target.is_empty()
        || target.starts_with('-')
        || target.starts_with('@')
        || target.chars().any(char::is_whitespace)
    {
        return Err(repo::invalid("integration.invalidTarget"));
    }
    if target.starts_with("refs/") {
        r.run(
            ReadCommand::new(ReadOperation::CheckRefFormat).args([target]),
            o,
        )
        .await?;
    }
    let out = r
        .run(
            ReadCommand::new(ReadOperation::RevParse).args([
                "--verify",
                "--end-of-options",
                &format!("{target}^{{commit}}"),
            ]),
            o,
        )
        .await?;
    let oid = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    crate::history::details::validate_hash(&oid)?;
    Ok(oid)
}
async fn target(
    r: &GitRunner,
    o: &RunOptions,
    plan: &mut Plan,
    name: &str,
) -> Result<String, GitError> {
    let oid = pin(r, o, name).await?;
    plan.targets.push((name.into(), oid.clone()));
    Ok(oid)
}
async fn plan(
    r: &GitRunner,
    o: &RunOptions,
    request: &Integration,
    expected: &str,
) -> Result<Plan, GitError> {
    let mut p = Plan {
        command: "merge",
        args: vec![],
        targets: vec![],
        checkout: None,
        branch: repo::status::read(r, o).await?.branch.head,
    };
    target(r, o, &mut p, "HEAD").await?;
    match request {
        Integration::Merge {
            target: name,
            options,
        } => {
            p.args = options.arguments()?;
            let refs = repo::refs::read(r, o).await?;
            if !refs
                .local
                .iter()
                .chain(refs.remotes.values().flatten())
                .any(|b| b.name == *name)
            {
                return Err(repo::invalid("integration.branchRequired"));
            }
            let oid = target(r, o, &mut p, name).await?;
            let (kind, short) = if let Some(short) = name.strip_prefix("refs/heads/") {
                ("branch", short)
            } else {
                (
                    "remote-tracking branch",
                    name.strip_prefix("refs/remotes/").unwrap(),
                )
            };
            // Git uses "of ." for local sources, including shortlog label shortening.
            let input = format!("{oid}\t\t{kind} '{short}' of .\n");
            default_message(r, o, options, input.as_bytes(), &mut p.args).await?;
            p.args.extend(["--".into(), oid]);
        }
        Integration::Rebase { target: name, onto } => {
            p.command = "rebase";
            // Prevent pull.rebase=interactive / rebase.autoStash from taking over this workflow.
            p.args = vec!["--no-autostash".into()];
            if let Some(onto) = onto {
                let oid = target(r, o, &mut p, onto).await?;
                p.args.extend(["--onto".into(), oid]);
            }
            let oid = target(r, o, &mut p, name).await?;
            p.args.extend(["--".into(), oid]);
        }
        Integration::CheckoutRebase { branch, local_name } => {
            p.checkout =
                Some(checkout_plan(r, o, &mut p.targets, branch, local_name.as_deref()).await?);
            let oid = target(r, o, &mut p, "HEAD").await?;
            p.command = "rebase";
            p.args = vec!["--no-autostash".into(), "--".into(), oid];
        }
        Integration::Pull {
            remote,
            branches,
            rebase,
            options,
        } => {
            pull_plan(r, o, &mut p, remote, branches, *rebase, options).await?;
            remote::check(r, o, expected).await?;
        }
    }
    Ok(p)
}
async fn checkout_plan(
    r: &GitRunner,
    o: &RunOptions,
    targets: &mut Vec<(String, String)>,
    branch: &str,
    local_name: Option<&str>,
) -> Result<Vec<String>, GitError> {
    let refs = repo::refs::read(r, o).await?;
    if refs.local.iter().any(|b| b.name == branch) {
        let short = branch
            .strip_prefix("refs/heads/")
            .ok_or_else(|| repo::invalid("integration.branchRequired"))?;
        if refs.head.head.as_deref() == Some(short) {
            return Err(repo::invalid("integration.currentBranch"));
        }
        targets.push((branch.into(), pin(r, o, branch).await?));
        return Ok(vec![short.into(), "--".into()]);
    }
    let remote = refs
        .remotes
        .iter()
        .find(|(_, values)| values.iter().any(|b| b.name == branch))
        .map(|(name, _)| name)
        .ok_or_else(|| repo::invalid("integration.branchRequired"))?;
    let prefix = format!("refs/remotes/{remote}/");
    let name = local_name.unwrap_or(
        branch
            .strip_prefix(&prefix)
            .ok_or_else(|| repo::invalid("integration.branchRequired"))?,
    );
    super::branch::validate(r, o, name).await?;
    if refs
        .local
        .iter()
        .any(|b| b.name == format!("refs/heads/{name}"))
    {
        return Err(repo::invalid("integration.localBranchExists"));
    }
    targets.push((branch.into(), pin(r, o, branch).await?));
    Ok(vec![
        "--track".into(),
        "-b".into(),
        name.into(),
        branch.into(),
        "--".into(),
    ])
}
async fn pull_plan(
    r: &GitRunner,
    o: &RunOptions,
    p: &mut Plan,
    remote: &str,
    branches: &[String],
    rebase: bool,
    options: &MergeOptions,
) -> Result<(), GitError> {
    if branches.is_empty()
        || branches.len() > 200
        || (rebase
            && (branches.len() != 1
                || options.no_ff
                || options.ff_only
                || options.squash
                || options.no_commit
                || options.message.is_some()))
    {
        return Err(repo::invalid("integration.incompatibleOptions"));
    }
    // Pull's IDEA options do not include -m.
    if options.message.is_some() {
        return Err(repo::invalid("integration.incompatibleOptions"));
    }
    p.args = options.arguments()?;
    let remote = remote::remote_name(r, o, Some(remote)).await?;
    let mut specs = vec!["--progress".into(), "--".into(), remote.clone()];
    let mut names = std::collections::HashSet::new();
    for branch in branches {
        super::branch::validate(r, o, branch).await?;
        if !names.insert(branch) {
            return Err(repo::invalid("integration.invalidTarget"));
        }
        specs.push(format!("refs/heads/{branch}"));
    }
    command(r, &remote::transport_options(r, o).await?, "fetch", &specs).await?;
    // FETCH_HEAD also handles custom fetch refspecs, without forcing tracking refs backwards.
    let repository = Repository::open(r, o).await?;
    let fetched = std::fs::read_to_string(repository.git_dir.join("FETCH_HEAD"))?;
    let ids: Vec<_> = fetched
        .lines()
        .filter_map(|line| line.split_once('\t').map(|(id, _)| id.to_owned()))
        .collect();
    if ids.len() != branches.len() {
        return Err(repo::invalid("integration.fetchChanged"));
    }
    for id in &ids {
        crate::history::details::validate_hash(id)?;
    }
    if rebase {
        p.command = "rebase";
        p.args = vec!["--no-autostash".into()];
    }
    if !rebase {
        default_message(r, o, options, fetched.as_bytes(), &mut p.args).await?;
    }
    p.args.push("--".into());
    p.args.extend(ids);
    Ok(())
}
/// Keep the original ref descriptions while merging immutable objects. fmt-merge-msg
/// reads merge.log and destination settings; --no-log prevents merge appending it twice.
async fn default_message(
    r: &GitRunner,
    o: &RunOptions,
    options: &MergeOptions,
    input: &[u8],
    args: &mut Vec<String>,
) -> Result<(), GitError> {
    if options.message.is_some() || options.squash || options.ff_only {
        return Ok(());
    }
    let mut formatting = o.clone();
    formatting.stdin = input.to_vec();
    let output = r
        .run(WriteCommand::new("fmt-merge-msg"), &formatting)
        .await?;
    let message = String::from_utf8(output.stdout)
        .map_err(|_| repo::invalid("integration.invalidMessageEncoding"))?;
    args.extend(["--no-log".into(), "-m".into(), message]);
    Ok(())
}
async fn ready(r: &GitRunner, o: &RunOptions) -> Result<(), GitError> {
    let repository = Repository::open(r, o).await?;
    if super::update_recovery::pending(&repository.git_dir)?.is_some() {
        return Err(repo::invalid("remote.stashRecoveryRequired"));
    }
    if operation::read(&repository.git_dir)
        .map_err(GitError::from)?
        .is_some()
    {
        return Err(repo::invalid("remote.operationInProgress"));
    }
    let state = repo::status::read(r, o).await?;
    if !state.conflicts.is_empty() {
        return Err(repo::invalid("remote.resolveConflictsFirst"));
    }
    if state
        .branch
        .head
        .as_deref()
        .is_none_or(|h| h == "(detached)")
        || state.branch.oid.as_deref() == Some("(initial)")
    {
        return Err(repo::invalid("remote.detachedHead"));
    }
    Ok(())
}
/// The recovery marker and M1 continue/abort/skip own a saved stash until Git finishes.
pub async fn run(
    r: &GitRunner,
    o: &RunOptions,
    request: &Integration,
    expected: &str,
) -> Result<RemoteOutcome, RemoteError> {
    let _gate = tokio::select! {
        _ = o.cancellation.cancelled() => return Err(GitError::Cancelled.into()),
        guard = super::WRITES.lock() => guard,
    };
    remote::check(r, o, expected).await?;
    ready(r, o).await?;
    let p = plan(r, o, request, expected).await?;
    remote::check(r, o, expected).await?;
    for (name, oid) in &p.targets {
        if pin(r, o, name).await? != *oid {
            return Err(repo::invalid("remote.concurrentChange").into());
        }
    }
    let (stash, interrupted) = remote::protect(r, o, "Gitwood Integration").await?;
    if let Some(error) = interrupted {
        return Ok(RemoteOutcome::Interrupted { stash, error });
    }
    let result = execute(r, o, &p).await;
    finish(r, o, stash, result).await
}
async fn execute(r: &GitRunner, o: &RunOptions, p: &Plan) -> Result<(), GitError> {
    let clean = super::snapshot(r, o).await?.fingerprint;
    let state = repo::status::read(r, o).await?;
    if state.branch.head != p.branch
        || !state.staged.is_empty()
        || !state.unstaged.is_empty()
        || !state.conflicts.is_empty()
    {
        return Err(repo::invalid("remote.concurrentChange"));
    }
    let mut options = o.clone();
    options.env.insert("GIT_EDITOR".into(), ":".into());
    for (name, oid) in &p.targets {
        if pin(r, o, name).await? != *oid {
            return Err(repo::invalid("remote.concurrentChange"));
        }
    }
    if let Some(args) = &p.checkout {
        remote::check(r, o, &clean).await?;
        command(r, &options, "checkout", args).await?;
    }
    let current = if p.checkout.is_some() {
        super::snapshot(r, o).await?.fingerprint
    } else {
        clean
    };
    remote::check(r, o, &current).await?;
    command(r, &options, p.command, &p.args).await
}
async fn finish(
    r: &GitRunner,
    o: &RunOptions,
    stash: Option<String>,
    result: Result<(), GitError>,
) -> Result<RemoteOutcome, RemoteError> {
    if matches!(&result, Err(GitError::InvalidCommand(key)) if key == "remote.concurrentChange" || key == "remote.indexLocked")
    {
        return Ok(RemoteOutcome::Interrupted {
            stash,
            error: result.unwrap_err().into(),
        });
    }
    let mut recovery = o.clone();
    recovery.cancellation = CancellationToken::new();
    let repository = Repository::open(r, &recovery).await?;
    let files: Vec<_> = repo::status::read(r, &recovery)
        .await?
        .conflicts
        .into_iter()
        .map(|e| e.path)
        .collect();
    if !files.is_empty() {
        return Ok(RemoteOutcome::Conflicts { files, stash });
    }
    if operation::read(&repository.git_dir)
        .map_err(GitError::from)?
        .is_some()
    {
        return Ok(RemoteOutcome::Interrupted {
            stash,
            error: RemoteError {
                key: if result.is_ok() {
                    "integration.paused"
                } else {
                    "remote.interrupted"
                }
                .into(),
                detail: result
                    .err()
                    .map(|e| remote::redact(&e.to_string()))
                    .unwrap_or_default(),
                guidance: "sequence.inspectOutput",
            },
        });
    }
    if let Some(stash) = stash {
        let restored = remote::restore(r, &recovery, &stash).await?;
        if !matches!(restored, RemoteOutcome::Completed) {
            return Ok(restored);
        }
    }
    result?;
    Ok(RemoteOutcome::Completed)
}
