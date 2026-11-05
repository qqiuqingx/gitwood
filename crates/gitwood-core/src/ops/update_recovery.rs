//! Worktree-local ownership of the stash saved by an interrupted Update Project.
//! Remove ownership before apply: a crash must never trigger a second stash application.
use crate::{
    git::{GitError, GitRunner, RunOptions},
    repo,
};
use std::path::Path;
const FILE: &str = "gitwood-update-stash";
pub fn pending(git_dir: &Path) -> Result<Option<String>, GitError> {
    match std::fs::read_to_string(git_dir.join(FILE)) {
        Ok(stash)
            if [40, 64].contains(&stash.len()) && stash.bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            Ok(Some(stash))
        }
        Ok(_) => Err(repo::invalid("remote.invalidStash")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub async fn save(r: &GitRunner, o: &RunOptions, stash: &str) -> Result<(), GitError> {
    let repository = repo::Repository::open(r, o).await?;
    std::fs::write(repository.git_dir.join(FILE), stash)?;
    Ok(())
}
pub async fn clear(r: &GitRunner, o: &RunOptions) -> Result<(), GitError> {
    let repository = repo::Repository::open(r, o).await?;
    match std::fs::remove_file(repository.git_dir.join(FILE)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
