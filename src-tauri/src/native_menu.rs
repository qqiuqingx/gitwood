//! Native menus; all user-visible strings are supplied by ui/src/i18n.
use crate::{
    native_git_menu,
    project_windows::{ProjectWindows, WindowSession},
    settings::{Settings, ThemeChoice},
};
use gitwood_core::repo::{refs::References, session::StatusSummary};
use serde::Deserialize;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu},
    AppHandle, Manager,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MenuLabels {
    file: String,
    open: String,
    recent_projects: String,
    close_project: String,
    view: String,
    theme: String,
    system: String,
    light: String,
    dark: String,
    walnut: String,
    git: String,
    update: String,
    commit_action: String,
    push: String,
    commit: String,
    log: String,
    quit: String,
    edit: String,
    undo: String,
    redo: String,
    cut: String,
    copy: String,
    paste: String,
    select_all: String,
    pull: String,
    fetch: String,
    merge: String,
    commit_merge: String,
    abort_merge: String,
    rebase: String,
    continue_rebase: String,
    abort_rebase: String,
    skip_commit: String,
    branches: String,
    new_branch: String,
    new_tag: String,
    reset_head: String,
    show_git_log: String,
    uncommitted_changes: String,
    stash: String,
    unstash: String,
    rollback: String,
    current_file: String,
    commit_file: String,
    add_to_vcs: String,
    show_history: String,
    annotate: String,
    compare_revision: String,
    compare_branch: String,
    vcs_operations: String,
}

impl MenuLabels {
    pub(crate) fn text(&self, key: &str) -> &str {
        match key {
            "file" | "open" | "recentProjects" | "closeProject" | "view" | "theme" | "system"
            | "light" | "dark" | "walnut" | "commit" | "log" | "quit" | "edit" | "undo"
            | "redo" | "cut" | "copy" | "paste" | "selectAll" => self.application_text(key),
            _ => self.git_text(key),
        }
    }

    fn application_text(&self, key: &str) -> &str {
        match key {
            "file" => &self.file,
            "open" => &self.open,
            "recentProjects" => &self.recent_projects,
            "closeProject" => &self.close_project,
            "view" => &self.view,
            "theme" => &self.theme,
            "system" => &self.system,
            "light" => &self.light,
            "dark" => &self.dark,
            "walnut" => &self.walnut,
            "commit" => &self.commit,
            "log" => &self.log,
            "quit" => &self.quit,
            "edit" => &self.edit,
            "undo" => &self.undo,
            "redo" => &self.redo,
            "cut" => &self.cut,
            "copy" => &self.copy,
            "paste" => &self.paste,
            "selectAll" => &self.select_all,
            _ => unreachable!("unknown application menu label key: {key}"),
        }
    }

    fn git_text(&self, key: &str) -> &str {
        match key {
            "git" => &self.git,
            "update" => &self.update,
            "commitAction" => &self.commit_action,
            "push" => &self.push,
            "pull" => &self.pull,
            "fetch" => &self.fetch,
            "merge" => &self.merge,
            "commitMerge" => &self.commit_merge,
            "abortMerge" => &self.abort_merge,
            "rebase" => &self.rebase,
            "continueRebase" => &self.continue_rebase,
            "abortRebase" => &self.abort_rebase,
            "skipCommit" => &self.skip_commit,
            "branches" => &self.branches,
            "newBranch" => &self.new_branch,
            "newTag" => &self.new_tag,
            "resetHead" => &self.reset_head,
            "showGitLog" => &self.show_git_log,
            "uncommittedChanges" => &self.uncommitted_changes,
            "stash" => &self.stash,
            "unstash" => &self.unstash,
            "rollback" => &self.rollback,
            "currentFile" => &self.current_file,
            "commitFile" => &self.commit_file,
            "addToVcs" => &self.add_to_vcs,
            "showHistory" => &self.show_history,
            "annotate" => &self.annotate,
            "compareRevision" => &self.compare_revision,
            "compareBranch" => &self.compare_branch,
            "vcsOperations" => &self.vcs_operations,
            _ => unreachable!("unknown Git menu label key: {key}"),
        }
    }
}

pub(crate) use crate::native_menu_state::{
    NativeMenuActions, NativeMenuSelection, ProjectMenuContext,
};

pub(crate) fn install_menu(
    app: &AppHandle,
    settings: &Settings,
    labels: &MenuLabels,
) -> tauri::Result<()> {
    let menu = Menu::new(app)?;
    #[cfg(target_os = "macos")]
    menu.append(&quit_menu(app, labels)?)?;
    append_project_menus(app, &menu, settings, labels)?;
    menu.append(&native_git_menu::build(
        app,
        labels,
        &NativeMenuActions::default(),
    )?)?;
    append_view_menu(app, &menu, settings, labels)?;
    #[cfg(not(target_os = "macos"))]
    menu.append(&quit_menu(app, labels)?)?;
    app.set_menu(menu)?;
    update_menu(
        app.menu().expect("menu was installed").items()?,
        settings,
        &NativeMenuActions::default(),
    )?;
    Ok(())
}

fn quit_menu(app: &AppHandle, labels: &MenuLabels) -> tauri::Result<Submenu<tauri::Wry>> {
    Submenu::with_items(
        app,
        "Gitwood",
        true,
        &[&PredefinedMenuItem::quit(app, Some(labels.text("quit")))?],
    )
}

fn append_project_menus(
    app: &AppHandle,
    menu: &Menu<tauri::Wry>,
    settings: &Settings,
    labels: &MenuLabels,
) -> tauri::Result<()> {
    let recent = Submenu::with_id(app, "recent-projects", labels.text("recentProjects"), true)?;
    populate_recent(app, &recent, settings)?;
    menu.append(&Submenu::with_items(
        app,
        labels.text("file"),
        true,
        &[
            &MenuItem::with_id(
                app,
                "open-project",
                labels.text("open"),
                true,
                Some("CmdOrCtrl+O"),
            )?,
            &recent,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                "close-project",
                labels.text("closeProject"),
                true,
                None::<&str>,
            )?,
        ],
    )?)?;
    let edit = Submenu::with_items(
        app,
        labels.text("edit"),
        true,
        &[
            &PredefinedMenuItem::undo(app, Some(labels.text("undo")))?,
            &PredefinedMenuItem::redo(app, Some(labels.text("redo")))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some(labels.text("cut")))?,
            &PredefinedMenuItem::copy(app, Some(labels.text("copy")))?,
            &PredefinedMenuItem::paste(app, Some(labels.text("paste")))?,
            &PredefinedMenuItem::select_all(app, Some(labels.text("selectAll")))?,
        ],
    )?;
    menu.append(&edit)?;
    Ok(())
}

fn append_view_menu(
    app: &AppHandle,
    menu: &Menu<tauri::Wry>,
    settings: &Settings,
    labels: &MenuLabels,
) -> tauri::Result<()> {
    let commit = CheckMenuItem::with_id(
        app,
        "commit",
        labels.text("commit"),
        true,
        settings.layout.commit_visible,
        Some(if cfg!(target_os = "macos") {
            "Cmd+0"
        } else {
            "Alt+0"
        }),
    )?;
    let log = CheckMenuItem::with_id(
        app,
        "log",
        labels.text("log"),
        true,
        settings.layout.log_visible,
        Some(if cfg!(target_os = "macos") {
            "Cmd+9"
        } else {
            "Alt+9"
        }),
    )?;
    let themes = theme_menu(app, settings, labels)?;
    menu.append(&Submenu::with_items(
        app,
        labels.text("view"),
        true,
        &[&commit, &log, &PredefinedMenuItem::separator(app)?, &themes],
    )?)?;
    Ok(())
}

fn theme_menu(
    app: &AppHandle,
    settings: &Settings,
    labels: &MenuLabels,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let themes = Submenu::new(app, labels.text("theme"), true)?;
    for (id, key, choice) in [
        ("system", "system", ThemeChoice::System),
        ("light", "light", ThemeChoice::Light),
        ("dark", "dark", ThemeChoice::Dark),
        ("walnut", "walnut", ThemeChoice::Walnut),
    ] {
        themes.append(&CheckMenuItem::with_id(
            app,
            format!("theme-{id}"),
            labels.text(key),
            true,
            settings.theme == choice,
            None::<&str>,
        )?)?;
    }
    Ok(themes)
}

pub(crate) fn update_menu(
    items: Vec<MenuItemKind<tauri::Wry>>,
    settings: &Settings,
    native: &NativeMenuActions,
) -> tauri::Result<()> {
    for item in items {
        update_item(item, settings, native)?;
    }
    Ok(())
}

fn update_item(
    item: MenuItemKind<tauri::Wry>,
    settings: &Settings,
    native: &NativeMenuActions,
) -> tauri::Result<()> {
    match item {
        MenuItemKind::Submenu(submenu) => update_submenu(submenu, settings, native),
        MenuItemKind::MenuItem(menu_item) => update_action_item(menu_item, native),
        MenuItemKind::Check(check) => update_check_item(check, settings),
        _ => Ok(()),
    }
}

fn update_submenu(
    submenu: Submenu<tauri::Wry>,
    settings: &Settings,
    native: &NativeMenuActions,
) -> tauri::Result<()> {
    match submenu.id().as_ref() {
        "git-menu" => submenu.set_enabled(native.has_repository)?,
        "Git.MainMenu.LocalChanges" => submenu.set_enabled(
            native.is_enabled("Git.StashChanges")
                || native.is_enabled("Git.UnstashChanges")
                || native.is_enabled("ChangesView.Revert"),
        )?,
        "Git.MainMenu.FileActions" => submenu.set_enabled(
            native.is_enabled("ChangesView.CommitFile")
                || native.is_enabled("ChangesView.AddUnversioned")
                || native.is_enabled("Git.CurrentFile.ShowHistory"),
        )?,
        _ => {}
    }
    update_menu(submenu.items()?, settings, native)
}

fn update_action_item(item: MenuItem<tauri::Wry>, native: &NativeMenuActions) -> tauri::Result<()> {
    let id = item.id();
    if let Some(enabled) = native.actions.get(id.as_ref()) {
        item.set_enabled(*enabled)?;
    }
    Ok(())
}

fn update_check_item(item: CheckMenuItem<tauri::Wry>, settings: &Settings) -> tauri::Result<()> {
    let checked = match item.id().as_ref() {
        "commit" => Some(settings.layout.commit_visible),
        "log" => Some(settings.layout.log_visible),
        "theme-system" => Some(settings.theme == ThemeChoice::System),
        "theme-light" => Some(settings.theme == ThemeChoice::Light),
        "theme-dark" => Some(settings.theme == ThemeChoice::Dark),
        "theme-walnut" => Some(settings.theme == ThemeChoice::Walnut),
        _ => None,
    };
    if let Some(checked) = checked {
        item.set_checked(checked)?;
    }
    Ok(())
}

fn is_active_window(app: &AppHandle, label: &str) -> bool {
    if app
        .webview_windows()
        .values()
        .any(|window| window.label() == label && window.is_focused().unwrap_or(false))
    {
        return true;
    }
    app.state::<ProjectWindows>()
        .focused
        .lock()
        .unwrap()
        .as_deref()
        == Some(label)
}

fn publish_if_active(app: &AppHandle, sessions: &WindowSession, changed: bool) {
    if changed && is_active_window(app, &sessions.label) {
        crate::refresh_active_menu(app);
    }
}

pub(crate) fn repository_opening(app: &AppHandle, sessions: &WindowSession) {
    let changed = sessions
        .state
        .native_menu
        .lock()
        .unwrap()
        .clear_repository();
    publish_if_active(app, sessions, changed);
}

pub(crate) fn status_changed(app: &AppHandle, sessions: &WindowSession, status: &StatusSummary) {
    let changed = sessions
        .state
        .native_menu
        .lock()
        .unwrap()
        .set_status(status);
    publish_if_active(app, sessions, changed);
}

pub(crate) fn references_changed(
    app: &AppHandle,
    sessions: &WindowSession,
    references: &References,
) {
    let changed = sessions
        .state
        .native_menu
        .lock()
        .unwrap()
        .set_references(references);
    publish_if_active(app, sessions, changed);
}

#[tauri::command]
pub(crate) fn update_native_menu_context(
    app: AppHandle,
    sessions: WindowSession,
    generation: u64,
    selection: NativeMenuSelection,
) -> Result<NativeMenuActions, String> {
    sessions
        .lease(generation)
        .map_err(|error| error.to_string())?;
    let (changed, presentation) = {
        let mut context = sessions.state.native_menu.lock().unwrap();
        let changed = context.set_selection(selection);
        (changed, context.presentation())
    };
    publish_if_active(&app, &sessions, changed);
    Ok(presentation)
}

pub(crate) fn menu_action_enabled(sessions: &WindowSession, action: &str) -> bool {
    sessions
        .state
        .native_menu
        .lock()
        .unwrap()
        .presentation()
        .is_enabled(action)
}

fn populate_recent(
    app: &AppHandle,
    submenu: &Submenu<tauri::Wry>,
    settings: &Settings,
) -> tauri::Result<()> {
    while submenu.remove_at(0)?.is_some() {}
    for repository in &settings.recent_repositories {
        let path = repository.path.to_string_lossy();
        submenu.append(&MenuItem::with_id(
            app,
            format!("recent:{path}"),
            path.as_ref(),
            true,
            None::<&str>,
        )?)?;
    }
    Ok(())
}

pub(crate) fn refresh_recent_menu(app: &AppHandle, settings: &Settings) -> tauri::Result<()> {
    fn visit(
        app: &AppHandle,
        settings: &Settings,
        items: Vec<MenuItemKind<tauri::Wry>>,
    ) -> tauri::Result<()> {
        for item in items {
            if let MenuItemKind::Submenu(submenu) = item {
                if submenu.id().as_ref() == "recent-projects" {
                    populate_recent(app, &submenu, settings)?;
                } else {
                    visit(app, settings, submenu.items()?)?;
                }
            }
        }
        Ok(())
    }
    if let Some(menu) = app.menu() {
        visit(app, settings, menu.items()?)?;
    }
    Ok(())
}
