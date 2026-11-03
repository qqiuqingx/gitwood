//! Shell-free Git execution. Passive reads and explicit writes have separate policies.
pub mod askpass;
mod command;
mod discovery;
mod error;
mod filters;
mod runner;
mod version;

pub use askpass::{Askpass, Prompt, PromptKind};
pub use command::{GitCommand, ReadCommand, ReadOperation, RunOptions, WriteCommand};
pub use error::GitError;
pub use runner::{GitOutput, GitRunner};
pub use tokio_util::sync::CancellationToken;
pub use version::{GitVersion, MIN_GIT_VERSION};

/// Remove all URL userinfo, including passwords containing punctuation.
pub fn redact(input: &str) -> String {
    let mut result = input.to_owned();
    let mut offset = 0;
    while let Some(relative) = result[offset..].find("://") {
        let start = offset + relative + 3;
        let end = result[start..]
            .find(['/', ' ', '\n', '\r', '\t', '\'', '"'])
            .map_or(result.len(), |n| start + n);
        if let Some(at) = result[start..end].rfind('@') {
            result.replace_range(start..start + at, "[redacted]");
            offset = start + "[redacted]@".len();
        } else {
            offset = end;
        }
    }
    result
}

pub use tokio::sync::mpsc::unbounded_channel as progress_channel;
