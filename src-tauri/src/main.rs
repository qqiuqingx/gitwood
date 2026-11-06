#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod integrate;
mod stash_changes;
use integrate::*;
use stash_changes::*;
mod conflicts;
mod credential;
mod hosting;
mod native_git_menu;
mod native_menu;
mod native_menu_state;
mod native_window;
mod operations;
mod project_windows;
mod repository;
mod themes;
use conflicts::*;
use operations::*;
mod settings;
use project_windows::{initial_repository, open_project, ProjectWindows, WindowSession};
use repository::*;

use native_menu::{install_menu, update_menu, MenuLabels};
use native_window::{
    display_work_areas, displays_changed, fit_window, normal_window, remember_window, window_rect,
};
use serde::Serialize;
use settings::{Ratios, Settings, SettingsStore, ThemeChoice};
use std::sync::{atomic::Ordering, Mutex};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewWindow, WindowEvent};
use themes::{ThemeCatalogResponse, ThemeRegistry};

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
fn get_theme(
    name: &str,
    system_dark: bool,
    themes: State<'_, ThemeRegistry>,
) -> gitwood_core::theme::Theme {
    themes.theme(name, system_dark)
}

#[tauri::command]
fn theme_catalog(themes: State<'_, ThemeRegistry>) -> ThemeCatalogResponse {
    themes.response()
}

#[derive(Default)]
struct MenuLabelsStore(Mutex<Option<MenuLabels>>);

fn publish_preferences(app: &AppHandle, store: &SettingsStore) {
    for (label, state) in app.state::<ProjectWindows>().all() {
        let sessions = WindowSession { label, state };
        if let Err(error) = sessions.emit(
            app,
            "preferences-changed",
            window_settings(store, &sessions),
        ) {
            eprintln!("Unable to update preferences: {error}");
        }
    }
}

fn refresh_theme_menu(app: &AppHandle, store: &SettingsStore, themes: &ThemeRegistry) {
    let labels = app.state::<MenuLabelsStore>().0.lock().unwrap().clone();
    if let Some(labels) = labels {
        if let Err(error) = native_menu::refresh_theme_menu(app, &store.snapshot(), &labels, themes)
        {
            eprintln!("Unable to update theme menu: {error}");
        }
    }
}

fn apply_theme_choice(
    app: &AppHandle,
    store: &SettingsStore,
    themes: &ThemeRegistry,
    choice: ThemeChoice,
) -> Result<(), String> {
    if choice.is_user() && !themes.has_user(&choice.id()) {
        return Err("The selected user theme is unavailable".into());
    }
    store.update(|settings| settings.theme = choice);
    publish_preferences(app, store);
    refresh_theme_menu(app, store, themes);
    refresh_active_menu(app);
    Ok(())
}

fn theme_catalog_changed(app: &AppHandle, themes: &ThemeRegistry, store: &SettingsStore) {
    themes.refresh();
    refresh_theme_menu(app, store, themes);
    for (label, state) in app.state::<ProjectWindows>().all() {
        let sessions = WindowSession { label, state };
        if let Err(error) = sessions.emit(app, "theme-catalog-changed", ()) {
            eprintln!("Unable to update theme list: {error}");
        }
    }
    refresh_active_menu(app);
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
    themes: State<'_, ThemeRegistry>,
    label_store: State<'_, MenuLabelsStore>,
) -> Result<SettingsResponse, String> {
    let settings = window_settings(&store, &sessions);
    *label_store.0.lock().unwrap() = Some(labels.clone());
    install_menu(&app, &settings, &labels, &themes).map_err(|e| e.to_string())?;
    sessions.state.ui_ready.store(true, Ordering::SeqCst);
    Ok(SettingsResponse {
        settings,
        error: store.error(),
    })
}

#[tauri::command]
fn set_theme(
    app: AppHandle,
    store: State<'_, SettingsStore>,
    themes: State<'_, ThemeRegistry>,
    name: String,
) -> Result<(), String> {
    let choice = ThemeChoice::from_id(&name).ok_or_else(|| "Unknown theme choice".to_owned())?;
    apply_theme_choice(&app, &store, &themes, choice)
}

/// Layout the UI owns: split ratios and the Commit panel view toggles. Tool window
/// visibility is excluded because it changes through the native menu and `set_tool_window`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayoutChange {
    ratios: Ratios,
    commit_flat: bool,
    commit_preview: bool,
}

#[tauri::command]
fn save_layout(
    store: State<'_, SettingsStore>,
    sessions: WindowSession,
    layout: LayoutChange,
) -> Result<(), String> {
    let LayoutChange {
        mut ratios,
        commit_flat,
        commit_preview,
    } = layout;
    ratios.normalize();
    {
        let mut current = sessions.state.layout.lock().unwrap();
        current.ratios = ratios;
        current.commit_flat = commit_flat;
        current.commit_preview = commit_preview;
    }
    store.update(|s| s.layout = sessions.state.layout.lock().unwrap().clone());
    if let Some(error) = store.save_error() {
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
fn save_history_preferences(
    app: AppHandle,
    sessions: WindowSession,
    store: State<'_, SettingsStore>,
    add_suffix_to_published: bool,
    empty_cherry_pick: gitwood_core::ops::history::EmptyCherryPick,
) -> Result<(), String> {
    store.update(|settings| {
        settings.cherry_pick_add_suffix_to_published_commits = add_suffix_to_published;
        settings.empty_cherry_pick = empty_cherry_pick;
    });
    if let Some(error) = store.save_error() {
        return Err(error);
    }
    let _ = sessions.emit(&app, "preferences-changed", store.snapshot());
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
    refresh_active_menu(&app);
    let settings = window_settings(&store, &sessions);
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

pub(crate) fn refresh_active_menu(app: &AppHandle) {
    if let Some(window) = active_window(app) {
        if let Ok(state) = app.state::<ProjectWindows>().get(window.label()) {
            let sessions = WindowSession {
                label: window.label().into(),
                state: state.clone(),
            };
            if let Some(menu) = app.menu() {
                let native = state.native_menu.lock().unwrap().presentation();
                if let Err(error) = menu.items().and_then(|items| {
                    update_menu(
                        items,
                        &window_settings(&app.state::<SettingsStore>(), &sessions),
                        &native,
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
    layout: Option<LayoutChange>,
) -> Result<(), String> {
    let saved = (|| {
        if let Some(layout) = layout {
            save_layout(store.clone(), sessions.clone(), layout)?;
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
    // Git and SSH start this executable as their askpass program for one prompt; that run
    // must not open a window or touch the application state.
    if let Some(code) = gitwood_core::git::askpass::run_as_helper() {
        std::process::exit(code);
    }
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
            let theme_directory =
                themes::user_theme_directory(app.handle()).map_err(std::io::Error::other)?;
            let theme_registry = ThemeRegistry::open(theme_directory.clone());
            app.manage(theme_registry.clone());
            app.manage(MenuLabelsStore::default());
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
            let watcher_app = app.handle().clone();
            let watcher_themes = theme_registry.clone();
            let watcher_store = store.clone();
            app.manage(store);
            app.manage(windows);
            match gitwood_core::theme::ThemeWatch::start(theme_directory, move || {
                theme_catalog_changed(&watcher_app, &watcher_themes, &watcher_store);
            }) {
                Ok(watcher) => {
                    app.manage(watcher);
                }
                Err(error) => eprintln!("Unable to watch user themes: {error}"),
            }
            window.show()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            get_theme,
            theme_catalog,
            set_theme,
            get_settings,
            hosting::create_hosting_link,
            hosting::hosting_remotes,
            hosting::open_hosting_link,
            hosting::update_hosting_hosts,
            hosting::get_hosting_hosts,
            hosting::open_theme_directory,
            hosting::reveal_workspace_file,
            native_menu::update_native_menu_context,
            save_layout,
            save_history_preferences,
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
            history_hashes,
            history_children,
            history_revert_has_merge,
            history_tree_open,
            history_tree_files,
            history_tree_diff,
            file_history_open,
            file_history_page,
            annotate_open,
            annotate_page,
            annotate_cancel,
            compare_file_with_revision,
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
            history_write,
            operation_state,
            sequence,
            conflict_content,
            resolve_conflict,
            continue_update,
            rollback_hunk,
            rollback_files,
            create_branch,
            checkout_branch,
            checkout_remote_branch,
            rename_branch,
            delete_branch,
            integrate,
            comparison_open,
            comparison_files,
            comparison_commits,
            comparison_diff,
            set_upstream,
            fetch,
            update_project,
            default_update_strategy,
            abort_update,
            push,
            push_preview,
            commit_and_push,
            stash_page,
            stash_diff,
            stash_write,
            stash_create,
            changes_add,
            changes_delete,
            credential::answer_credential,
        ])
        .on_menu_event(|app, event| {
            let store = app.state::<SettingsStore>();
            let id = event.id().as_ref();
            let window = active_window(app);
            if id == "theme-settings" {
                if let Some(window) = window {
                    let _ = app.emit_to(window.label(), "open-theme-settings", ());
                }
                return;
            }
            if matches!(id, "open-project" | "close-project") || id.starts_with("recent:") {
                if let Some(window) = window {
                    let _ = app.emit_to(window.label(), "repository-menu", id);
                }
                return;
            }
            if let Some(window) = &window {
                if let Ok(state) = app.state::<ProjectWindows>().get(window.label()) {
                    let sessions = WindowSession {
                        label: window.label().into(),
                        state,
                    };
                    if native_menu::menu_action_enabled(&sessions, id) {
                        let _ = app.emit_to(window.label(), "repository-menu", id);
                        return;
                    }
                }
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
            let Some(choice_id) = id.strip_prefix("theme-") else {
                return;
            };
            let Some(choice) = ThemeChoice::from_id(choice_id) else {
                return;
            };
            let themes = app.state::<ThemeRegistry>();
            if let Err(error) = apply_theme_choice(app, &store, &themes, choice) {
                eprintln!("Unable to select theme: {error}");
            }
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
