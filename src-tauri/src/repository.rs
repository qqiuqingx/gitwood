//! Typed, read-only repository IPC. No generic Git execution or filesystem API.
use crate::settings::{RecentRepository, SettingsStore};
use gitwood_core::{
    git::{GitError, GitRunner, RunOptions},
    history::details::{Commit, FileChange, Filter},
    repo::{
        refs::References,
        session::{LayoutInfo, Rows, Sessions, StatusSummary},
        Repository,
    },
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
#[derive(Debug, Serialize)]
pub struct Error {
    generation: Option<u64>,
    key: &'static str,
    path: Option<std::path::PathBuf>,
    detail: String,
}
impl From<GitError> for Error {
    fn from(e: GitError) -> Self {
        let (key, path) = match &e {
            GitError::UnsafeRepository { path, .. } => {
                ("repository.unsafeDirectory", Some(path.clone()))
            }
            GitError::InvalidCommand(key) if key == "repository.stale" => {
                ("repository.stale", None)
            }
            GitError::InvalidCommand(key) if key == "repository.bareUnsupported" => {
                ("repository.bareUnsupported", None)
            }
            _ => ("repository.error", None),
        };
        Self {
            generation: None,
            key,
            path,
            detail: e.to_string(),
        }
    }
}
#[derive(Serialize)]
pub struct Opened {
    generation: u64,
    repository: Repository,
}
#[tauri::command]
pub async fn open_repository(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    store: State<'_, SettingsStore>,
    path: std::path::PathBuf,
) -> Result<Opened, Error> {
    let runner = GitRunner::discover(None).await?;
    let result = sessions.open(runner, RunOptions::new(&path)).await;
    let record = match &result {
        Ok(l) => l.repository.root.clone(),
        Err(_) => path,
    };
    store.update(|s| {
        s.recent_repositories.retain(|r| r.path != record);
        s.recent_repositories.insert(
            0,
            RecentRepository {
                path: record,
                failed: result.is_err(),
            },
        );
        s.recent_repositories.truncate(20);
    });
    let _ = crate::native_menu::refresh_recent_menu(&app, &store.snapshot());
    let l = result?;
    let generation = l.generation;
    if let Err(error) = sessions.watch(generation, move |current, areas| {
        let _ = app.emit("repository-refresh", (current, areas));
    }) {
        store.update(|s| {
            if let Some(record) = s
                .recent_repositories
                .iter_mut()
                .find(|r| r.path == l.repository.root)
            {
                record.failed = true;
            }
        });
        return Err(error.into());
    }
    sessions.check(&l)?;
    Ok(Opened {
        generation,
        repository: l.repository,
    })
}
#[tauri::command]
pub fn recent_repositories(store: State<'_, SettingsStore>) -> Vec<RecentProject> {
    let home = std::env::home_dir();
    store
        .snapshot()
        .recent_repositories
        .into_iter()
        .map(|record| RecentProject {
            display_path: display_path(&record.path, home.as_deref()),
            path: record.path,
            failed: record.failed,
        })
        .collect()
}

/// IPC presentation only: the settings schema still stores path + failed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    path: std::path::PathBuf,
    display_path: String,
    failed: bool,
}

fn display_path(path: &std::path::Path, home: Option<&std::path::Path>) -> String {
    if let Some(relative) = home
        .filter(|home| !home.as_os_str().is_empty())
        .and_then(|home| path.strip_prefix(home).ok())
    {
        if relative.as_os_str().is_empty() {
            return "~".into();
        }
        return std::path::Path::new("~")
            .join(relative)
            .to_string_lossy()
            .into_owned();
    }
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod display_path_tests {
    use super::display_path;
    use std::path::Path;
    #[test]
    fn abbreviates_only_complete_home_path_components() {
        let home = Path::new("/users/me");
        assert_eq!(display_path(home, Some(home)), "~");
        assert_eq!(
            display_path(&home.join("repo"), Some(home)),
            Path::new("~").join("repo").to_string_lossy()
        );
        assert_eq!(
            display_path(Path::new("/users/member/repo"), Some(home)),
            "/users/member/repo"
        );
        assert_eq!(display_path(home, None), "/users/me");
        assert_eq!(display_path(home, Some(Path::new(""))), "/users/me");
    }
}
#[tauri::command]
pub fn close_repository(sessions: State<'_, Sessions>) -> u64 {
    sessions.close()
}
#[tauri::command]
pub async fn repository_status(
    sessions: State<'_, Sessions>,
    generation: u64,
) -> Result<StatusSummary, Error> {
    let l = sessions.lease(generation)?;
    let r = l.status().await;
    sessions.check(&l)?;
    Ok(r?)
}
#[tauri::command]
pub async fn repository_refs(
    sessions: State<'_, Sessions>,
    generation: u64,
) -> Result<References, Error> {
    let l = sessions.lease(generation)?;
    let r = l.refs().await;
    sessions.check(&l)?;
    Ok(r?)
}
#[tauri::command]
pub async fn history_layout(
    sessions: State<'_, Sessions>,
    generation: u64,
    filter: Filter,
) -> Result<LayoutInfo, Error> {
    let generation = sessions.refresh(generation)?;
    let l = sessions.lease(generation)?;
    let result = l.layout(&filter).await;
    sessions.check(&l)?;
    result.map_err(|error| {
        let mut error = Error::from(error);
        error.generation = Some(generation);
        error
    })
}
#[tauri::command]
pub async fn history_rows(
    sessions: State<'_, Sessions>,
    generation: u64,
    start: usize,
    end: usize,
) -> Result<Rows, Error> {
    let l = sessions.lease(generation)?;
    let r = l.rows(start, end).await;
    sessions.check(&l)?;
    Ok(r?)
}
#[tauri::command]
pub async fn commit_details(
    sessions: State<'_, Sessions>,
    generation: u64,
    hash: String,
) -> Result<Commit, Error> {
    let l = sessions.lease(generation)?;
    let r = l.details(&hash).await;
    sessions.check(&l)?;
    Ok(r?)
}
#[derive(Serialize)]
pub struct FilePage {
    total: usize,
    start: usize,
    files: Vec<FileChange>,
}
#[tauri::command]
pub async fn commit_files(
    sessions: State<'_, Sessions>,
    generation: u64,
    hash: String,
    start: Option<usize>,
    end: Option<usize>,
) -> Result<FilePage, Error> {
    let l = sessions.lease(generation)?;
    let start = start.unwrap_or(0);
    let end = end.unwrap_or(start.saturating_add(200));
    if end < start || end - start > 200 {
        return Err(gitwood_core::repo::invalid("page exceeds 200 files").into());
    }
    let r = l.files(&hash).await;
    sessions.check(&l)?;
    let files = r?;
    Ok(FilePage {
        total: files.len(),
        start,
        files: files
            .iter()
            .skip(start)
            .take(end - start)
            .cloned()
            .collect(),
    })
}

#[tauri::command]
pub async fn history_position(
    sessions: State<'_, Sessions>,
    generation: u64,
    hash: String,
) -> Result<Option<u32>, Error> {
    let lease = sessions.lease(generation)?;
    let position = lease.position(&hash).await?;
    sessions.check(&lease)?;
    Ok(position)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeViewport {
    expanded: std::collections::HashSet<String>,
    start: usize,
    end: usize,
    flat: bool,
    position_ids: Vec<String>,
    #[serde(default)]
    selection: gitwood_core::repo::inclusion::Selection,
}
#[tauri::command]
pub async fn commit_file_tree(
    sessions: State<'_, Sessions>,
    generation: u64,
    hash: String,
    viewport: TreeViewport,
) -> Result<gitwood_core::repo::file_tree::FileTreePage, Error> {
    let lease = sessions.lease(generation)?;
    let result = lease
        .file_tree(
            &hash,
            &viewport.expanded,
            viewport.start,
            viewport.end,
            &viewport.position_ids,
        )
        .await;
    sessions.check(&lease)?;
    Ok(result?)
}
#[derive(Serialize)]
pub struct IncludedPage {
    #[serde(flatten)]
    page: gitwood_core::repo::file_tree::FileTreePage,
    inclusion: gitwood_core::repo::write::InclusionSummary,
}
#[tauri::command]
pub async fn repository_changes(
    sessions: State<'_, Sessions>,
    generation: u64,
    viewport: TreeViewport,
) -> Result<IncludedPage, Error> {
    let lease = sessions.lease(generation)?;
    let result = lease
        .changes_tree(
            &viewport.expanded,
            viewport.flat,
            viewport.start,
            viewport.end,
            &viewport.position_ids,
        )
        .await;
    sessions.check(&lease)?;
    let page = result?;
    let nodes: Vec<_> = page.rows.iter().map(|r| r.node.clone()).collect();
    let inclusion = lease.inclusion(&viewport.selection, &nodes).await?;
    Ok(IncludedPage { page, inclusion })
}

#[tauri::command]
pub async fn worktree_diff(
    sessions: State<'_, Sessions>,
    generation: u64,
    path: gitwood_core::repo::RepoPath,
    area: gitwood_core::repo::preview::Area,
) -> Result<gitwood_core::repo::preview::Preview, Error> {
    let l = sessions.lease(generation)?;
    let worker = l.clone();
    let r = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(worker.worktree_diff(&path, area))
    })
    .await
    .map_err(|e| Error::from(gitwood_core::repo::invalid(&e.to_string())))?;
    sessions.check(&l)?;
    Ok(r?)
}

#[tauri::command]
pub async fn commit_diff(
    sessions: State<'_, Sessions>,
    generation: u64,
    hash: String,
    path: gitwood_core::repo::RepoPath,
) -> Result<gitwood_core::repo::preview::Preview, Error> {
    let l = sessions.lease(generation)?;
    let worker = l.clone();
    let r = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(worker.commit_diff(&hash, &path))
    })
    .await
    .map_err(|e| Error::from(gitwood_core::repo::invalid(&e.to_string())))?;
    sessions.check(&l)?;
    Ok(r?)
}

#[tauri::command]
pub async fn containing_branches(
    sessions: State<'_, Sessions>,
    generation: u64,
    hash: String,
) -> Result<gitwood_core::repo::preview::ContainingBranches, Error> {
    let l = sessions.lease(generation)?;
    let r = l.containing_branches(&hash).await;
    sessions.check(&l)?;
    Ok(r?)
}

impl From<gitwood_core::ops::CommitError> for Error {
    fn from(error: gitwood_core::ops::CommitError) -> Self {
        Self {
            generation: None,
            key: "repository.writeError",
            path: None,
            detail: error.to_string(),
        }
    }
}
#[tauri::command]
pub async fn selection_preview(
    sessions: State<'_, Sessions>,
    generation: u64,
    path: gitwood_core::repo::RepoPath,
) -> Result<gitwood_core::repo::write::SelectionPreview, Error> {
    let l = sessions.lease(generation)?;
    let worker = l.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(worker.selection_preview(&path))
    })
    .await
    .map_err(|e| Error::from(gitwood_core::repo::invalid(&e.to_string())))?;
    sessions.check(&l)?;
    Ok(result?)
}
#[tauri::command]
pub async fn commit(
    sessions: State<'_, Sessions>,
    generation: u64,
    selection: gitwood_core::repo::inclusion::Selection,
    message: String,
    amend: bool,
    expected: String,
) -> Result<gitwood_core::repo::write::ResultState, Error> {
    let l = sessions.lease(generation)?;
    // A completed write must be delivered even if reads have since refreshed.
    let worker = l.clone();
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(
            worker.commit_selection(&selection, &message, amend, &expected),
        )
    })
    .await
    .map_err(|e| Error::from(gitwood_core::repo::invalid(&e.to_string())))?
    .map_err(Into::into)
}
#[tauri::command]
pub fn cancel_write(sessions: State<'_, Sessions>, generation: u64) -> Result<(), Error> {
    sessions.lease(generation)?.cancel_write();
    Ok(())
}
#[tauri::command]
pub async fn last_commit_message(
    sessions: State<'_, Sessions>,
    generation: u64,
) -> Result<String, Error> {
    let l = sessions.lease(generation)?;
    let result = l.last_message().await;
    sessions.check(&l)?;
    Ok(result?)
}
#[tauri::command]
pub async fn rollback(
    sessions: State<'_, Sessions>,
    generation: u64,
    ids: Vec<String>,
    delete_added: bool,
    expected: String,
) -> Result<(), Error> {
    let l = sessions.lease(generation)?;
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(l.rollback_ids(&ids, delete_added, &expected))
    })
    .await
    .map_err(|e| Error::from(gitwood_core::repo::invalid(&e.to_string())))?
    .map_err(Into::into)
}
#[tauri::command]
pub async fn rollback_hunk(
    sessions: State<'_, Sessions>,
    generation: u64,
    hunk: gitwood_core::repo::inclusion::Hunks,
) -> Result<(), Error> {
    let l = sessions.lease(generation)?;
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(l.rollback_hunk(&hunk))
    })
    .await
    .map_err(|e| Error::from(gitwood_core::repo::invalid(&e.to_string())))?
    .map_err(Into::into)
}
#[tauri::command]
pub async fn rollback_files(
    sessions: State<'_, Sessions>,
    generation: u64,
    ids: Vec<String>,
    start: usize,
    end: usize,
) -> Result<gitwood_core::repo::write::RollbackPage, Error> {
    let l = sessions.lease(generation)?;
    let result = l.rollback_files(&ids, start, end).await;
    sessions.check(&l)?;
    Ok(result?)
}
#[tauri::command]
pub async fn commit_message_history(
    sessions: State<'_, Sessions>,
    generation: u64,
) -> Result<Vec<String>, Error> {
    let l = sessions.lease(generation)?;
    let result = l.message_history().await;
    sessions.check(&l)?;
    Ok(result?)
}

#[tauri::command]
pub async fn inclusion_states(
    sessions: State<'_, Sessions>,
    generation: u64,
    selection: gitwood_core::repo::inclusion::Selection,
    ids: Vec<String>,
) -> Result<Vec<gitwood_core::repo::inclusion::Inclusion>, Error> {
    let l = sessions.lease(generation)?;
    let result = l.inclusion_ids(&selection, &ids).await;
    sessions.check(&l)?;
    Ok(result?)
}

async fn branch_write(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    operation: gitwood_core::ops::branch::BranchOperation,
    expected: String,
) -> Result<gitwood_core::ops::branch::BranchOutcome, Error> {
    let lease = sessions.lease(generation)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.branch_operation(&operation, &expected))
    })
    .await
    .map_err(|e| Error::from(gitwood_core::repo::invalid(&e.to_string())))?;
    let _ = app.emit(
        "repository-refresh",
        (generation, gitwood_core::repo::watch::Refresh::all()),
    );
    result.map_err(Into::into)
}

#[tauri::command]
pub async fn create_branch(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    name: String,
    start: Option<String>,
    checkout: bool,
    expected: String,
) -> Result<gitwood_core::ops::branch::BranchOutcome, Error> {
    branch_write(
        app,
        sessions,
        generation,
        gitwood_core::ops::branch::BranchOperation::Create {
            name,
            start,
            checkout,
        },
        expected,
    )
    .await
}

#[tauri::command]
pub async fn checkout_branch(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    name: String,
    mode: gitwood_core::ops::branch::CheckoutMode,
    expected: String,
) -> Result<gitwood_core::ops::branch::BranchOutcome, Error> {
    branch_write(
        app,
        sessions,
        generation,
        gitwood_core::ops::branch::BranchOperation::Checkout { name, mode },
        expected,
    )
    .await
}

#[tauri::command]
pub async fn checkout_remote_branch(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    remote: String,
    branch: String,
    mode: gitwood_core::ops::branch::CheckoutMode,
    expected: String,
) -> Result<gitwood_core::ops::branch::BranchOutcome, Error> {
    branch_write(
        app,
        sessions,
        generation,
        gitwood_core::ops::branch::BranchOperation::CheckoutRemote {
            remote,
            branch,
            mode,
        },
        expected,
    )
    .await
}

#[tauri::command]
pub async fn rename_branch(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    name: String,
    new_name: String,
    expected: String,
) -> Result<gitwood_core::ops::branch::BranchOutcome, Error> {
    branch_write(
        app,
        sessions,
        generation,
        gitwood_core::ops::branch::BranchOperation::Rename { name, new_name },
        expected,
    )
    .await
}

#[tauri::command]
pub async fn delete_branch(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    name: String,
    force: bool,
    expected: String,
) -> Result<gitwood_core::ops::branch::BranchOutcome, Error> {
    branch_write(
        app,
        sessions,
        generation,
        gitwood_core::ops::branch::BranchOperation::Delete { name, force },
        expected,
    )
    .await
}

#[tauri::command]
pub async fn set_upstream(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    name: String,
    upstream: Option<String>,
    expected: String,
) -> Result<gitwood_core::ops::branch::BranchOutcome, Error> {
    branch_write(
        app,
        sessions,
        generation,
        gitwood_core::ops::branch::BranchOperation::SetUpstream { name, upstream },
        expected,
    )
    .await
}

#[tauri::command]
pub async fn fetch(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    remote: Option<String>,
    all: bool,
    expected: String,
) -> Result<gitwood_core::ops::remote::RemoteOutcome, gitwood_core::ops::remote::RemoteError> {
    let lease = sessions.lease(generation)?;
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_app.emit("remote_progress", (generation, message));
        }
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.fetch_remote(
            remote.as_deref(),
            all,
            &expected,
            progress,
        ))
    })
    .await
    .map_err(|e| gitwood_core::repo::invalid(&e.to_string()))?;
    let _ = app.emit(
        "repository-refresh",
        (generation, gitwood_core::repo::watch::Refresh::all()),
    );
    result
}

#[tauri::command]
pub async fn update_project(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    expected: String,
    strategy: Option<gitwood_core::ops::remote::UpdateStrategy>,
) -> Result<gitwood_core::ops::remote::RemoteOutcome, gitwood_core::ops::remote::RemoteError> {
    let lease = sessions.lease(generation)?;
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_app.emit("remote_progress", (generation, message));
        }
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.update_project(&expected, strategy, progress))
    })
    .await
    .map_err(|e| gitwood_core::repo::invalid(&e.to_string()))?;
    let _ = app.emit(
        "repository-refresh",
        (generation, gitwood_core::repo::watch::Refresh::all()),
    );
    result
}

#[tauri::command]
pub async fn default_update_strategy(
    sessions: State<'_, Sessions>,
    generation: u64,
) -> Result<gitwood_core::ops::remote::UpdateStrategy, Error> {
    let lease = sessions.lease(generation)?;
    let result = lease.default_update_strategy().await;
    sessions.check(&lease)?;
    Ok(result?)
}

#[tauri::command]
pub async fn abort_update(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    expected: String,
    stash: Option<String>,
) -> Result<gitwood_core::ops::remote::RemoteOutcome, gitwood_core::ops::remote::RemoteError> {
    let lease = sessions.lease(generation)?;
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_app.emit("remote_progress", (generation, message));
        }
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.abort_update(&expected, stash.as_deref(), progress))
    })
    .await
    .map_err(|e| gitwood_core::repo::invalid(&e.to_string()))?;
    let _ = app.emit(
        "repository-refresh",
        (generation, gitwood_core::repo::watch::Refresh::all()),
    );
    result
}

#[tauri::command]
pub async fn push(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    options: gitwood_core::ops::remote::PushOptions,
    expected: String,
) -> Result<gitwood_core::ops::remote::RemoteOutcome, gitwood_core::ops::remote::RemoteError> {
    let lease = sessions.lease(generation)?;
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_app.emit("remote_progress", (generation, message));
        }
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.push_remote(&options, &expected, progress))
    })
    .await
    .map_err(|e| gitwood_core::repo::invalid(&e.to_string()))?;
    let _ = app.emit(
        "repository-refresh",
        (generation, gitwood_core::repo::watch::Refresh::all()),
    );
    result
}

#[tauri::command]
pub async fn push_preview(
    sessions: State<'_, Sessions>,
    generation: u64,
    options: gitwood_core::ops::remote::PushOptions,
    start: Option<usize>,
    limit: Option<usize>,
) -> Result<gitwood_core::ops::remote::PushPreview, gitwood_core::ops::remote::RemoteError> {
    let lease = sessions.lease(generation)?;
    if start.unwrap_or(0) == 0 {
        lease.push_preview(&options).await?;
    }
    let result = lease.push_preview_page(start.unwrap_or(0), limit.unwrap_or(200));
    sessions.check(&lease)?;
    result
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn commit_and_push(
    app: AppHandle,
    sessions: State<'_, Sessions>,
    generation: u64,
    selection: gitwood_core::repo::inclusion::Selection,
    message: String,
    amend: bool,
    expected: String,
    options: gitwood_core::ops::remote::PushOptions,
) -> Result<gitwood_core::repo::write::CommitPushResult, Error> {
    let lease = sessions.lease(generation)?;
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_app.emit("remote_progress", (generation, message));
        }
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.commit_and_push(
            &selection,
            &message,
            amend,
            &expected,
            &options,
            Some(progress),
        ))
    })
    .await
    .map_err(|e| Error::from(gitwood_core::repo::invalid(&e.to_string())))?;
    let _ = app.emit(
        "repository-refresh",
        (generation, gitwood_core::repo::watch::Refresh::all()),
    );
    result.map_err(Into::into)
}

#[tauri::command]
pub async fn undo_commit(
    sessions: State<'_, Sessions>,
    generation: u64,
    expected: String,
) -> Result<String, Error> {
    let l = sessions.lease(generation)?;
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(l.undo_commit(&expected))
    })
    .await
    .map_err(|e| Error::from(gitwood_core::repo::invalid(&e.to_string())))?
    .map_err(Into::into)
}
#[tauri::command]
pub async fn undo_in_progress(
    sessions: State<'_, Sessions>,
    generation: u64,
) -> Result<bool, Error> {
    let l = sessions.lease(generation)?;
    let result = l.undo_in_progress().await;
    sessions.check(&l)?;
    Ok(result?)
}
