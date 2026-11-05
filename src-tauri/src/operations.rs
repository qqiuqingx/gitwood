use crate::{
    project_windows::WindowSession,
    repository::{branch_write, Error},
};
use tauri::AppHandle;
use tauri::State;

#[tauri::command]
pub async fn history_write(
    app: AppHandle,
    sessions: WindowSession,
    generation: u64,
    expected: String,
    request: gitwood_core::ops::history::HistoryWrite,
    store: State<'_, crate::settings::SettingsStore>,
) -> Result<gitwood_core::ops::history::HistoryOutcome, gitwood_core::ops::remote::RemoteError> {
    let lease = sessions.lease(generation)?;
    let settings = store.snapshot();
    let request = match request {
        gitwood_core::ops::history::HistoryWrite::CherryPick { commits, .. } => {
            gitwood_core::ops::history::HistoryWrite::CherryPick {
                commits,
                record_origin: settings.cherry_pick_add_suffix_to_published_commits,
                empty: settings.empty_cherry_pick,
            }
        }
        request => request,
    };
    let askpass = matches!(
        &request,
        gitwood_core::ops::history::HistoryWrite::PushTag { .. }
            | gitwood_core::ops::history::HistoryWrite::DeleteRemoteTag { .. }
    )
    .then(|| {
        sessions
            .state
            .credentials
            .start(&app, &sessions, generation)
    })
    .flatten();
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.history_write(&request, &expected, askpass))
    })
    .await
    .map_err(|e| gitwood_core::repo::invalid(&e.to_string()))?
    .map_err(|e| gitwood_core::ops::remote::RemoteError {
        key: "history.failed".into(),
        detail: gitwood_core::ops::remote::redact(&e.to_string()),
        guidance: "remote.tryFetchInTerminal",
    })?;
    let _ = sessions.emit(
        &app,
        "repository-refresh",
        (generation, gitwood_core::repo::watch::Refresh::all()),
    );
    Ok(result)
}

#[tauri::command]
pub async fn history_children(
    sessions: WindowSession,
    generation: u64,
    hash: String,
) -> Result<gitwood_core::ops::history::ChildCommits, Error> {
    let lease = sessions.lease(generation)?;
    let result = lease.history_children(&hash).await;
    sessions.check(&lease)?;
    Ok(result?)
}

#[tauri::command]
pub async fn history_revert_has_merge(
    sessions: WindowSession,
    generation: u64,
    hashes: Vec<String>,
) -> Result<bool, Error> {
    let lease = sessions.lease(generation)?;
    let result = lease.history_revert_has_merge(&hashes).await;
    sessions.check(&lease)?;
    Ok(result?)
}
#[tauri::command]
pub async fn set_upstream(
    app: AppHandle,
    sessions: WindowSession,
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
    sessions: WindowSession,
    generation: u64,
    remote: Option<String>,
    all: bool,
    expected: String,
) -> Result<gitwood_core::ops::remote::RemoteOutcome, gitwood_core::ops::remote::RemoteError> {
    let lease = sessions.lease(generation)?;
    let askpass = sessions
        .state
        .credentials
        .start(&app, &sessions, generation);
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    let event_sessions = sessions.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_sessions.emit(&event_app, "remote_progress", (generation, message));
        }
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.fetch_remote(
            remote.as_deref(),
            all,
            &expected,
            progress,
            askpass,
        ))
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
pub async fn update_project(
    app: AppHandle,
    sessions: WindowSession,
    generation: u64,
    expected: String,
    strategy: Option<gitwood_core::ops::remote::UpdateStrategy>,
) -> Result<gitwood_core::ops::remote::RemoteOutcome, gitwood_core::ops::remote::RemoteError> {
    let lease = sessions.lease(generation)?;
    let askpass = sessions
        .state
        .credentials
        .start(&app, &sessions, generation);
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    let event_sessions = sessions.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_sessions.emit(&event_app, "remote_progress", (generation, message));
        }
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.update_project(&expected, strategy, progress, askpass))
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
pub async fn default_update_strategy(
    sessions: WindowSession,
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
    sessions: WindowSession,
    generation: u64,
    expected: String,
    stash: Option<String>,
) -> Result<gitwood_core::ops::remote::RemoteOutcome, gitwood_core::ops::remote::RemoteError> {
    let lease = sessions.lease(generation)?;
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    let event_sessions = sessions.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_sessions.emit(&event_app, "remote_progress", (generation, message));
        }
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.abort_update(&expected, stash.as_deref(), progress))
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
pub async fn push(
    app: AppHandle,
    sessions: WindowSession,
    generation: u64,
    options: gitwood_core::ops::remote::PushOptions,
    expected: String,
) -> Result<gitwood_core::ops::remote::RemoteOutcome, gitwood_core::ops::remote::RemoteError> {
    let lease = sessions.lease(generation)?;
    let askpass = sessions
        .state
        .credentials
        .start(&app, &sessions, generation);
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    let event_sessions = sessions.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_sessions.emit(&event_app, "remote_progress", (generation, message));
        }
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.push_remote(&options, &expected, progress, askpass))
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
pub async fn push_preview(
    sessions: WindowSession,
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
    sessions: WindowSession,
    generation: u64,
    selection: gitwood_core::repo::inclusion::Selection,
    message: String,
    amend: bool,
    expected: String,
    options: gitwood_core::ops::remote::PushOptions,
) -> Result<gitwood_core::repo::write::CommitPushResult, Error> {
    let lease = sessions.lease(generation)?;
    let askpass = sessions
        .state
        .credentials
        .start(&app, &sessions, generation);
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    let event_sessions = sessions.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_sessions.emit(&event_app, "remote_progress", (generation, message));
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
            askpass,
        ))
    })
    .await
    .map_err(|e| Error::from(gitwood_core::repo::invalid(&e.to_string())))?;
    let _ = sessions.emit(
        &app,
        "repository-refresh",
        (generation, gitwood_core::repo::watch::Refresh::all()),
    );
    result.map_err(Into::into)
}

#[tauri::command]
pub async fn undo_commit(
    sessions: WindowSession,
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
pub async fn operation_state(
    sessions: WindowSession,
    generation: u64,
) -> Result<Option<gitwood_core::repo::operation::Operation>, Error> {
    let l = sessions.lease(generation)?;
    let result = l.operation_state().await;
    sessions.check(&l)?;
    Ok(result?)
}

#[tauri::command]
pub async fn sequence(
    app: AppHandle,
    sessions: WindowSession,
    generation: u64,
    expected: String,
    action: gitwood_core::ops::sequence::SequenceAction,
    message: Option<String>,
) -> Result<gitwood_core::ops::sequence::SequenceOutcome, gitwood_core::ops::remote::RemoteError> {
    let lease = sessions.lease(generation)?;
    let (progress, mut events) = gitwood_core::git::progress_channel::<String>();
    let event_app = app.clone();
    let event_sessions = sessions.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(message) = events.recv().await {
            let _ = event_sessions.emit(&event_app, "remote_progress", (generation, message));
        }
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(lease.sequence(
            &expected,
            action,
            message.as_deref(),
            progress,
        ))
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
