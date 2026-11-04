//! Session write boundary: validate selection snapshots, reject concurrent writes,
//! and keep cancellation independent from history/status refresh generations.
use super::{
    inclusion::{self, Hunks, Inclusion, Selection},
    invalid, os,
    preview::Preview,
    session::Lease,
    status, RepoPath,
};
use crate::{
    diff::{self, Content, ContentSource, ReadLimits},
    git::{CancellationToken, GitError, ReadCommand, ReadOperation, RunOptions},
    ops::{self, CommitError, CommitOutcome, CommitSelection},
};
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf, sync::Mutex};
#[derive(Default)]
pub(super) struct Writes {
    active: Mutex<Option<CancellationToken>>,
    push_preview: Mutex<Option<crate::ops::remote::PushPreview>>,
    plans: Mutex<HashMap<Vec<u8>, Plan>>,
}
struct Plan {
    token: String,
    blocks: Vec<diff::ChangeBlock>,
}
pub(super) struct WriteGuard<'a> {
    writes: &'a Writes,
    pub options: RunOptions,
}
impl Drop for WriteGuard<'_> {
    fn drop(&mut self) {
        self.writes.active.lock().unwrap().take();
    }
}
impl Writes {
    fn start(&self, options: &RunOptions) -> Result<WriteGuard<'_>, CommitError> {
        let mut active = self.active.lock().unwrap();
        if active.is_some() {
            return Err(CommitError::InvalidSelection(
                "repository write already running".into(),
            ));
        }
        let mut options = options.clone();
        options.cancellation = CancellationToken::new();
        *active = Some(options.cancellation.clone());
        Ok(WriteGuard {
            writes: self,
            options,
        })
    }
    pub(super) fn cancel(&self) {
        if let Some(token) = self.active.lock().unwrap().as_ref() {
            token.cancel();
        }
    }
    pub(super) fn busy(&self) -> bool {
        self.active.lock().unwrap().is_some()
    }
}
#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ResultState {
    Committed {
        hash: String,
    },
    Recovery {
        hash: String,
        directory: PathBuf,
        instructions: String,
        reason: String,
    },
}
#[derive(Serialize)]
pub struct SelectionPreview {
    #[serde(flatten)]
    pub preview: Preview,
    pub token: String,
    pub selectable: bool,
}
#[derive(Serialize)]
pub struct InclusionSummary {
    pub states: Vec<Inclusion>,
    pub count: usize,
}
impl Lease {
    pub fn cancel_write(&self) {
        self.writes.cancel();
    }
    pub async fn inclusion(
        &self,
        selection: &Selection,
        nodes: &[super::file_tree::FileNode],
    ) -> Result<InclusionSummary, GitError> {
        let data = self.data.lock().await;
        let state = data
            .status
            .as_ref()
            .ok_or_else(|| invalid("repository.statusRequired"))?;
        let entries = inclusion::entries(state);
        Ok(InclusionSummary {
            states: nodes.iter().map(|n| selection.node(n, &entries)).collect(),
            count: entries
                .iter()
                .filter(|(e, u)| selection.file(&e.path, *u) != Inclusion::None)
                .count(),
        })
    }
    async fn check_snapshot(
        &self,
        options: &RunOptions,
        expected: &str,
    ) -> Result<(), CommitError> {
        if self.repository.git_dir.join("index.lock").exists() {
            return Err(CommitError::IndexLocked(
                self.repository.git_dir.join("index.lock"),
            ));
        }
        if ops::snapshot(&self.runner, options).await?.fingerprint != expected {
            return Err(CommitError::ConcurrentChange);
        }
        Ok(())
    }
    pub async fn selection_preview(
        &self,
        path: &RepoPath,
    ) -> Result<SelectionPreview, CommitError> {
        let before = ops::snapshot(&self.runner, &self.options).await?;
        let (entry, _) = self.entry(path).await?;
        let (left, right) = self.selection_content(&entry).await?;
        let result = match (&left, &right) {
            (Content::Text { text: a, .. }, Content::Text { text: b, .. }) => {
                Some(diff::compare(a, b, Default::default()))
            }
            _ => None,
        };
        let selectable = partial_file(&entry) && result.is_some();
        if ops::snapshot(&self.runner, &self.options).await? != before {
            return Err(CommitError::ConcurrentChange);
        }
        if let Some(r) = result.as_ref().filter(|_| selectable) {
            self.writes.plans.lock().unwrap().insert(
                path.bytes.clone(),
                Plan {
                    token: before.fingerprint.clone(),
                    blocks: r.blocks.clone(),
                },
            );
        }
        Ok(SelectionPreview {
            preview: Preview {
                left,
                right,
                result,
                left_label: "HEAD".into(),
                right_label: "Working tree".into(),
                conflicted: entry.kind == 'u',
            },
            token: before.fingerprint,
            selectable,
        })
    }
    async fn entry(&self, path: &RepoPath) -> Result<(status::Entry, bool), CommitError> {
        inclusion::entries(&status::read(&self.runner, &self.options).await?)
            .into_iter()
            .find(|(e, _)| e.path.bytes == path.bytes)
            .ok_or_else(|| CommitError::InvalidSelection("file no longer changed".into()))
    }
    async fn selection_content(
        &self,
        entry: &status::Entry,
    ) -> Result<(Content, Content), CommitError> {
        let p = PathBuf::from(os(entry.path.bytes.clone()));
        let original = PathBuf::from(os(entry
            .original
            .as_ref()
            .unwrap_or(&entry.path)
            .bytes
            .clone()));
        let empty = || diff::classify(vec![], ReadLimits::default());
        let left = if entry.kind == '?' || entry.xy.starts_with('A') {
            empty()
        } else {
            diff::read_content(
                &self.runner,
                &self.options,
                ContentSource::Commit {
                    revision: "HEAD",
                    path: &original,
                },
                ReadLimits::default(),
            )
            .await
            .map_err(|e| CommitError::InvalidSelection(e.to_string()))?
        };
        let right =
            if !self.options.cwd.join(&p).exists() && !self.options.cwd.join(&p).is_symlink() {
                empty()
            } else {
                diff::read_content(
                    &self.runner,
                    &self.options,
                    ContentSource::Worktree { path: &p },
                    ReadLimits::default(),
                )
                .await
                .map_err(|e| CommitError::InvalidSelection(e.to_string()))?
            };
        Ok((left, right))
    }
    async fn selected_patch(
        &self,
        h: &Hunks,
        expected: &str,
        purpose: PatchPurpose,
    ) -> Result<Vec<u8>, CommitError> {
        if h.token != expected {
            return Err(CommitError::ConcurrentChange);
        }
        let blocks = {
            let plans = self.writes.plans.lock().unwrap();
            let plan = plans
                .get(&h.path.bytes)
                .ok_or(CommitError::ConcurrentChange)?;
            if plan.token != h.token {
                return Err(CommitError::ConcurrentChange);
            }
            plan.blocks.clone()
        };
        let (entry, _) = self.entry(&h.path).await?;
        if !partial_file(&entry) {
            return Err(CommitError::InvalidSelection(
                "whole-file selection required".into(),
            ));
        }
        let (Content::Text { text: left, .. }, Content::Text { text: right, .. }) =
            self.selection_content(&entry).await?
        else {
            return Err(CommitError::InvalidSelection("text required".into()));
        };
        let patch = match purpose {
            PatchPurpose::Rollback => {
                diff::unified_patch(&h.path.bytes, &left, &right, &blocks, &h.blocks)
            }
            PatchPurpose::Commit => diff::unified_file_patch(
                diff::PatchFile {
                    old_path: &entry
                        .original
                        .as_ref()
                        .filter(|_| entry.is_rename())
                        .unwrap_or(&entry.path)
                        .bytes,
                    new_path: &entry.path.bytes,
                    old_mode: &entry.metadata[0],
                    new_mode: &entry.metadata[2],
                },
                &left,
                &right,
                &blocks,
                &h.blocks,
            ),
        };
        patch.map_err(|e| CommitError::InvalidSelection(e.to_string()))
    }
    pub async fn commit_selection(
        &self,
        selection: &Selection,
        message: &str,
        amend: bool,
        expected: &str,
    ) -> Result<ResultState, CommitError> {
        let guard = self.writes.start(&self.options)?;
        self.commit_with_guard(selection, message, amend, expected, &guard)
            .await
    }
    async fn commit_with_guard(
        &self,
        selection: &Selection,
        message: &str,
        amend: bool,
        expected: &str,
        guard: &WriteGuard<'_>,
    ) -> Result<ResultState, CommitError> {
        if message.trim().is_empty() {
            return Err(CommitError::InvalidSelection(
                "commit message is empty".into(),
            ));
        }

        self.check_snapshot(&guard.options, expected).await?;
        let entries = inclusion::entries(&status::read(&self.runner, &guard.options).await?);
        let mut selected = CommitSelection {
            expected: Some(expected.into()),
            ..Default::default()
        };
        for (e, u) in entries {
            match selection.file(&e.path, u) {
                Inclusion::None => {}
                Inclusion::Some => {
                    let h = selection
                        .hunks
                        .iter()
                        .find(|h| h.path.bytes == e.path.bytes)
                        .ok_or(CommitError::ConcurrentChange)?;
                    selected.patches.push(
                        self.selected_patch(h, expected, PatchPurpose::Commit)
                            .await?,
                    );
                    if e.is_rename() {
                        if let Some(original) = &e.original {
                            selected.renames.push((
                                PathBuf::from(os(original.bytes.clone())),
                                PathBuf::from(os(e.path.bytes.clone())),
                            ));
                        }
                    }
                }
                Inclusion::All => {
                    if e.kind == 'u' {
                        return Err(CommitError::InvalidSelection(
                            "resolve conflicts first".into(),
                        ));
                    }
                    let renamed = e.is_rename();
                    selected.files.push(PathBuf::from(os(e.path.bytes)));
                    if let Some(old) = e.original.filter(|_| renamed) {
                        selected.files.push(PathBuf::from(os(old.bytes)));
                    }
                }
            }
        }
        if selected.files.is_empty() && selected.patches.iter().all(Vec::is_empty) && !amend {
            return Err(CommitError::InvalidSelection("no changes selected".into()));
        }
        match ops::partial_commit(
            &self.runner,
            &guard.options,
            &selected,
            message.as_bytes(),
            amend,
        )
        .await?
        {
            CommitOutcome::Committed { hash } => Ok(ResultState::Committed { hash }),
            CommitOutcome::CommittedWithIndexRecovery { hash, recovery } => {
                Ok(ResultState::Recovery {
                    hash,
                    directory: recovery.directory,
                    instructions: recovery.instructions,
                    reason: recovery.reason,
                })
            }
        }
    }
    pub async fn last_message(&self) -> Result<String, GitError> {
        let state = status::read(&self.runner, &self.options).await?;
        if state.branch.oid.as_deref() == Some("(initial)") {
            return Err(invalid("cannot amend an unborn HEAD"));
        }
        let out = self
            .runner
            .run(
                ReadCommand::new(ReadOperation::Log).args(["-1", "--format=%B", "HEAD"]),
                &self.options,
            )
            .await?;
        Ok(String::from_utf8_lossy(&out.stdout)
            .trim_end_matches('\n')
            .into())
    }
    pub async fn rollback_selection(
        &self,
        paths: &[RepoPath],
        delete_added: bool,
        expected: &str,
    ) -> Result<(), CommitError> {
        let guard = self.writes.start(&self.options)?;
        self.check_snapshot(&guard.options, expected).await?;
        let entries = inclusion::entries(&status::read(&self.runner, &guard.options).await?);
        let selected: Result<Vec<_>, _> = paths
            .iter()
            .map(|p| {
                entries
                    .iter()
                    .find(|(e, _)| e.path.bytes == p.bytes)
                    .cloned()
                    .ok_or_else(|| CommitError::InvalidSelection("stale rollback path".into()))
            })
            .collect();
        ops::rollback(
            &self.runner,
            &guard.options,
            &selected?,
            delete_added,
            expected,
        )
        .await
    }
    pub async fn rollback_hunk(&self, h: &Hunks) -> Result<(), CommitError> {
        let guard = self.writes.start(&self.options)?;
        self.check_snapshot(&guard.options, &h.token).await?;
        let patch = self
            .selected_patch(h, &h.token, PatchPurpose::Rollback)
            .await?;
        ops::rollback_patch(
            &self.runner,
            &guard.options,
            &patch,
            &[PathBuf::from(os(h.path.bytes.clone()))],
            &h.token,
        )
        .await
    }
}
#[derive(Serialize)]
pub struct RollbackPage {
    pub paths: Vec<RepoPath>,
    pub total: usize,
    pub start: usize,
    pub fingerprint: String,
}
impl Lease {
    pub async fn rollback_files(
        &self,
        ids: &[String],
        start: usize,
        end: usize,
    ) -> Result<RollbackPage, CommitError> {
        if end < start || end - start > 200 {
            return Err(CommitError::InvalidSelection(
                "page exceeds 200 files".into(),
            ));
        }
        let before = ops::snapshot(&self.runner, &self.options).await?;
        let state = status::read(&self.runner, &self.options).await?;
        let paths: Vec<_> = inclusion::entries(&state)
            .into_iter()
            .filter(|(e, u)| ids.iter().any(|id| inclusion::matches(id, &e.path, *u)))
            .map(|(e, _)| {
                let mut path = e.path;
                if let Some(original) = e.original {
                    path.display = format!("{} → {}", original.display, path.display);
                }
                path
            })
            .collect();
        if ops::snapshot(&self.runner, &self.options).await? != before {
            return Err(CommitError::ConcurrentChange);
        }
        Ok(RollbackPage {
            total: paths.len(),
            paths: paths.into_iter().skip(start).take(end - start).collect(),
            start,
            fingerprint: before.fingerprint,
        })
    }
    pub async fn rollback_ids(
        &self,
        ids: &[String],
        delete_added: bool,
        expected: &str,
    ) -> Result<(), CommitError> {
        let paths = inclusion::entries(&status::read(&self.runner, &self.options).await?)
            .into_iter()
            .filter(|(e, u)| ids.iter().any(|id| inclusion::matches(id, &e.path, *u)))
            .map(|(e, _)| e.path)
            .collect::<Vec<_>>();
        self.rollback_selection(&paths, delete_added, expected)
            .await
    }
}
impl Lease {
    pub async fn message_history(&self) -> Result<Vec<String>, GitError> {
        let state = status::read(&self.runner, &self.options).await?;
        if state.branch.oid.as_deref() == Some("(initial)") {
            return Ok(vec![]);
        }
        let out = self
            .runner
            .run(
                ReadCommand::new(ReadOperation::Log).args(["-25", "--format=%B%x00", "HEAD"]),
                &self.options,
            )
            .await?;
        Ok(out
            .stdout
            .split(|b| *b == 0)
            .map(|b| String::from_utf8_lossy(b).trim_matches('\n').to_owned())
            .filter(|s| !s.trim().is_empty())
            .collect())
    }
}

impl Lease {
    pub async fn inclusion_ids(
        &self,
        selection: &Selection,
        ids: &[String],
    ) -> Result<Vec<Inclusion>, GitError> {
        if ids.len() > 200 {
            return Err(invalid("page exceeds 200 ids"));
        }
        let data = self.data.lock().await;
        let entries = inclusion::entries(
            data.status
                .as_ref()
                .ok_or_else(|| invalid("repository.statusRequired"))?,
        );
        Ok(ids.iter().map(|id| selection.id(id, &entries)).collect())
    }
}

#[derive(Clone, Copy)]
enum PatchPurpose {
    Commit,
    Rollback,
}
fn partial_file(entry: &status::Entry) -> bool {
    (entry.kind == '1' || entry.is_rename())
        && !entry.xy.contains(['A', 'D', 'T'])
        && [0, 2].into_iter().all(|i| {
            entry
                .metadata
                .get(i)
                .is_some_and(|mode| mode == "100644" || mode == "100755")
        })
}

impl Lease {
    pub async fn branch_operation(
        &self,
        operation: &crate::ops::branch::BranchOperation,
        expected: &str,
    ) -> Result<crate::ops::branch::BranchOutcome, CommitError> {
        let guard = self.writes.start(&self.options)?;
        Ok(crate::ops::branch::branch(&self.runner, &guard.options, operation, expected).await?)
    }
}

impl Lease {
    pub async fn fetch_remote(
        &self,
        remote: Option<&str>,
        all: bool,
        expected: &str,
        progress: tokio::sync::mpsc::UnboundedSender<String>,
        askpass: Option<std::sync::Arc<crate::git::askpass::Askpass>>,
    ) -> Result<crate::ops::remote::RemoteOutcome, crate::ops::remote::RemoteError> {
        let mut guard = self
            .writes
            .start(&self.options)
            .map_err(|e| crate::repo::invalid(&e.to_string()))?;
        guard.options.progress = Some(progress);
        guard.options.askpass = askpass.clone();
        let result =
            crate::ops::remote::fetch(&self.runner, &guard.options, remote, all, expected).await;
        auth_cancelled(result, askpass.as_deref())
    }
    pub async fn update_project(
        &self,
        expected: &str,
        strategy: Option<crate::ops::remote::UpdateStrategy>,
        progress: tokio::sync::mpsc::UnboundedSender<String>,
        askpass: Option<std::sync::Arc<crate::git::askpass::Askpass>>,
    ) -> Result<crate::ops::remote::RemoteOutcome, crate::ops::remote::RemoteError> {
        let mut guard = self
            .writes
            .start(&self.options)
            .map_err(|e| crate::repo::invalid(&e.to_string()))?;
        guard.options.progress = Some(progress);
        guard.options.askpass = askpass.clone();
        let result =
            crate::ops::remote::update(&self.runner, &guard.options, expected, strategy).await;
        auth_cancelled(result, askpass.as_deref())
    }
    pub async fn default_update_strategy(
        &self,
    ) -> Result<crate::ops::remote::UpdateStrategy, GitError> {
        crate::ops::remote::default_update_strategy(&self.runner, &self.options).await
    }
    pub async fn abort_update(
        &self,
        expected: &str,
        stash: Option<&str>,
        progress: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<crate::ops::remote::RemoteOutcome, crate::ops::remote::RemoteError> {
        // Aborting a merge/rebase and restoring the stash are local: nothing can prompt.
        let mut guard = self
            .writes
            .start(&self.options)
            .map_err(|e| crate::repo::invalid(&e.to_string()))?;
        guard.options.progress = Some(progress);
        crate::ops::remote::abort_update(&self.runner, &guard.options, expected, stash).await
    }
    pub async fn sequence(
        &self,
        expected: &str,
        action: crate::ops::sequence::SequenceAction,
        message: Option<&str>,
        progress: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<crate::ops::sequence::SequenceOutcome, crate::ops::remote::RemoteError> {
        // Local only: continuing runs hooks but never a remote transport.
        let mut guard = self
            .writes
            .start(&self.options)
            .map_err(|e| crate::repo::invalid(&e.to_string()))?;
        guard.options.progress = Some(progress);
        crate::ops::sequence::run(&self.runner, &guard.options, expected, action, message).await
    }
    pub async fn push_preview(
        &self,
        options: &crate::ops::remote::PushOptions,
    ) -> Result<crate::ops::remote::PushPreview, crate::ops::remote::RemoteError> {
        let preview = crate::ops::remote::preview(&self.runner, &self.options, options).await?;
        *self.writes.push_preview.lock().unwrap() = Some(preview);
        self.push_preview_page(0, 200)
    }
    pub fn push_preview_page(
        &self,
        start: usize,
        limit: usize,
    ) -> Result<crate::ops::remote::PushPreview, crate::ops::remote::RemoteError> {
        let cache = self.writes.push_preview.lock().unwrap();
        let preview = cache
            .as_ref()
            .ok_or_else(|| invalid("remote.previewRequired"))?;
        Ok(crate::ops::remote::PushPreview {
            remote: preview.remote.clone(),
            branch: preview.branch.clone(),
            total: preview.total,
            start,
            commits: preview
                .commits
                .iter()
                .skip(start)
                .take(limit.min(200))
                .cloned()
                .collect(),
            subjects: preview
                .subjects
                .iter()
                .skip(start)
                .take(limit.min(200))
                .cloned()
                .collect(),
        })
    }
    pub async fn push_remote(
        &self,
        options: &crate::ops::remote::PushOptions,
        expected: &str,
        progress: tokio::sync::mpsc::UnboundedSender<String>,
        askpass: Option<std::sync::Arc<crate::git::askpass::Askpass>>,
    ) -> Result<crate::ops::remote::RemoteOutcome, crate::ops::remote::RemoteError> {
        let mut guard = self
            .writes
            .start(&self.options)
            .map_err(|e| crate::repo::invalid(&e.to_string()))?;
        guard.options.progress = Some(progress);
        guard.options.askpass = askpass.clone();
        let result =
            crate::ops::remote::push(&self.runner, &guard.options, options, expected).await;
        auth_cancelled(result, askpass.as_deref())
    }
}

/// A cancelled prompt aborts authentication, so it reports that instead of a generic failure.
fn auth_cancelled<T>(
    result: Result<T, crate::ops::remote::RemoteError>,
    askpass: Option<&crate::git::askpass::Askpass>,
) -> Result<T, crate::ops::remote::RemoteError> {
    result.map_err(|mut error| {
        if askpass.is_some_and(|askpass| askpass.cancelled()) {
            error.key = "remote.authCancelled".into();
            error.guidance = "remote.checkCredentials";
        }
        error
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitPushResult {
    pub commit: ResultState,
    pub push: Option<Result<crate::ops::remote::RemoteOutcome, crate::ops::remote::RemoteError>>,
}
impl Lease {
    /// Mirrors the IPC surface of one user action: commit fields plus the push transport.
    #[allow(clippy::too_many_arguments)]
    pub async fn commit_and_push(
        &self,
        selection: &Selection,
        message: &str,
        amend: bool,
        expected: &str,
        options: &crate::ops::remote::PushOptions,
        progress: Option<tokio::sync::mpsc::UnboundedSender<String>>,
        askpass: Option<std::sync::Arc<crate::git::askpass::Askpass>>,
    ) -> Result<CommitPushResult, CommitError> {
        let mut guard = self.writes.start(&self.options)?;
        guard.options.progress = progress;
        let commit = self
            .commit_with_guard(selection, message, amend, expected, &guard)
            .await?;
        let push = if matches!(commit, ResultState::Committed { .. }) {
            // Only the push may prompt; the commit and its hooks ran without the helper.
            let mut transport = guard.options.clone();
            transport.askpass = askpass.clone();
            Some(auth_cancelled(
                match ops::snapshot(&self.runner, &guard.options).await {
                    Ok(snapshot) => {
                        crate::ops::remote::push(
                            &self.runner,
                            &transport,
                            options,
                            &snapshot.fingerprint,
                        )
                        .await
                    }
                    Err(error) => Err(error.into()),
                },
                askpass.as_deref(),
            ))
        } else {
            None
        };
        Ok(CommitPushResult { commit, push })
    }
}

impl Lease {
    pub async fn undo_commit(&self, expected: &str) -> Result<String, CommitError> {
        let guard = self.writes.start(&self.options)?;
        ops::undo_commit(&self.runner, &guard.options, expected).await
    }
    pub async fn operation_state(
        &self,
    ) -> Result<Option<super::operation::Operation>, CommitError> {
        ops::operation_state(&self.runner, &self.options).await
    }
}
