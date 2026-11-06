//! Typed hosting and local-file opening commands. WebView never supplies a URL.
use crate::{
    project_windows::WindowSession,
    settings::{HostingHosts, SettingsStore},
    themes::ThemeRegistry,
};
use gitwood_core::{
    hosting::{self, LinkTarget, Platform},
    repo::{refs::References, RepoPath},
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum HostingTarget {
    File {
        revision: String,
        path: RepoPath,
        start: Option<u32>,
        end: Option<u32>,
    },
    Commit {
        revision: String,
    },
    Branch {
        branch: String,
    },
    CreateRequest {
        base: Option<String>,
        head: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostingRequest {
    generation: u64,
    remote: String,
    target: HostingTarget,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedRemote {
    pub name: String,
    pub platform: Platform,
}

async fn validate_published_target(
    lease: &gitwood_core::repo::session::Lease,
    remote: &str,
    target: &HostingTarget,
) -> Result<(), String> {
    let published = match target {
        HostingTarget::File { revision, .. } | HostingTarget::Commit { revision } => {
            lease.hosting_revision_published(remote, revision).await
        }
        HostingTarget::Branch { branch } => lease.hosting_branch_published(remote, branch).await,
        HostingTarget::CreateRequest { base, head } => {
            if !lease.hosting_branch_published(remote, head).await {
                false
            } else if let Some(base) = base {
                lease.hosting_remote_branch_exists(remote, base).await
            } else {
                false
            }
        }
    };
    if published {
        Ok(())
    } else {
        Err("hosting.notPublished".into())
    }
}

#[tauri::command]
pub async fn hosting_remotes(
    sessions: WindowSession,
    store: State<'_, SettingsStore>,
    generation: u64,
) -> Result<Vec<HostedRemote>, String> {
    let lease = sessions
        .lease(generation)
        .map_err(|_| "repository.stale".to_owned())?;
    let refs: References = lease
        .refs()
        .await
        .map_err(|_| "repository.error".to_owned())?;
    let hosts = store.snapshot().hosting_hosts;
    let mut result = Vec::new();
    for remote in refs.remotes.keys() {
        let Ok(url) = lease.hosting_remote_url(remote).await else {
            continue;
        };
        if let Some(parsed) = hosting::parse_remote(&url, &hosts.github, &hosts.gitlab) {
            result.push(HostedRemote {
                name: remote.clone(),
                platform: parsed.platform,
            });
        }
    }
    sessions
        .check(&lease)
        .map_err(|_| "repository.stale".to_owned())?;
    Ok(result)
}

fn link(
    remote_url: &str,
    target: &HostingTarget,
    hosts: &HostingHosts,
) -> Result<(String, Platform), String> {
    let remote = hosting::parse_remote(remote_url, &hosts.github, &hosts.gitlab)
        .ok_or_else(|| "hosting.remoteUnavailable".to_owned())?;
    let target = match target {
        HostingTarget::File {
            revision,
            path,
            start,
            end,
        } => LinkTarget::File {
            revision,
            path: &path.display,
            start: *start,
            end: *end,
        },
        HostingTarget::Commit { revision } => LinkTarget::Commit { revision },
        HostingTarget::Branch { branch } => LinkTarget::Branch { branch },
        HostingTarget::CreateRequest { base, head } => LinkTarget::CreateRequest {
            base: base.as_deref(),
            head,
        },
    };
    let url =
        hosting::build_link(&remote, target).ok_or_else(|| "hosting.invalidTarget".to_owned())?;
    Ok((url, remote.platform))
}

#[tauri::command]
pub async fn create_hosting_link(
    sessions: WindowSession,
    store: State<'_, SettingsStore>,
    request: HostingRequest,
) -> Result<String, String> {
    let lease = sessions
        .lease(request.generation)
        .map_err(|_| "repository.stale".to_owned())?;
    validate_published_target(&lease, &request.remote, &request.target).await?;
    let remote_url = lease
        .hosting_remote_url(&request.remote)
        .await
        .map_err(|_| "hosting.remoteUnavailable".to_owned())?;
    let (url, _) = link(
        &remote_url,
        &request.target,
        &store.snapshot().hosting_hosts,
    )?;
    sessions
        .check(&lease)
        .map_err(|_| "repository.stale".to_owned())?;
    Ok(url)
}

#[tauri::command]
pub async fn open_hosting_link(
    sessions: WindowSession,
    store: State<'_, SettingsStore>,
    request: HostingRequest,
) -> Result<(), String> {
    let lease = sessions
        .lease(request.generation)
        .map_err(|_| "repository.stale".to_owned())?;
    validate_published_target(&lease, &request.remote, &request.target).await?;
    let remote_url = lease
        .hosting_remote_url(&request.remote)
        .await
        .map_err(|_| "hosting.remoteUnavailable".to_owned())?;
    let (url, _) = link(
        &remote_url,
        &request.target,
        &store.snapshot().hosting_hosts,
    )?;
    sessions
        .check(&lease)
        .map_err(|_| "repository.stale".to_owned())?;
    if !url.starts_with("https://") {
        return Err("hosting.invalidTarget".into());
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|_| "hosting.openFailed".to_owned())
}

#[tauri::command]
pub fn update_hosting_hosts(
    app: AppHandle,
    store: State<'_, SettingsStore>,
    hosts: HostingHosts,
) -> Result<(), String> {
    hosts.validate()?;
    store.update(|settings| settings.hosting_hosts = hosts);
    for (label, _) in app.state::<crate::project_windows::ProjectWindows>().all() {
        let _ = app.emit_to(label, "hosting-hosts-changed", ());
    }
    store.save_error().map_or(Ok(()), Err)
}

#[tauri::command]
pub fn get_hosting_hosts(store: State<'_, SettingsStore>) -> HostingHosts {
    store.snapshot().hosting_hosts
}

#[tauri::command]
pub fn open_theme_directory(themes: State<'_, ThemeRegistry>) -> Result<(), String> {
    let directory = themes.directory();
    std::fs::create_dir_all(&directory).map_err(|_| "theme.directoryUnavailable".to_owned())?;
    tauri_plugin_opener::open_path(directory, None::<&str>)
        .map_err(|_| "theme.openDirectoryFailed".to_owned())?;
    Ok(())
}

#[tauri::command]
pub fn reveal_workspace_file(
    sessions: WindowSession,
    generation: u64,
    path: RepoPath,
) -> Result<(), String> {
    let lease = sessions
        .lease(generation)
        .map_err(|_| "repository.stale".to_owned())?;
    #[cfg(unix)]
    let relative = {
        use std::os::unix::ffi::OsStringExt;
        std::path::PathBuf::from(std::ffi::OsString::from_vec(path.bytes))
    };
    #[cfg(not(unix))]
    let relative = std::path::PathBuf::from(path.display);
    let resolved = hosting::resolve_workspace_file(&lease.repository.root, &relative)
        .map_err(str::to_owned)?;
    sessions
        .check(&lease)
        .map_err(|_| "repository.stale".to_owned())?;
    tauri_plugin_opener::reveal_item_in_dir(resolved)
        .map_err(|_| "workspace.revealFailed".to_owned())
}
