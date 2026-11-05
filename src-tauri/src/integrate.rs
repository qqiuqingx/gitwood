use crate::project_windows::WindowSession;
use gitwood_core::{
    ops::{
        integrate::Integration,
        remote::{RemoteError, RemoteOutcome},
    },
    repo::{comparison::Summary, file_tree::FileTreePage, preview::Preview, RepoPath},
};
use tauri::AppHandle;

#[tauri::command]
pub async fn history_tree_open(
    sessions: WindowSession,
    generation: u64,
    revision: String,
) -> Result<gitwood_core::repo::history_browser::HistoryTreeSummary, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease.history_tree_open(&revision).await;
    sessions.check(&lease)?;
    Ok(result?)
}

#[tauri::command]
pub async fn history_tree_files(
    sessions: WindowSession,
    generation: u64,
    id: String,
    expanded: Vec<String>,
    start: usize,
    end: usize,
) -> Result<FileTreePage, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease
        .history_tree_files(&id, &expanded.into_iter().collect(), start, end)
        .await;
    sessions.check(&lease)?;
    Ok(result?)
}

#[tauri::command]
pub async fn history_tree_diff(
    sessions: WindowSession,
    generation: u64,
    id: String,
    path: RepoPath,
) -> Result<Preview, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease.history_tree_diff(&id, &path).await;
    sessions.check(&lease)?;
    Ok(result?)
}

#[tauri::command]
pub async fn file_history_open(
    sessions: WindowSession,
    generation: u64,
    path: RepoPath,
    revision: String,
) -> Result<gitwood_core::repo::file_history::FileHistorySummary, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease.file_history_open(&path, &revision).await;
    sessions.check(&lease)?;
    Ok(result?)
}

#[tauri::command]
pub async fn file_history_page(
    sessions: WindowSession,
    generation: u64,
    id: String,
    start: usize,
    end: usize,
) -> Result<gitwood_core::repo::file_history::FileHistoryPage, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease.file_history_page(&id, start, end).await;
    sessions.check(&lease)?;
    Ok(result?)
}

#[tauri::command]
pub async fn annotate_open(
    sessions: WindowSession,
    generation: u64,
    path: RepoPath,
    revision: String,
) -> Result<gitwood_core::repo::annotate::AnnotationSummary, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease.annotate_open(&path, &revision).await;
    sessions.check(&lease)?;
    Ok(result?)
}

#[tauri::command]
pub async fn annotate_page(
    sessions: WindowSession,
    generation: u64,
    id: String,
    start: usize,
    end: usize,
) -> Result<gitwood_core::repo::annotate::AnnotationPage, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease.annotate_page(&id, start, end).await;
    sessions.check(&lease)?;
    Ok(result?)
}

#[tauri::command]
pub async fn annotate_cancel(sessions: WindowSession, generation: u64) -> Result<(), RemoteError> {
    let lease = sessions.lease(generation)?;
    lease.annotate_cancel().await;
    sessions.check(&lease)?;
    Ok(())
}

#[tauri::command]
pub async fn compare_file_with_revision(
    sessions: WindowSession,
    generation: u64,
    path: RepoPath,
    revision: String,
) -> Result<gitwood_core::repo::preview::Preview, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease.compare_file_with_revision(&path, &revision).await;
    sessions.check(&lease)?;
    Ok(result?)
}
#[tauri::command]
pub async fn integrate(
    app: AppHandle,
    sessions: WindowSession,
    generation: u64,
    expected: String,
    request: Integration,
) -> Result<RemoteOutcome, RemoteError> {
    let lease = sessions.lease(generation)?;
    let askpass = if matches!(&request, Integration::Pull { .. }) {
        sessions
            .state
            .credentials
            .start(&app, &sessions, generation)
    } else {
        None
    };
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    let event_sessions = sessions.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_sessions.emit(&event_app, "remote_progress", (generation, message));
        }
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.integrate(&request, &expected, progress, askpass))
    })
    .await
    .map_err(|e| gitwood_core::repo::invalid(&e.to_string()))?;
    let _ = sessions.emit(
        &app,
        "repository-refresh",
        (generation, gitwood_core::repo::watch::Refresh::all()),
    );
    result
}
#[tauri::command]
pub async fn comparison_open(
    sessions: WindowSession,
    generation: u64,
    left: String,
    working: bool,
    right: Option<String>,
) -> Result<Summary, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease
        .comparison_open(&left, working, right.as_deref())
        .await;
    sessions.check(&lease)?;
    Ok(result?)
}
#[tauri::command]
pub async fn comparison_files(
    sessions: WindowSession,
    generation: u64,
    id: String,
    expanded: Vec<String>,
    start: usize,
    end: usize,
) -> Result<FileTreePage, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease
        .comparison_files(&id, &expanded.into_iter().collect(), start, end)
        .await;
    sessions.check(&lease)?;
    Ok(result?)
}
#[tauri::command]
pub async fn comparison_commits(
    sessions: WindowSession,
    generation: u64,
    id: String,
    start: usize,
    end: usize,
) -> Result<gitwood_core::repo::comparison::CommitPage, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease.comparison_commits(&id, start, end).await;
    sessions.check(&lease)?;
    Ok(result?)
}
#[tauri::command]
pub async fn comparison_diff(
    sessions: WindowSession,
    generation: u64,
    id: String,
    path: RepoPath,
) -> Result<Preview, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease.comparison_diff(&id, &path).await;
    sessions.check(&lease)?;
    Ok(result?)
}
