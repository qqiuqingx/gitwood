//! Native menus; all user-visible strings are supplied by ui/src/i18n.
use crate::settings::{Settings, ThemeChoice};
use serde::Deserialize;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu},
    AppHandle,
};

/// Native menu strings originate in the same frontend i18n catalog as shell labels.
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
}
pub(crate) fn install_menu(
    app: &AppHandle,
    settings: &Settings,
    labels: &MenuLabels,
) -> tauri::Result<()> {
    let menu = Menu::new(app)?;
    // macOS reserves the first menu for application actions.
    #[cfg(target_os = "macos")]
    menu.append(&Submenu::with_items(
        app,
        "Gitwood",
        true,
        &[&PredefinedMenuItem::quit(app, Some(&labels.quit))?],
    )?)?;
    let recent = Submenu::with_id(app, "recent-projects", &labels.recent_projects, true)?;
    populate_recent(app, &recent, settings)?;
    menu.append(&Submenu::with_items(
        app,
        &labels.file,
        true,
        &[
            &MenuItem::with_id(app, "open-project", &labels.open, true, Some("CmdOrCtrl+O"))?,
            &recent,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                "close-project",
                &labels.close_project,
                true,
                None::<&str>,
            )?,
        ],
    )?)?;
    let edit = Submenu::with_items(
        app,
        &labels.edit,
        true,
        &[
            &PredefinedMenuItem::undo(app, Some(&labels.undo))?,
            &PredefinedMenuItem::redo(app, Some(&labels.redo))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some(&labels.cut))?,
            &PredefinedMenuItem::copy(app, Some(&labels.copy))?,
            &PredefinedMenuItem::paste(app, Some(&labels.paste))?,
            &PredefinedMenuItem::select_all(app, Some(&labels.select_all))?,
        ],
    )?;
    menu.append(&edit)?;
    let git = Submenu::with_items(
        app,
        &labels.git,
        true,
        &[
            &MenuItem::with_id(
                app,
                "update-project",
                &labels.update,
                true,
                Some("CmdOrCtrl+T"),
            )?,
            &MenuItem::with_id(
                app,
                "commit-project",
                &labels.commit_action,
                true,
                Some("CmdOrCtrl+K"),
            )?,
            &MenuItem::with_id(
                app,
                "push-project",
                &labels.push,
                true,
                Some("CmdOrCtrl+Shift+K"),
            )?,
        ],
    )?;
    menu.append(&git)?;
    let commit = CheckMenuItem::with_id(
        app,
        "commit",
        &labels.commit,
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
        &labels.log,
        true,
        settings.layout.log_visible,
        Some(if cfg!(target_os = "macos") {
            "Cmd+9"
        } else {
            "Alt+9"
        }),
    )?;
    let themes = Submenu::new(app, &labels.theme, true)?;
    for (id, label, choice) in [
        ("system", &labels.system, ThemeChoice::System),
        ("light", &labels.light, ThemeChoice::Light),
        ("dark", &labels.dark, ThemeChoice::Dark),
        ("walnut", &labels.walnut, ThemeChoice::Walnut),
    ] {
        themes.append(&CheckMenuItem::with_id(
            app,
            format!("theme-{id}"),
            label,
            true,
            settings.theme == choice,
            None::<&str>,
        )?)?;
    }
    menu.append(&Submenu::with_items(
        app,
        &labels.view,
        true,
        &[&commit, &log, &PredefinedMenuItem::separator(app)?, &themes],
    )?)?;
    #[cfg(not(target_os = "macos"))]
    menu.append(&Submenu::with_items(
        app,
        "Gitwood",
        true,
        &[&PredefinedMenuItem::quit(app, Some(&labels.quit))?],
    )?)?;
    app.set_menu(menu)?;
    Ok(())
}

pub(crate) fn update_menu(
    items: Vec<MenuItemKind<tauri::Wry>>,
    settings: &Settings,
) -> tauri::Result<()> {
    for item in items {
        match item {
            MenuItemKind::Submenu(submenu) => update_menu(submenu.items()?, settings)?,
            MenuItemKind::Check(check) => {
                let checked = match check.id().as_ref() {
                    "commit" => settings.layout.commit_visible,
                    "log" => settings.layout.log_visible,
                    "theme-system" => settings.theme == ThemeChoice::System,
                    "theme-light" => settings.theme == ThemeChoice::Light,
                    "theme-dark" => settings.theme == ThemeChoice::Dark,
                    "theme-walnut" => settings.theme == ThemeChoice::Walnut,
                    _ => continue,
                };
                check.set_checked(checked)?;
            }
            _ => {}
        }
    }
    Ok(())
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
