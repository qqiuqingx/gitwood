//! Tauri commands for stash management and the Changes context menu.
use crate::{project_windows::WindowSession, repository::Error};
use gitwood_core::repo::{self, RepoPath};
use tauri::AppHandle;

fn write_error(error: impl std::fmt::Display) -> Error {
    Error::from(repo::invalid(&gitwood_core::ops::remote::redact(
        &error.to_string(),
    )))
}

fn refresh(app: &AppHandle, sessions: &WindowSession, generation: u64) {
    let _ = sessions.emit(
        app,
        "repository-refresh",
        (generation, repo::watch::Refresh::all()),
    );
}

#[tauri::command]
pub async fn stash_page(
    sessions: WindowSession,
    generation: u64,
    start: usize,
    end: usize,
) -> Result<gitwood_core::repo::workspace::StashPage, Error> {
    let lease = sessions.lease(generation)?;
    let result = lease.stash_page(start, end).await.map_err(Error::from)?;
    sessions.check(&lease)?;
    Ok(result)
}

#[tauri::command]
pub async fn stash_diff(
    sessions: WindowSession,
    generation: u64,
    entry: gitwood_core::ops::stash::StashEntry,
) -> Result<String, Error> {
    let lease = sessions.lease(generation)?;
    let result = lease.stash_diff(&entry).await.map_err(Error::from)?;
    sessions.check(&lease)?;
    Ok(String::from_utf8_lossy(&result).into_owned())
}

#[tauri::command]
pub async fn stash_write(
    app: AppHandle,
    sessions: WindowSession,
    generation: u64,
    expected: String,
    action: gitwood_core::ops::stash::StashAction,
) -> Result<gitwood_core::ops::stash::StashOutcome, Error> {
    let lease = sessions.lease(generation)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.stash_write(&expected, &action))
    })
    .await
    .map_err(write_error)?
    .map_err(write_error)?;
    refresh(&app, &sessions, generation);
    Ok(result)
}

#[tauri::command]
pub async fn stash_create(
    app: AppHandle,
    sessions: WindowSession,
    generation: u64,
    expected: String,
    message: String,
    keep_index: bool,
    include_untracked: bool,
) -> Result<(), Error> {
    let lease = sessions.lease(generation)?;
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.stash_create(
            &expected,
            &message,
            keep_index,
            include_untracked,
        ))
    })
    .await
    .map_err(write_error)?
    .map_err(write_error)?;
    refresh(&app, &sessions, generation);
    Ok(())
}

#[tauri::command]
pub async fn changes_add(
    app: AppHandle,
    sessions: WindowSession,
    generation: u64,
    paths: Vec<RepoPath>,
    expected: String,
) -> Result<(), Error> {
    let lease = sessions.lease(generation)?;
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.changes_add(&paths, &expected))
    })
    .await
    .map_err(write_error)?
    .map_err(write_error)?;
    refresh(&app, &sessions, generation);
    Ok(())
}

#[tauri::command]
pub async fn changes_delete(
    app: AppHandle,
    sessions: WindowSession,
    generation: u64,
    paths: Vec<RepoPath>,
    expected: String,
) -> Result<(), Error> {
    let lease = sessions.lease(generation)?;
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.changes_delete(&paths, &expected))
    })
    .await
    .map_err(write_error)?
    .map_err(write_error)?;
    refresh(&app, &sessions, generation);
    Ok(())
}
