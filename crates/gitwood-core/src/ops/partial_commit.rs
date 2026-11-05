use super::WRITES;
use super::{
    context::{as_text, Context},
    index::{align, IndexTransaction},
};
use crate::git::{GitError, GitRunner, ReadOperation, RunOptions};
use std::{
    path::{Component, Path, PathBuf},
    time::Duration,
};

/// Patches are HEAD-relative unified diffs, not index-to-worktree diffs. Include both
/// old and new paths for a whole-file rename. All paths are repository-root-relative.
#[derive(Clone, Debug, Default)]
pub struct CommitSelection {
    pub files: Vec<PathBuf>,
    pub patches: Vec<Vec<u8>>,
    /// Partial-file renames (old, new), already represented in patches. Align
    /// their baseline paths before merging original staged content into new HEAD.
    pub renames: Vec<(PathBuf, PathBuf)>,
    pub expected: Option<String>,
}

/// Both variants represent a completed commit and must never be retried as a failure.
#[derive(Debug)]
pub enum CommitOutcome {
    Committed {
        hash: String,
    },
    CommittedWithIndexRecovery {
        hash: String,
        recovery: IndexRecovery,
    },
}

#[derive(Debug)]
pub struct IndexRecovery {
    pub directory: PathBuf,
    pub index: PathBuf,
    /// UTF-8 recovery instructions, also saved as RECOVERY.md in the directory.
    pub instructions: String,
    pub reason: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error("index is locked: {0:?}")]
    IndexLocked(PathBuf),
    #[error("repository HEAD or index changed externally; refresh and retry")]
    ConcurrentChange,
    #[error("invalid selection: {0}")]
    InvalidSelection(String),
    /// The requested path is no longer among the working tree or index changes,
    /// typically because it was committed or reverted after it was selected.
    #[error("file is no longer changed")]
    NoLongerChanged,
    #[error("selected changes conflict with existing staging: {0}")]
    StagedConflict(String),
    #[error("commit may have completed ({head:?}); real index preserved; inspect HEAD before retrying: {reason}")]
    OutcomeUncertain {
        head: Option<String>,
        reason: String,
    },
    #[error("partial commit I/O: {0}")]
    Io(#[from] std::io::Error),
}

/// Commits only the selection, preserving disjoint prior staging and all working files.
/// `options` supplies repository root, environment, timeout and cancellation. A timeout
/// is per Git invocation; writes queued in this process also respond to cancellation.
pub async fn partial_commit(
    runner: &GitRunner,
    options: &RunOptions,
    selection: &CommitSelection,
    message: &[u8],
    amend: bool,
) -> Result<CommitOutcome, CommitError> {
    validate(selection, options)?;
    let _guard = tokio::select! {
        _ = options.cancellation.cancelled() => return Err(GitError::Cancelled.into()),
        guard = WRITES.lock() => guard,
    };
    let context = Context {
        runner,
        options: options.clone(),
    };
    PreparedCommit::prepare(&context, selection, amend)
        .await?
        .finish(context, message, amend)
        .await
}

struct PreparedCommit {
    tx: IndexTransaction,
    old_head: Option<String>,
    base: String,
    files: Vec<PathBuf>,
    renames: Vec<(PathBuf, PathBuf)>,
    snapshot: super::Snapshot,
}

impl PreparedCommit {
    async fn prepare(
        context: &Context<'_>,
        selection: &CommitSelection,
        amend: bool,
    ) -> Result<Self, CommitError> {
        context.require_root().await?;
        let snapshot = super::snapshot(context.runner, &context.options).await?;
        if selection
            .expected
            .as_ref()
            .is_some_and(|expected| expected != &snapshot.fingerprint)
        {
            return Err(CommitError::ConcurrentChange);
        }
        let ongoing = operation_commit(context, selection, amend).await?;
        let tx = IndexTransaction::acquire(context.git_path("index").await?)?;
        let old_head = context.head().await?;
        if amend && old_head.is_none() {
            return Err(CommitError::InvalidSelection(
                "cannot amend an unborn HEAD".into(),
            ));
        }
        let index = tx.path("selected");
        let base = seed(context, &index, old_head.as_deref(), amend).await?;
        if ongoing {
            tx.copy_original(&index)?;
        }
        select(context, &index, selection).await?;
        let tree = as_text(&context.write("write-tree", &[], &index, &[]).await?)?;
        align(
            context,
            &tx,
            &base,
            &tree,
            &selection.files,
            &selection.renames,
        )
        .await?;
        ensure_current(context, &tx, &old_head).await?;
        Ok(Self {
            tx,
            old_head,
            base,
            files: selection.files.clone(),
            renames: selection.renames.clone(),
            snapshot,
        })
    }

    async fn finish(
        mut self,
        mut context: Context<'_>,
        message: &[u8],
        amend: bool,
    ) -> Result<CommitOutcome, CommitError> {
        let index = self.tx.path("selected");
        let args = if amend {
            vec!["--amend", "-F", "-"]
        } else {
            vec!["-F", "-"]
        };
        if super::snapshot(context.runner, &context.options).await? != self.snapshot {
            return Err(CommitError::ConcurrentChange);
        }
        let committed = context.write("commit", &args, &index, message).await;
        // A late cancellation must not prevent reconciliation of a completed commit.
        context.options.cancellation = Default::default();
        context.options.timeout = Some(Duration::from_secs(30));
        let head = context
            .head()
            .await
            .map_err(|e| CommitError::OutcomeUncertain {
                head: None,
                reason: e.to_string(),
            })?;
        if let Err(error) = committed {
            if head == self.old_head {
                return Err(error);
            }
            return Err(CommitError::OutcomeUncertain {
                head,
                reason: error.to_string(),
            });
        }
        let hash = head.ok_or_else(|| CommitError::OutcomeUncertain {
            head: None,
            reason: "Git returned success without HEAD".into(),
        })?;
        if let Err(reason) = self.install(&context, &hash).await {
            let recovery = self.tx.preserve(&hash, &self.base, reason.to_string());
            return Ok(CommitOutcome::CommittedWithIndexRecovery { hash, recovery });
        }
        Ok(CommitOutcome::Committed { hash })
    }

    async fn install(&mut self, context: &Context<'_>, hash: &str) -> Result<(), CommitError> {
        let after = super::snapshot(context.runner, &context.options).await?;
        if super::snapshot::worktree_at(&context.options, &self.snapshot.paths)?
            != self.snapshot.worktree
            || after.paths.iter().any(|p| !self.snapshot.paths.contains(p))
        {
            return Err(CommitError::ConcurrentChange);
        }
        // Use the committed tree: hooks may have legitimately staged extra changes.
        let tree = as_text(
            &context
                .read(ReadOperation::RevParse, &[&format!("{hash}^{{tree}}")])
                .await?,
        )?;
        let aligned = align(
            context,
            &self.tx,
            &self.base,
            &tree,
            &self.files,
            &self.renames,
        )
        .await?;
        ensure_current(context, &self.tx, &Some(hash.into())).await?;
        self.tx.install(&aligned)
    }
}

fn validate(selection: &CommitSelection, options: &RunOptions) -> Result<(), CommitError> {
    if options
        .env
        .contains_key(std::ffi::OsStr::new("GIT_INDEX_FILE"))
        || std::env::var_os("GIT_INDEX_FILE").is_some()
    {
        return Err(CommitError::InvalidSelection(
            "caller must not override the real index".into(),
        ));
    }
    for path in selection
        .files
        .iter()
        .chain(selection.renames.iter().flat_map(|(old, new)| [old, new]))
    {
        if options
            .cwd
            .join(path)
            .symlink_metadata()
            .is_ok_and(|m| m.is_dir())
            || path.as_os_str().is_empty()
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(CommitError::InvalidSelection(format!(
                "expected relative file path: {path:?}"
            )));
        }
    }
    Ok(())
}

async fn seed(
    context: &Context<'_>,
    index: &Path,
    head: Option<&str>,
    amend: bool,
) -> Result<String, CommitError> {
    context.write("read-tree", &["--empty"], index, &[]).await?;
    let empty = as_text(&context.write("write-tree", &[], index, &[]).await?)?;
    let Some(head) = head else {
        return Ok(empty);
    };
    if amend {
        let parents = context
            .read(
                ReadOperation::RevParse,
                &["--verify", "-q", &format!("{head}^1")],
            )
            .await;
        match parents {
            Ok(parent) => {
                context
                    .write("read-tree", &[as_text(&parent)?.as_str()], index, &[])
                    .await?;
            }
            Err(CommitError::Git(GitError::Exit { code: Some(1), .. })) => {}
            Err(e) => return Err(e),
        }
        // Retain the old commit's other changes; amend replaces its tree, not just
        // the selected paths relative to its parent. This also preserves merge parents.
    }
    context.write("read-tree", &[head], index, &[]).await?;
    Ok(head.into())
}

async fn select(
    context: &Context<'_>,
    index: &Path,
    selection: &CommitSelection,
) -> Result<(), CommitError> {
    for path in &selection.files {
        let mut options = context.options.clone();
        options
            .env
            .insert("GIT_INDEX_FILE".into(), index.as_os_str().into());
        options
            .env
            .insert("GIT_LITERAL_PATHSPECS".into(), "1".into());
        context
            .runner
            .run(
                crate::git::WriteCommand::new("add").args([
                    std::ffi::OsStr::new("-A"),
                    std::ffi::OsStr::new("--"),
                    path.as_os_str(),
                ]),
                &options,
            )
            .await?;
    }
    for patch in &selection.patches {
        context
            .write("apply", &["--cached", "--whitespace=nowarn"], index, patch)
            .await?;
    }
    Ok(())
}

async fn ensure_current(
    context: &Context<'_>,
    tx: &IndexTransaction,
    head: &Option<String>,
) -> Result<(), CommitError> {
    tx.check_unchanged()?;
    if &context.head().await? != head {
        return Err(CommitError::ConcurrentChange);
    }
    Ok(())
}

/// During a merge the index contains already merged changes from both parents. They
/// must all participate, even when the UI only selected the formerly conflicted file.
async fn operation_commit(
    context: &Context<'_>,
    selection: &CommitSelection,
    amend: bool,
) -> Result<bool, CommitError> {
    let status = crate::repo::status::read(context.runner, &context.options).await?;
    if !status.conflicts.is_empty() {
        return Err(CommitError::InvalidSelection(
            "resolve all conflicts first".into(),
        ));
    }
    let operation = crate::repo::operation::read(&context.git_dir().await?)?;
    let Some(operation) = operation.filter(|op| op.blocks_history_edit()) else {
        return Ok(false);
    };
    if amend
        || !matches!(
            operation.kind,
            crate::repo::operation::OperationKind::Merge
                | crate::repo::operation::OperationKind::CherryPick
                | crate::repo::operation::OperationKind::Revert
        )
    {
        return Err(CommitError::InvalidSelection(
            "use Continue for the current operation".into(),
        ));
    }
    if !selection.patches.is_empty()
        || status.staged.iter().any(|e| {
            !selection
                .files
                .contains(&PathBuf::from(crate::repo::os(e.path.bytes.clone())))
        })
    {
        return Err(CommitError::InvalidSelection(
            "select all staged files to complete the operation".into(),
        ));
    }
    Ok(true)
}
