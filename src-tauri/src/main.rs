#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod native_menu;
mod native_window;
mod repository;
mod settings;
use repository::*;

use native_menu::{install_menu, update_menu, MenuLabels};
use native_window::{
    display_work_areas, displays_changed, fit_window, normal_window, remember_window, window_rect,
    Displays,
};
use serde::Serialize;
use settings::{Ratios, Settings, SettingsStore, ThemeChoice};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};

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
    store: State<'_, SettingsStore>,
    labels: MenuLabels,
) -> Result<SettingsResponse, String> {
    install_menu(&app, &store.snapshot(), &labels).map_err(|e| e.to_string())?;
    app.state::<ExitCoordination>()
        .ui_ready
        .store(true, Ordering::SeqCst);
    Ok(SettingsResponse {
        settings: store.snapshot(),
        error: store.error(),
    })
}

#[tauri::command]
fn save_layout(store: State<'_, SettingsStore>, ratios: Ratios) -> Result<(), String> {
    store.update(|s| s.layout.ratios = ratios);
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
    store: State<'_, SettingsStore>,
    panel: ToolWindow,
    visible: bool,
) -> Result<(), String> {
    store.update(|settings| match panel {
        ToolWindow::Commit => settings.layout.commit_visible = visible,
        ToolWindow::Log => settings.layout.log_visible = visible,
    });
    let settings = store.snapshot();
    if let Some(menu) = app.menu() {
        update_menu(menu.items().map_err(|error| error.to_string())?, &settings)
            .map_err(|error| error.to_string())?;
    }
    app.emit("preferences-changed", settings)
        .map_err(|error| error.to_string())
}

#[derive(Default)]
struct ExitCoordination {
    ui_ready: AtomicBool,
    awaiting_ui: AtomicBool,
    authorized: AtomicBool,
}

// Closing first lets the UI drain in-flight ratio IPC, then persists the final snapshot.
#[tauri::command]
fn finish_exit(
    app: AppHandle,
    store: State<'_, SettingsStore>,
    ratios: Option<Ratios>,
) -> Result<(), String> {
    let exit = app.state::<ExitCoordination>();
    if let Some(ratios) = ratios {
        store.update(|s| s.layout.ratios = ratios);
    }
    if let Err(error) = store.flush() {
        exit.awaiting_ui.store(false, Ordering::SeqCst);
        return Err(error);
    }
    exit.authorized.store(true, Ordering::SeqCst);
    app.exit(0);
    Ok(())
}

fn request_exit_snapshot(app: &AppHandle) -> bool {
    let exit = app.state::<ExitCoordination>();
    if !exit.ui_ready.load(Ordering::SeqCst) || exit.authorized.load(Ordering::SeqCst) {
        return false;
    }
    if !exit.awaiting_ui.swap(true, Ordering::SeqCst) {
        if let Err(error) = app.emit("save-before-exit", ()) {
            exit.awaiting_ui.store(false, Ordering::SeqCst);
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
            app.manage(Displays(Mutex::new(display_work_areas(&window)?)));
            app.manage(store);
            app.manage(gitwood_core::repo::session::Sessions::default());
            app.manage(ExitCoordination::default());
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
            if id == "open-project" || id == "close-project" || id.starts_with("recent:") {
                let _ = app.emit("repository-menu", id);
                return;
            }
            match id {
                "commit" => store.update(|s| s.layout.commit_visible = !s.layout.commit_visible),
                "log" => store.update(|s| s.layout.log_visible = !s.layout.log_visible),
                "theme-system" => store.update(|s| s.theme = ThemeChoice::System),
                "theme-light" => store.update(|s| s.theme = ThemeChoice::Light),
                "theme-dark" => store.update(|s| s.theme = ThemeChoice::Dark),
                "theme-walnut" => store.update(|s| s.theme = ThemeChoice::Walnut),
                _ => return,
            }
            let settings = store.snapshot();
            if let Some(menu) = app.menu() {
                if let Err(e) = menu.items().and_then(|items| update_menu(items, &settings)) {
                    eprintln!("Unable to update menu: {e}");
                }
            }
            if let Err(e) = app.emit("preferences-changed", settings) {
                eprintln!("Unable to update preferences: {e}");
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build Gitwood");
    app.run(|app, event| {
        let store = app.state::<SettingsStore>();
        match event {
            RunEvent::WindowEvent { label, event, .. } if label == "main" => {
                if let Some(window) = app.get_webview_window("main") {
                    match event {
                        WindowEvent::Focused(true) | WindowEvent::ScaleFactorChanged { .. } => {
                            if matches!(event, WindowEvent::Focused(true)) {
                                let sessions = app.state::<gitwood_core::repo::session::Sessions>();
                                let _ = app.emit(
                                    "repository-refresh",
                                    (
                                        sessions.generation(),
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
                            if displays_changed(&window, &app.state::<Displays>()).unwrap_or(false)
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
                            if request_exit_snapshot(app) {
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
                if let Some(window) = app.get_webview_window("main") {
                    remember_window(&window, &store);
                }
                if request_exit_snapshot(app) {
                    api.prevent_exit();
                    return;
                }
                if let Err(e) = store.flush() {
                    eprintln!("Unable to save settings before exit: {e}");
                }
            }
            RunEvent::Exit => {
                app.state::<gitwood_core::repo::session::Sessions>().close();
                if let Err(e) = store.flush() {
                    eprintln!("Unable to save settings before exit: {e}");
                }
            }
            _ => {}
        }
    });
}
