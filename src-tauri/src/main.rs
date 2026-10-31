#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod native_menu;
mod native_window;
mod project_windows;
mod repository;
mod settings;
use project_windows::{initial_repository, open_project, ProjectWindows, WindowSession};
use repository::*;

use native_menu::{install_menu, update_menu, MenuLabels};
use native_window::{
    display_work_areas, displays_changed, fit_window, normal_window, remember_window, window_rect,
};
use serde::Serialize;
use settings::{Ratios, Settings, SettingsStore, ThemeChoice};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewWindow, WindowEvent};

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
fn get_theme(name: &str) -> gitwood_core::theme::Theme {
    gitwood_core::theme::get_theme(name)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsResponse {
    settings: Settings,
    error: Option<String>,
}

#[tauri::command]
fn get_settings(
    app: AppHandle,
    sessions: WindowSession,
    store: State<'_, SettingsStore>,
    labels: MenuLabels,
) -> Result<SettingsResponse, String> {
    let settings = window_settings(&store, &sessions);
    install_menu(&app, &settings, &labels).map_err(|e| e.to_string())?;
    sessions.state.ui_ready.store(true, Ordering::SeqCst);
    Ok(SettingsResponse {
        settings,
        error: store.error(),
    })
}

#[tauri::command]
fn save_layout(
    store: State<'_, SettingsStore>,
    sessions: WindowSession,
    mut ratios: Ratios,
) -> Result<(), String> {
    ratios.normalize();
    sessions.state.layout.lock().unwrap().ratios = ratios;
    store.update(|s| s.layout = sessions.state.layout.lock().unwrap().clone());
    if let Some(error) = store.save_error() {
        return Err(error);
    }
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum ToolWindow {
    Commit,
    Log,
}
#[tauri::command]
fn set_tool_window(
    app: AppHandle,
    sessions: WindowSession,
    store: State<'_, SettingsStore>,
    panel: ToolWindow,
    visible: bool,
) -> Result<(), String> {
    {
        let mut layout = sessions.state.layout.lock().unwrap();
        match panel {
            ToolWindow::Commit => layout.commit_visible = visible,
            ToolWindow::Log => layout.log_visible = visible,
        }
    }
    store.update(|s| s.layout = sessions.state.layout.lock().unwrap().clone());
    let settings = window_settings(&store, &sessions);
    if let Some(menu) = app.menu() {
        update_menu(menu.items().map_err(|error| error.to_string())?, &settings)
            .map_err(|error| error.to_string())?;
    }
    sessions
        .emit(&app, "preferences-changed", settings)
        .map_err(|error| error.to_string())
}

fn window_settings(store: &SettingsStore, sessions: &WindowSession) -> Settings {
    let mut settings = store.snapshot();
    settings.layout = sessions.state.layout.lock().unwrap().clone();
    settings
}

fn active_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.webview_windows()
        .into_values()
        .find(|window| window.is_focused().unwrap_or(false))
        .or_else(|| {
            app.state::<ProjectWindows>()
                .focused
                .lock()
                .unwrap()
                .as_deref()
                .and_then(|label| app.get_webview_window(label))
        })
}

fn refresh_active_menu(app: &AppHandle) {
    if let Some(window) = active_window(app) {
        if let Ok(state) = app.state::<ProjectWindows>().get(window.label()) {
            let sessions = WindowSession {
                label: window.label().into(),
                state,
            };
            if let Some(menu) = app.menu() {
                if let Err(error) = menu.items().and_then(|items| {
                    update_menu(
                        items,
                        &window_settings(&app.state::<SettingsStore>(), &sessions),
                    )
                }) {
                    eprintln!("Unable to update menu: {error}");
                }
            }
        }
    }
}

// Closing first lets the UI drain in-flight ratio IPC, then persists the final snapshot.
#[tauri::command]
fn finish_exit(
    app: AppHandle,
    window: WebviewWindow,
    sessions: WindowSession,
    store: State<'_, SettingsStore>,
    ratios: Option<Ratios>,
) -> Result<(), String> {
    let saved = (|| {
        if let Some(ratios) = ratios {
            save_layout(store.clone(), sessions.clone(), ratios)?;
        }
        store.flush()
    })();
    if let Err(error) = saved {
        sessions.state.awaiting_ui.store(false, Ordering::SeqCst);
        return Err(error);
    }
    sessions
        .state
        .authorized_close
        .store(true, Ordering::SeqCst);
    let windows = app.state::<ProjectWindows>();
    if windows.quitting.load(Ordering::SeqCst) {
        if windows
            .all()
            .iter()
            .all(|(_, state)| !state.needs_snapshot())
        {
            app.exit(0);
        }
    } else {
        if let Err(error) = window.close() {
            sessions
                .state
                .authorized_close
                .store(false, Ordering::SeqCst);
            sessions.state.awaiting_ui.store(false, Ordering::SeqCst);
            return Err(error.to_string());
        }
    }
    Ok(())
}

fn request_exit_snapshot(app: &AppHandle, label: &str) -> bool {
    let Ok(state) = app.state::<ProjectWindows>().get(label) else {
        return false;
    };
    if !state.needs_snapshot() {
        return false;
    }
    if !state.awaiting_ui.swap(true, Ordering::SeqCst) {
        if let Err(error) = app.emit_to(label, "save-before-exit", ()) {
            state.awaiting_ui.store(false, Ordering::SeqCst);
            eprintln!("Unable to request final layout: {error}");
        }
    }
    true
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let store = SettingsStore::open(
                app.path().app_config_dir()?.join("settings.json"),
                move |error| {
                    eprintln!("Unable to save Gitwood settings: {error}");
                    let _ = handle.emit("settings-error", error);
                },
            );
            let window = app
                .get_webview_window("main")
                .expect("configured main window");
            let requested = store.snapshot().window.unwrap_or(window_rect(&window)?);
            fit_window(&window, requested)?;
            remember_window(&window, &store);
            let windows = ProjectWindows::default();
            windows.insert("main".into(), store.snapshot().layout, None);
            *windows
                .get("main")
                .expect("registered main window")
                .displays
                .0
                .lock()
                .unwrap() = display_work_areas(&window)?;
            app.manage(store);
            app.manage(windows);
            window.show()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            get_theme,
            get_settings,
            save_layout,
            set_tool_window,
            finish_exit,
            initial_repository,
            open_project,
            open_repository,
            recent_repositories,
            close_repository,
            repository_status,
            repository_changes,
            repository_refs,
            history_layout,
            history_rows,
            history_position,
            commit_details,
            commit_files,
            commit_file_tree,
            worktree_diff,
            commit_diff,
            containing_branches,
            selection_preview,
            inclusion_states,
            commit,
            cancel_write,
            last_commit_message,
            commit_message_history,
            rollback,
            undo_commit,
            undo_in_progress,
            rollback_hunk,
            rollback_files,
            create_branch,
            checkout_branch,
            checkout_remote_branch,
            rename_branch,
            delete_branch,
            set_upstream,
            fetch,
            update_project,
            default_update_strategy,
            abort_update,
            push,
            push_preview,
            commit_and_push,
        ])
        .on_menu_event(|app, event| {
            let store = app.state::<SettingsStore>();
            let id = event.id().as_ref();
            let window = active_window(app);
            if matches!(
                id,
                "open-project"
                    | "close-project"
                    | "update-project"
                    | "commit-project"
                    | "push-project"
            ) || id.starts_with("recent:")
            {
                if let Some(window) = window {
                    let _ = app.emit_to(window.label(), "repository-menu", id);
                }
                return;
            }
            if matches!(id, "commit" | "log") {
                if let Some(window) = window {
                    if let Ok(state) = app.state::<ProjectWindows>().get(window.label()) {
                        let sessions = WindowSession {
                            label: window.label().into(),
                            state,
                        };
                        let visible = {
                            let layout = sessions.state.layout.lock().unwrap();
                            if id == "commit" {
                                !layout.commit_visible
                            } else {
                                !layout.log_visible
                            }
                        };
                        let panel = if id == "commit" {
                            ToolWindow::Commit
                        } else {
                            ToolWindow::Log
                        };
                        if let Err(error) =
                            set_tool_window(app.clone(), sessions, store, panel, visible)
                        {
                            eprintln!("Unable to update tool window: {error}");
                        }
                    }
                }
                return;
            }
            match id {
                "theme-system" => store.update(|s| s.theme = ThemeChoice::System),
                "theme-light" => store.update(|s| s.theme = ThemeChoice::Light),
                "theme-dark" => store.update(|s| s.theme = ThemeChoice::Dark),
                "theme-walnut" => store.update(|s| s.theme = ThemeChoice::Walnut),
                _ => return,
            }
            // Theme is application-wide; each recipient retains its own layout.
            for (label, state) in app.state::<ProjectWindows>().all() {
                let sessions = WindowSession { label, state };
                if let Err(error) = sessions.emit(
                    app,
                    "preferences-changed",
                    window_settings(&store, &sessions),
                ) {
                    eprintln!("Unable to update preferences: {error}");
                }
            }
            refresh_active_menu(app);
        })
        .build(tauri::generate_context!())
        .expect("failed to build Gitwood");
    app.run(|app, event| {
        let store = app.state::<SettingsStore>();
        match event {
            RunEvent::WindowEvent { label, event, .. } => {
                let windows = app.state::<ProjectWindows>();
                if matches!(event, WindowEvent::Destroyed) {
                    windows.remove(&label);
                    return;
                }
                if let (Some(window), Ok(state)) =
                    (app.get_webview_window(&label), windows.get(&label))
                {
                    match event {
                        WindowEvent::Focused(true) | WindowEvent::ScaleFactorChanged { .. } => {
                            if matches!(event, WindowEvent::Focused(true)) {
                                *windows.focused.lock().unwrap() = Some(label.clone());
                                refresh_active_menu(app);
                                let _ = app.emit_to(
                                    label.as_str(),
                                    "repository-refresh",
                                    (
                                        state.sessions.generation(),
                                        gitwood_core::repo::watch::Refresh::all(),
                                    ),
                                );
                            }
                            if normal_window(&window) {
                                if let Ok(rect) = window_rect(&window) {
                                    if let Err(e) = fit_window(&window, rect) {
                                        eprintln!("Unable to fit window: {e}");
                                    }
                                }
                            }
                            remember_window(&window, &store);
                        }
                        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                            if displays_changed(&window, &state.displays).unwrap_or(false)
                                && normal_window(&window)
                            {
                                if let Ok(rect) = window_rect(&window) {
                                    if let Err(e) = fit_window(&window, rect) {
                                        eprintln!("Unable to fit changed display: {e}");
                                    }
                                }
                            }
                            remember_window(&window, &store);
                        }
                        WindowEvent::CloseRequested { api, .. } => {
                            remember_window(&window, &store);
                            if request_exit_snapshot(app, &label) {
                                api.prevent_close();
                                return;
                            }
                            if let Err(e) = store.flush() {
                                eprintln!("Unable to save settings before close: {e}");
                            }
                        }
                        _ => {}
                    }
                }
            }
            RunEvent::ExitRequested { api, .. } => {
                let windows = app.state::<ProjectWindows>();
                windows.quitting.store(true, Ordering::SeqCst);
                let mut waiting = false;
                for (label, _) in windows.all() {
                    if let Some(window) = app.get_webview_window(&label) {
                        remember_window(&window, &store);
                        waiting |= request_exit_snapshot(app, &label);
                    }
                }
                if waiting {
                    api.prevent_exit();
                    return;
                }
                if let Err(e) = store.flush() {
                    eprintln!("Unable to save settings before exit: {e}");
                }
            }
            RunEvent::Exit => {
                for (label, _) in app.state::<ProjectWindows>().all() {
                    app.state::<ProjectWindows>().remove(&label);
                }
                if let Err(e) = store.flush() {
                    eprintln!("Unable to save settings before exit: {e}");
                }
            }
            _ => {}
        }
    });
}
