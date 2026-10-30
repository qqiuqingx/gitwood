//! Optimistic write preconditions: status, HEAD, raw index and changed file bytes.
use crate::{
    git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions},
    repo::{os, status},
};
use std::{
    collections::{hash_map::DefaultHasher, BTreeSet},
    hash::{Hash, Hasher},
    io::Read,
    path::Path,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub fingerprint: String,
    pub worktree: String,
    pub(crate) paths: Vec<Vec<u8>>,
}
pub async fn snapshot(runner: &GitRunner, options: &RunOptions) -> Result<Snapshot, GitError> {
    let raw = runner
        .run(
            ReadCommand::new(ReadOperation::Status).args([
                "--porcelain=v2",
                "-z",
                "--branch",
                "--untracked-files=all",
            ]),
            options,
        )
        .await?
        .stdout;
    let state = status::parse(&raw)?;
    let mut all = DefaultHasher::new();
    // Remote-tracking updates change ahead/behind without changing the local
    // write precondition. HEAD, index and worktree remain fingerprinted.
    for record in raw.split(|b| *b == 0) {
        if !record.starts_with(b"# branch.ab ") && !record.starts_with(b"# branch.upstream ") {
            record.hash(&mut all);
        }
    }
    let mut worktree = DefaultHasher::new();
    let paths: BTreeSet<_> = state
        .staged
        .iter()
        .chain(&state.unstaged)
        .chain(&state.untracked)
        .chain(&state.conflicts)
        .flat_map(|e| {
            std::iter::once(e.path.bytes.clone())
                .chain(e.original.as_ref().map(|p| p.bytes.clone()))
        })
        .collect();
    for path in &paths {
        path.hash(&mut worktree);
        hash_path(
            &options.cwd,
            &options.cwd.join(os(path.clone())),
            &mut worktree,
        )?;
    }
    let worktree = format!("{:016x}", worktree.finish());
    worktree.hash(&mut all);
    let index = runner
        .run(
            ReadCommand::new(ReadOperation::RevParse).args([
                "--path-format=absolute",
                "--git-path",
                "index",
            ]),
            options,
        )
        .await?
        .stdout;
    let mut index = index;
    if index.last() == Some(&b'\n') {
        index.pop();
    }
    hash_optional(&std::path::PathBuf::from(os(index)), &mut all)?;
    Ok(Snapshot {
        fingerprint: format!("{:016x}", all.finish()),
        worktree,
        paths: paths.into_iter().collect(),
    })
}
fn hash_path(root: &Path, path: &Path, hash: &mut DefaultHasher) -> Result<(), GitError> {
    // Resolve the parent, never the leaf: symlink contents are Git's link text.
    if let Some(parent) = path.parent() {
        let mut parent = parent.to_path_buf();
        while !parent.exists() {
            if !parent.pop() {
                return Err(crate::repo::invalid("invalid parent"));
            }
        }
        if !parent.canonicalize()?.starts_with(root.canonicalize()?) {
            return Err(crate::repo::invalid("path escapes worktree"));
        }
    }
    match path.symlink_metadata() {
        Ok(metadata) => {
            metadata.permissions().readonly().hash(hash);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode().hash(hash);
            }
            if metadata.file_type().is_symlink() {
                std::fs::read_link(path)?.hash(hash);
            } else if metadata.is_file() {
                hash_optional(path, hash)?;
            } else {
                return Err(crate::repo::invalid(
                    "write selection contains non-regular file",
                ));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            0u8.hash(hash);
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
fn hash_optional(path: &Path, hash: &mut DefaultHasher) -> Result<(), GitError> {
    match std::fs::File::open(path) {
        Ok(mut file) => {
            1u8.hash(hash);
            let mut buffer = [0; 65536];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hash.write(&buffer[..n]);
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            0u8.hash(hash);
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

pub(crate) fn worktree_at(options: &RunOptions, paths: &[Vec<u8>]) -> Result<String, GitError> {
    let mut hash = DefaultHasher::new();
    for path in paths {
        path.hash(&mut hash);
        hash_path(&options.cwd, &options.cwd.join(os(path.clone())), &mut hash)?;
    }
    Ok(format!("{:016x}", hash.finish()))
}
