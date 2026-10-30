use crate::git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs::File,
    io::Read,
    path::{Component, Path},
};

pub const DEFAULT_MAX_BYTES: u64 = 8 * 1024 * 1024;
#[derive(Clone, Copy, Debug)]
pub struct ReadLimits {
    pub max_bytes: u64,
}
impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

/// Paths are literal, root-relative. Revision is HEAD or a full commit object ID.
#[derive(Clone, Copy, Debug)]
pub enum ContentSource<'a> {
    Commit { revision: &'a str, path: &'a Path },
    Index { path: &'a Path },
    Worktree { path: &'a Path },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Content {
    Text {
        text: String,
        byte_len: u64,
        empty: bool,
        missing_final_newline: bool,
    },
    Binary {
        byte_len: u64,
    },
    Undecodable {
        byte_len: u64,
    },
    TooLarge {
        byte_len: u64,
        limit: u64,
    },
}
#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid diff source: {0}")]
    InvalidSource(String),
}
/// Binary policy: any NUL anywhere in the bounded bytes, before UTF-8 validation.
/// No encoding guessing or Git attributes/textconv. Original bytes/EOL are preserved.
pub fn classify(bytes: Vec<u8>, limits: ReadLimits) -> Content {
    let byte_len = bytes.len() as u64;
    if byte_len > limits.max_bytes {
        return Content::TooLarge {
            byte_len,
            limit: limits.max_bytes,
        };
    }
    if bytes.contains(&0) {
        return Content::Binary { byte_len };
    }
    match String::from_utf8(bytes) {
        Ok(text) => Content::Text {
            empty: text.is_empty(),
            missing_final_newline: !text.is_empty() && !text.ends_with('\n'),
            text,
            byte_len,
        },
        Err(_) => Content::Undecodable { byte_len },
    }
}

pub async fn read_content(
    runner: &GitRunner,
    options: &RunOptions,
    source: ContentSource<'_>,
    limits: ReadLimits,
) -> Result<Content, DiffError> {
    let path = match source {
        ContentSource::Commit { path, .. }
        | ContentSource::Index { path }
        | ContentSource::Worktree { path } => path,
    };
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(DiffError::InvalidSource(
            "expected a root-relative file path".into(),
        ));
    }
    if options.cancellation.is_cancelled() {
        return Err(GitError::Cancelled.into());
    }
    if matches!(source, ContentSource::Worktree { .. }) {
        let root = options.cwd.canonicalize()?;
        let resolved = root.join(path).canonicalize()?;
        if !resolved.starts_with(&root) {
            return Err(DiffError::InvalidSource("file escapes worktree".into()));
        }
        // Reject directories/devices/FIFOs before opening (a FIFO open can block).
        if !std::fs::metadata(&resolved)?.is_file() {
            return Err(DiffError::InvalidSource("expected a regular file".into()));
        }
        let file = File::open(resolved)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(DiffError::InvalidSource("expected a regular file".into()));
        }
        let byte_len = metadata.len();
        if byte_len > limits.max_bytes {
            return Ok(Content::TooLarge {
                byte_len,
                limit: limits.max_bytes,
            });
        }
        // Bound even a file that grows after metadata. Never read an unbounded stream.
        let mut bytes = Vec::new();
        file.take(limits.max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        return Ok(classify(bytes, limits));
    }
    let mut spec = match source {
        ContentSource::Commit { revision, .. } => {
            if revision != "HEAD"
                && !([40, 64].contains(&revision.len())
                    && revision.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                return Err(DiffError::InvalidSource(
                    "revision must be HEAD or a full commit ID".into(),
                ));
            }
            OsString::from(format!("{revision}:"))
        }
        ContentSource::Index { .. } => OsString::from(":0:"),
        ContentSource::Worktree { .. } => unreachable!(),
    };
    spec.push(path);
    // Freeze the blob ID before size/read, so concurrent index changes cannot swap it.
    let output = runner
        .run(
            ReadCommand::new(ReadOperation::RevParse).args([
                OsString::from("--verify"),
                OsString::from("--end-of-options"),
                spec,
            ]),
            options,
        )
        .await?;
    let oid = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if ![40, 64].contains(&oid.len()) || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(DiffError::InvalidSource("expected a blob ID".into()));
    }
    let output = runner
        .run(
            ReadCommand::new(ReadOperation::CatFile).args(["-s", &oid]),
            options,
        )
        .await?;
    let byte_len = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .map_err(|_| DiffError::InvalidSource("invalid blob size".into()))?;
    if byte_len > limits.max_bytes {
        return Ok(Content::TooLarge {
            byte_len,
            limit: limits.max_bytes,
        });
    }
    let bytes = runner
        .run(
            ReadCommand::new(ReadOperation::CatFile).args(["blob", &oid]),
            options,
        )
        .await?
        .stdout;
    Ok(classify(bytes, limits))
}
