//! Project windows own independent repository sessions and live layout state.
use crate::{
    credential::Credentials,
    native_window::{fit_window, window_rect, Displays},
    repository::{record_recent, Error},
    settings::{Layout, SettingsStore},
};
use gitwood_core::{
    git::{GitRunner, RunOptions},
    repo::{session::Sessions, Repository},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ops::Deref,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{
    ipc::{CommandArg, CommandItem, InvokeError},
    AppHandle, Emitter, Manager, State, WebviewWindow, WebviewWindowBuilder,
};

pub struct ProjectWindow {
    pub sessions: Sessions,
    pub project: Mutex<Option<PathBuf>>,
    pub layout: Mutex<Layout>,
    pub displays: Displays,
    /// Prompts belong to the window that started the remote operation.
    pub credentials: std::sync::Arc<Credentials>,
    pub ui_ready: AtomicBool,
    pub awaiting_ui: AtomicBool,
    pub authorized_close: AtomicBool,
}
impl ProjectWindow {
    fn new(layout: Layout, project: Option<PathBuf>) -> Self {
        Self {
            sessions: Sessions::default(),
            project: Mutex::new(project),
            layout: Mutex::new(layout),
            displays: Displays::default(),
            credentials: Default::default(),
            ui_ready: AtomicBool::new(false),
            awaiting_ui: AtomicBool::new(false),
            authorized_close: AtomicBool::new(false),
        }
    }
    pub fn needs_snapshot(&self) -> bool {
        self.ui_ready.load(Ordering::SeqCst) && !self.authorized_close.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
pub struct ProjectWindows {
    windows: Mutex<HashMap<String, Arc<ProjectWindow>>>,
    pub focused: Mutex<Option<String>>,
    next_label: AtomicU64,
    pub opening: tauri::async_runtime::Mutex<()>,
    pub quitting: AtomicBool,
}
impl ProjectWindows {
    pub fn insert(&self, label: String, layout: Layout, project: Option<PathBuf>) {
        self.windows
            .lock()
            .unwrap()
            .insert(label, Arc::new(ProjectWindow::new(layout, project)));
    }
    pub fn get(&self, label: &str) -> Result<Arc<ProjectWindow>, String> {
        self.windows
            .lock()
            .unwrap()
            .get(label)
            .cloned()
            .ok_or_else(|| "Project window is closed".into())
    }
    pub fn remove(&self, label: &str) {
        if let Some(state) = self.windows.lock().unwrap().remove(label) {
            state.sessions.close();
        }
    }
    pub fn all(&self) -> Vec<(String, Arc<ProjectWindow>)> {
        self.windows
            .lock()
            .unwrap()
            .iter()
            .map(|(label, state)| (label.clone(), state.clone()))
            .collect()
    }
    fn owner(&self, project: &std::path::Path) -> Option<String> {
        self.all().into_iter().find_map(|(label, state)| {
            (state.project.lock().unwrap().as_deref() == Some(project)).then_some(label)
        })
    }
}

/// The IPC caller determines the session; frontend arguments cannot select another window.
#[derive(Clone)]
pub struct WindowSession {
    pub label: String,
    pub state: Arc<ProjectWindow>,
}
impl Deref for WindowSession {
    type Target = Sessions;
    fn deref(&self) -> &Sessions {
        &self.state.sessions
    }
}
impl<'de> CommandArg<'de, tauri::Wry> for WindowSession {
    fn from_command(command: CommandItem<'de, tauri::Wry>) -> Result<Self, InvokeError> {
        let window = WebviewWindow::from_command(command)?;
        let state = window.state::<ProjectWindows>().get(window.label())?;
        Ok(Self {
            label: window.label().into(),
            state,
        })
    }
}
impl WindowSession {
    pub fn emit<S: Serialize + Clone>(
        &self,
        app: &AppHandle,
        event: &str,
        payload: S,
    ) -> tauri::Result<()> {
        app.emit_to(self.label.as_str(), event, payload)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenTarget {
    Current,
    New,
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ProjectAction {
    Choose { path: PathBuf },
    Current { path: PathBuf },
    Focused,
    Created,
}

#[tauri::command]
pub fn initial_repository(sessions: WindowSession) -> Option<PathBuf> {
    sessions.state.project.lock().unwrap().clone()
}

#[tauri::command]
pub async fn open_project(
    app: AppHandle,
    window: WebviewWindow,
    sessions: WindowSession,
    windows: State<'_, ProjectWindows>,
    store: State<'_, SettingsStore>,
    path: PathBuf,
    target: Option<OpenTarget>,
) -> Result<ProjectAction, Error> {
    // Serialize discovery, duplicate detection and reservation across all windows.
    let _opening = windows.opening.lock().await;
    let runner = GitRunner::discover(None).await?;
    let project = match Repository::open(&runner, &RunOptions::new(&path)).await {
        Ok(project) => project,
        Err(error) => {
            record_recent(&app, &store, path, true);
            return Err(error.into());
        }
    };
    let path = project.root;
    windows
        .get(&sessions.label)
        .map_err(|error| Error::from(gitwood_core::repo::invalid(&error)))?;
    if windows.quitting.load(Ordering::SeqCst) {
        return Err(Error::from(gitwood_core::repo::invalid(
            "Application is closing",
        )));
    }
    let native_error =
        |error: tauri::Error| Error::from(gitwood_core::repo::invalid(&error.to_string()));
    if let Some(owner) = windows.owner(&path) {
        if let Some(existing) = app.get_webview_window(&owner) {
            existing.unminimize().map_err(native_error)?;
            existing.set_focus().map_err(native_error)?;
            return Ok(ProjectAction::Focused);
        }
    }
    let occupied = sessions.state.project.lock().unwrap().is_some();
    if occupied && target.is_none() {
        return Ok(ProjectAction::Choose { path });
    }
    if !occupied || matches!(target, Some(OpenTarget::Current)) {
        *sessions.state.project.lock().unwrap() = Some(path.clone());
        return Ok(ProjectAction::Current { path });
    }
    let label = format!(
        "project-{}",
        windows.next_label.fetch_add(1, Ordering::SeqCst)
    );
    let mut config = app.config().app.windows[0].clone();
    config.label = label.clone();
    config.title = format!("{} — Gitwood", path.display());
    // A new window starts with the source layout, then evolves independently.
    windows.insert(
        label.clone(),
        sessions.state.layout.lock().unwrap().clone(),
        Some(path),
    );
    let result = (|| -> tauri::Result<()> {
        let created = WebviewWindowBuilder::from_config(&app, &config)?.build()?;
        let requested = window_rect(&window)?;
        fit_window(&created, requested)?;
        created.show()?;
        created.set_focus()?;
        Ok(())
    })();
    if let Err(error) = result {
        windows.remove(&label);
        if let Some(created) = app.get_webview_window(&label) {
            let _ = created.destroy();
        }
        return Err(native_error(error));
    }
    Ok(ProjectAction::Created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitwood_core::git::WriteCommand;

    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    async fn repository(name: &str) -> (Fixture, GitRunner, RunOptions) {
        let path = std::env::temp_dir().join(format!(
            "gitwood-project-window-{}-{name}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        let fixture = Fixture(path.clone());
        let runner = GitRunner::discover(None).await.unwrap();
        let mut options = RunOptions::new(&path);
        options.env.insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
        options.env.insert(
            "GIT_CONFIG_GLOBAL".into(),
            if cfg!(windows) { "NUL" } else { "/dev/null" }.into(),
        );
        runner
            .run(
                WriteCommand::new("init").args(["-q", "--template="]),
                &options,
            )
            .await
            .unwrap();
        std::fs::write(path.join("file.txt"), name).unwrap();
        (fixture, runner, options)
    }

    #[test]
    fn closing_or_switching_one_project_preserves_the_other_session_and_layout() {
        tauri::async_runtime::block_on(async {
            let (first, runner_a, options_a) = repository("first").await;
            let (second, runner_b, options_b) = repository("second").await;
            let windows = ProjectWindows::default();
            windows.insert("main".into(), Layout::default(), None);
            windows.insert("project-0".into(), Layout::default(), None);
            let a = windows.get("main").unwrap();
            let b = windows.get("project-0").unwrap();
            let lease_a = a.sessions.open(runner_a.clone(), options_a).await.unwrap();
            let lease_b = b.sessions.open(runner_b, options_b).await.unwrap();
            *a.project.lock().unwrap() = Some(lease_a.repository.root.clone());
            *b.project.lock().unwrap() = Some(lease_b.repository.root.clone());
            assert_eq!(
                windows.owner(&lease_b.repository.root).as_deref(),
                Some("project-0")
            );
            // Aliases to the same worktree identify the existing project window.
            let nested = first.0.join("nested");
            std::fs::create_dir(&nested).unwrap();
            let alias = Repository::open(&runner_a, &RunOptions::new(nested))
                .await
                .unwrap();
            assert_eq!(windows.owner(&alias.root).as_deref(), Some("main"));
            assert_eq!(lease_a.generation, lease_b.generation);
            a.layout.lock().unwrap().commit_visible = false;
            a.layout.lock().unwrap().ratios.commit_diff = 0.4;
            assert!(b.layout.lock().unwrap().commit_visible);
            assert_eq!(b.layout.lock().unwrap().ratios.commit_diff, 0.22);
            a.sessions.close();
            assert!(a.sessions.check(&lease_a).is_err());
            b.sessions.check(&lease_b).unwrap();
            assert_eq!(lease_b.status().await.unwrap().untracked, 1);
            windows.remove("main");
            assert!(windows.get("main").is_err());
            assert_eq!(
                windows.owner(&lease_b.repository.root).as_deref(),
                Some("project-0")
            );
            b.sessions.check(&lease_b).unwrap();
            windows.remove("project-0");
            assert!(b.sessions.check(&lease_b).is_err());
            assert!(windows.all().is_empty());
            drop((first, second));
        });
    }
}
