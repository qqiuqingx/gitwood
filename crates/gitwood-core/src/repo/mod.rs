//! Repository discovery, byte-preserving status, references, sessions and event refresh.
pub mod file_tree;
pub mod history_browser;
pub mod operation;
pub mod refs;
pub mod session;
pub mod status;
pub mod watch;
pub mod workspace;
use crate::git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions};
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, path::PathBuf};

pub fn invalid(message: &str) -> GitError {
    GitError::InvalidCommand(message.into())
}
pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
#[cfg(unix)]
pub fn os(bytes: Vec<u8>) -> OsString {
    use std::os::unix::ffi::OsStringExt;
    OsString::from_vec(bytes)
}
#[cfg(not(unix))]
pub fn os(bytes: Vec<u8>) -> OsString {
    text(&bytes).into()
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoPath {
    pub bytes: Vec<u8>,
    pub display: String,
}
pub(crate) fn valid_repo_path(path: &RepoPath) -> bool {
    !path.bytes.is_empty()
        && !path.bytes.starts_with(b"/")
        && path
            .bytes
            .split(|byte| *byte == b'/')
            .all(|part| !part.is_empty() && part != b"." && part != b"..")
}
impl RepoPath {
    pub fn new(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.into(),
            display: text(bytes),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Repository {
    pub root: PathBuf,
    pub git_dir: PathBuf,
    pub common_dir: PathBuf,
    pub linked: bool,
}
impl Repository {
    pub async fn open(runner: &GitRunner, options: &RunOptions) -> Result<Self, GitError> {
        async fn value(r: &GitRunner, o: &RunOptions, flag: &str) -> Result<Vec<u8>, GitError> {
            let mut b = r
                .run(
                    ReadCommand::new(ReadOperation::RevParse)
                        .args(["--path-format=absolute", flag]),
                    o,
                )
                .await?
                .stdout;
            if b.last() == Some(&b'\n') {
                b.pop();
            }
            Ok(b)
        }
        if value(runner, options, "--is-bare-repository").await? == b"true" {
            return Err(invalid("repository.bareUnsupported"));
        }
        let root = PathBuf::from(os(value(runner, options, "--show-toplevel").await?));
        let git_dir = PathBuf::from(os(value(runner, options, "--git-dir").await?));
        let common_dir = PathBuf::from(os(value(runner, options, "--git-common-dir").await?));
        let linked = git_dir != common_dir;
        Ok(Self {
            root,
            git_dir,
            common_dir,
            linked,
        })
    }
}

pub mod preview;

pub mod inclusion;
pub mod write;

mod conflicts;

pub mod annotate;
pub mod comparison;
pub mod file_history;
mod integrate;
