use std::{io, path::PathBuf, time::Duration};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Git not found (requested: {0:?}); install Git or select its executable")]
    NotFound(Option<PathBuf>),
    #[error("Git {found} is too old; Git {minimum} or newer is required")]
    VersionTooOld { found: String, minimum: String },
    #[error("unrecognized Git version: {0}")]
    InvalidVersion(String),
    #[error("could not spawn Git at {path:?}: {source}")]
    Spawn { path: PathBuf, source: io::Error },
    #[error("Git exited with {code:?}: {stderr}")]
    Exit { code: Option<i32>, stderr: String },
    #[error("Git rejected repository ownership at {path:?}: {stderr}")]
    UnsafeRepository { path: PathBuf, stderr: String },
    #[error("Git timed out after {0:?}")]
    Timeout(Duration),
    #[error("Git operation cancelled")]
    Cancelled,
    #[error("invalid command: {0}")]
    InvalidCommand(String),
    #[error("Git I/O: {0}")]
    Io(#[from] io::Error),
}
