// Copyright 2026 Gitwood contributors. SPDX-License-Identifier: Apache-2.0
//! Relationship reads share GitRunner's asynchronous background-read policy.

use super::{CommitGraph, GraphError};
use crate::git::{GitError, GitRunner, ReadCommand, ReadOperation, RunOptions};

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Parse(#[from] GraphError),
}

/// Read all refs plus HEAD, including detached HEAD, through the hardened runner.
/// An unborn/empty repository returns Git's explicit error rather than fake data.
pub async fn read_log(runner: &GitRunner, options: &RunOptions) -> Result<Vec<u8>, ReadError> {
    Ok(runner
        .run(
            ReadCommand::new(ReadOperation::Log).args([
                "--no-color",
                "--no-decorate",
                "--no-notes",
                "--format=%H %P",
                "--topo-order",
                "--all",
                "HEAD",
                "--",
            ]),
            options,
        )
        .await?
        .stdout)
}

pub async fn read_graph(
    runner: &GitRunner,
    options: &RunOptions,
) -> Result<CommitGraph, ReadError> {
    CommitGraph::parse_log(&read_log(runner, options).await?).map_err(ReadError::Parse)
}
