//! Confirmed rollback, using an alternate index while the real index stays locked.
use super::{context::Context, index::IndexTransaction, snapshot, CommitError, WRITES};
use crate::{
    git::{GitError, GitRunner, RunOptions, WriteCommand},
    repo::{os, status, RepoPath},
};
use std::{ffi::OsString, path::PathBuf};
pub async fn rollback(
    runner: &GitRunner,
    options: &RunOptions,
    entries: &[(status::Entry, bool)],
    delete_added: bool,
    expected: &str,
) -> Result<(), CommitError> {
    let _guard = tokio::select! { _ = options.cancellation.cancelled() => return Err(GitError::Cancelled.into()), guard = WRITES.lock() => guard };
    if entries.is_empty() {
        return Err(CommitError::InvalidSelection(
            "no rollback files selected".into(),
        ));
    }
    let context = Context {
        runner,
        options: options.clone(),
    };
    context.require_root().await?;
    let mut tx = IndexTransaction::acquire(context.git_path("index").await?)?;
    let before = snapshot(runner, options).await?;
    if before.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    let head = context.head().await?;
    let alternate = tx.path("rollback");
    tx.copy_original(&alternate)?;
    if !alternate.exists() {
        context
            .write("read-tree", &["--empty"], &alternate, &[])
            .await?;
    }
    let mut tracked = vec![];
    let mut added: Vec<PathBuf> = vec![];
    let mut renamed: Vec<PathBuf> = vec![];
    for (e, untracked) in entries {
        if e.kind == 'u' {
            return Err(CommitError::InvalidSelection(
                "resolve conflicts first".into(),
            ));
        }
        if *untracked || e.xy.starts_with('A') {
            added.push(PathBuf::from(os(e.path.bytes.clone())));
        }
        if let Some(original) = &e.original {
            renamed.push(PathBuf::from(os(e.path.bytes.clone())));
            tracked.push(PathBuf::from(os(original.bytes.clone())));
        } else if !*untracked && !e.xy.starts_with('A') {
            tracked.push(PathBuf::from(os(e.path.bytes.clone())));
        }
    }
    // Validate all parents before any destructive command; never traverse a symlink outside root.
    for path in tracked.iter().chain(&added).chain(&renamed) {
        validate_path(options, path)?;
    }
    let affected: Vec<RepoPath> = entries
        .iter()
        .flat_map(|(e, _)| std::iter::once(e.path.clone()).chain(e.original.clone()))
        .collect();
    let unaffected: Vec<_> = before
        .paths
        .iter()
        .filter(|p| !affected.iter().any(|a| a.bytes == **p))
        .cloned()
        .collect();
    let untouched = super::snapshot::worktree_at(options, &unaffected)?;
    let mut write = options.clone();
    write
        .env
        .insert("GIT_INDEX_FILE".into(), alternate.as_os_str().into());
    write.env.insert("GIT_LITERAL_PATHSPECS".into(), "1".into());
    // Remove new names from the alternate index, preserving their disk contents by default.
    for path in added.iter().chain(&renamed) {
        runner
            .run(
                WriteCommand::new("update-index").args([
                    OsString::from("--force-remove"),
                    "--".into(),
                    path.as_os_str().into(),
                ]),
                &write,
            )
            .await?;
    }
    tx.check_unchanged()?;
    if snapshot(runner, options).await? != before {
        return Err(CommitError::ConcurrentChange);
    }
    // A rename is undone as a move back, independently of the option for NEW files.
    // Remove the destination before restore, including case-only renames on APFS.
    for path in &renamed {
        match std::fs::remove_file(options.cwd.join(path)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    if !tracked.is_empty() {
        if head.is_none() {
            return Err(CommitError::ConcurrentChange);
        }
        let mut args: Vec<OsString> = [
            "--staged",
            "--worktree",
            &format!("--source={}", head.as_deref().unwrap()),
            "--",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        args.extend(tracked.iter().map(|p| p.as_os_str().into()));
        runner
            .run(WriteCommand::new("restore").args(args), &write)
            .await
            .map_err(|e| {
                CommitError::InvalidSelection(format!(
                    "Rollback interrupted; inspect repository status: {e}"
                ))
            })?;
    }
    if delete_added {
        for path in &added {
            match std::fs::remove_file(options.cwd.join(path)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
    let after = snapshot(runner, options).await?;
    let external_new = after
        .paths
        .iter()
        .any(|p| !before.paths.contains(p) && !affected.iter().any(|a| a.bytes == *p));
    if context.head().await? != head
        || super::snapshot::worktree_at(options, &unaffected)? != untouched
        || external_new
    {
        return Err(CommitError::ConcurrentChange);
    }
    tx.install(&alternate)
}
fn validate_path(options: &RunOptions, path: &std::path::Path) -> Result<(), CommitError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(CommitError::InvalidSelection(
            "invalid rollback path".into(),
        ));
    }
    let root = options.cwd.canonicalize()?;
    let mut parent = options.cwd.join(path);
    parent.pop();
    while !parent.exists() {
        if !parent.pop() {
            return Err(CommitError::ConcurrentChange);
        }
    }
    if !parent.canonicalize()?.starts_with(root) {
        return Err(CommitError::InvalidSelection(
            "path escapes repository".into(),
        ));
    }
    Ok(())
}
pub async fn rollback_patch(
    runner: &GitRunner,
    options: &RunOptions,
    patch: &[u8],
    paths: &[PathBuf],
    expected: &str,
) -> Result<(), CommitError> {
    let _guard = tokio::select! { _ = options.cancellation.cancelled() => return Err(GitError::Cancelled.into()), guard = WRITES.lock() => guard };
    let context = Context {
        runner,
        options: options.clone(),
    };
    context.require_root().await?;
    let tx = IndexTransaction::acquire(context.git_path("index").await?)?;
    let before = snapshot(runner, options).await?;
    if before.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    for path in paths {
        validate_path(options, path)?;
    }
    let unaffected: Vec<_> = before
        .paths
        .iter()
        .filter(|p| {
            !paths
                .iter()
                .any(|path| path.as_os_str() == os((*p).clone()))
        })
        .cloned()
        .collect();
    let untouched = super::snapshot::worktree_at(options, &unaffected)?;
    let head = context.head().await?;
    let mut write = options.clone();
    write.stdin = patch.into();
    runner
        .run(
            WriteCommand::new("apply").args(["--reverse", "--check", "--whitespace=nowarn"]),
            &write,
        )
        .await?;
    tx.check_unchanged()?;
    if snapshot(runner, options).await?.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    runner
        .run(
            WriteCommand::new("apply").args(["--reverse", "--whitespace=nowarn"]),
            &write,
        )
        .await?;
    tx.check_unchanged()?;
    let after = snapshot(runner, options).await?;
    if context.head().await? != head
        || super::snapshot::worktree_at(options, &unaffected)? != untouched
        || after.paths.iter().any(|p| !before.paths.contains(p))
    {
        return Err(CommitError::ConcurrentChange);
    }
    Ok(())
}
