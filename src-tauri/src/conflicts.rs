use crate::project_windows::WindowSession;
use gitwood_core::{
    ops::{
        conflict::{MergeContent, Side},
        remote::RemoteError,
        sequence::SequenceAction,
    },
    repo::RepoPath,
};

#[tauri::command]
pub async fn conflict_content(
    sessions: WindowSession,
    generation: u64,
    path: RepoPath,
) -> Result<MergeContent, RemoteError> {
    let lease = sessions.lease(generation)?;
    let result = lease.conflict_content(&path).await;
    sessions.check(&lease)?;
    Ok(result?)
}
#[tauri::command]
pub async fn resolve_conflict(
    sessions: WindowSession,
    generation: u64,
    path: RepoPath,
    expected: String,
    side: Option<Side>,
    text: Option<String>,
) -> Result<(), RemoteError> {
    let lease = sessions.lease(generation)?;
    lease
        .resolve_conflict(&path, &expected, side, text.as_deref())
        .await
        .map_err(|e| RemoteError {
            key: "conflict.failed".into(),
            detail: gitwood_core::git::redact(&e.to_string()),
            guidance: "sequence.inspectOutput",
        })
}
#[tauri::command]
pub async fn continue_update(
    sessions: WindowSession,
    generation: u64,
    expected: String,
    stash: Option<String>,
    action: SequenceAction,
    message: Option<String>,
) -> Result<gitwood_core::ops::remote::RemoteOutcome, RemoteError> {
    let lease = sessions.lease(generation)?;
    lease
        .continue_update(&expected, stash.as_deref(), action, message.as_deref())
        .await
}
