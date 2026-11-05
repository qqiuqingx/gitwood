//! File-scoped actions from the Changes tree.
use super::{snapshot, CommitError, WRITES};
use crate::{
    git::{GitError, GitRunner, RunOptions, WriteCommand},
    repo::{os, status, RepoPath},
};
use std::{
    ffi::OsString,
    path::{Component, PathBuf},
};

fn validate_path(options: &RunOptions, path: &PathBuf) -> Result<(), CommitError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(CommitError::InvalidSelection("invalid Changes path".into()));
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
            "path escapes worktree".into(),
        ));
    }
    Ok(())
}

pub async fn add(
    runner: &GitRunner,
    options: &RunOptions,
    paths: &[RepoPath],
    expected: &str,
) -> Result<(), CommitError> {
    let _guard = tokio::select! { _ = options.cancellation.cancelled() => return Err(GitError::Cancelled.into()), guard = WRITES.lock() => guard };
    if paths.is_empty() {
        return Err(CommitError::InvalidSelection("no files selected".into()));
    }
    let before = snapshot(runner, options).await?;
    if before.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    let state = status::read(runner, options).await?;
    let mut args: Vec<OsString> = vec!["--".into()];
    for selected in paths {
        let entry = state
            .untracked
            .iter()
            .find(|entry| entry.path.bytes == selected.bytes)
            .ok_or_else(|| {
                CommitError::InvalidSelection(
                    "Add to VCS is only available for untracked files".into(),
                )
            })?;
        let path = PathBuf::from(os(entry.path.bytes.clone()));
        validate_path(options, &path)?;
        if snapshot(runner, options).await?.fingerprint != expected {
            return Err(CommitError::ConcurrentChange);
        }
        args.push(path.as_os_str().into());
    }
    let mut write = options.clone();
    write.env.insert("GIT_LITERAL_PATHSPECS".into(), "1".into());
    runner
        .run(WriteCommand::new("add").args(args), &write)
        .await?;
    Ok(())
}

pub async fn delete(
    runner: &GitRunner,
    options: &RunOptions,
    paths: &[RepoPath],
    expected: &str,
) -> Result<(), CommitError> {
    let _guard = tokio::select! { _ = options.cancellation.cancelled() => return Err(GitError::Cancelled.into()), guard = WRITES.lock() => guard };
    if paths.is_empty() {
        return Err(CommitError::InvalidSelection("no files selected".into()));
    }
    let before = snapshot(runner, options).await?;
    if before.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    let state = status::read(runner, options).await?;
    let mut files: Vec<PathBuf> = Vec::with_capacity(paths.len());
    for selected in paths {
        let known = state
            .staged
            .iter()
            .chain(&state.unstaged)
            .chain(&state.untracked)
            .chain(&state.conflicts)
            .any(|entry| entry.path.bytes == selected.bytes);
        if !known {
            return Err(CommitError::InvalidSelection("stale Changes path".into()));
        }
        let path = PathBuf::from(os(selected.bytes.clone()));
        validate_path(options, &path)?;
        let absolute = options.cwd.join(&path);
        let metadata = match std::fs::symlink_metadata(&absolute) {
            Ok(value) => Some(value),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if metadata.is_some_and(|metadata| metadata.is_dir()) {
            return Err(CommitError::InvalidSelection(
                "cannot delete a directory from Changes".into(),
            ));
        }
        files.push(absolute);
    }
    if snapshot(runner, options).await?.fingerprint != expected {
        return Err(CommitError::ConcurrentChange);
    }
    for path in files {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
