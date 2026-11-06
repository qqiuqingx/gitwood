use gitwood_core::theme::{
    contrast_issues, get_theme, Theme, ThemeError, ThemeFileError, UserTheme, UserThemeCatalog,
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tauri::Manager;

#[derive(Clone)]
pub struct ThemeRegistry {
    directory: PathBuf,
    catalog: Arc<RwLock<UserThemeCatalog>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeDescriptor {
    pub id: String,
    pub name: String,
    pub dark: bool,
    pub built_in: bool,
    pub contrast_issues: Vec<gitwood_core::theme::ContrastIssue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeCatalogResponse {
    pub directory: String,
    pub themes: Vec<ThemeDescriptor>,
    pub errors: Vec<ThemeFileError>,
}

impl ThemeRegistry {
    pub fn open(directory: PathBuf) -> Self {
        let catalog = gitwood_core::theme::load_user_themes(&directory);
        Self {
            directory,
            catalog: Arc::new(RwLock::new(catalog)),
        }
    }

    pub fn refresh(&self) -> UserThemeCatalog {
        let catalog = gitwood_core::theme::load_user_themes(&self.directory);
        *self.catalog.write().unwrap() = catalog.clone();
        catalog
    }

    pub fn users(&self) -> Vec<UserTheme> {
        self.catalog.read().unwrap().themes.clone()
    }

    pub fn has_user(&self, id: &str) -> bool {
        self.catalog
            .read()
            .unwrap()
            .themes
            .iter()
            .any(|theme| theme.id == id)
    }

    pub fn theme(&self, id: &str, system_dark: bool) -> Theme {
        match id {
            "light" | "dark" | "walnut" => get_theme(id),
            _ => match self
                .catalog
                .read()
                .unwrap()
                .themes
                .iter()
                .find(|theme| theme.id == id)
                .map(|theme| theme.theme.clone())
            {
                Some(theme) => theme,
                None if id.starts_with("user:") => {
                    unavailable_theme(id, if system_dark { "dark" } else { "light" })
                }
                None => unavailable_theme(id, "light"),
            },
        }
    }

    pub fn response(&self) -> ThemeCatalogResponse {
        let catalog = self.catalog.read().unwrap().clone();
        let mut themes = ["light", "dark", "walnut"]
            .into_iter()
            .map(|id| {
                let theme = get_theme(id);
                ThemeDescriptor {
                    id: id.into(),
                    name: theme.name.clone(),
                    dark: theme.dark,
                    built_in: true,
                    contrast_issues: contrast_issues(&theme),
                }
            })
            .collect::<Vec<_>>();
        themes.extend(catalog.themes.iter().map(|user| ThemeDescriptor {
            id: user.id.clone(),
            name: user.theme.name.clone(),
            dark: user.theme.dark,
            built_in: false,
            contrast_issues: contrast_issues(&user.theme),
        }));
        ThemeCatalogResponse {
            directory: self.directory.to_string_lossy().into_owned(),
            themes,
            errors: catalog.errors,
        }
    }

    pub fn directory(&self) -> PathBuf {
        self.directory.clone()
    }
}

fn unavailable_theme(id: &str, fallback_name: &str) -> Theme {
    let mut fallback = get_theme(fallback_name);
    fallback.errors.push(ThemeError {
        code: "theme_unavailable".into(),
        path: id.into(),
        message: "The selected theme is unavailable; using the current system theme".into(),
    });
    fallback
}

#[cfg(target_os = "macos")]
pub fn user_theme_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .home_dir()
        .map(|home| home.join("Library/Application Support/Gitwood/Themes"))
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
pub fn user_theme_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            app.path()
                .home_dir()
                .ok()
                .map(|home| home.join("AppData/Roaming"))
        })
        .ok_or_else(|| "Unable to locate the roaming application data directory".to_owned())?;
    Ok(app_data.join("Gitwood/Themes"))
}

#[cfg(target_os = "linux")]
pub fn user_theme_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .home_dir()
        .map(|home| home.join(".config/gitwood/themes"))
        .map_err(|error| error.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn user_theme_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|directory| directory.join("themes"))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{decode, Settings, ThemeChoice};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temporary_directory() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "gitwood-theme-registry-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn unavailable_saved_user_choice_uses_the_live_system_theme() {
        let directory = temporary_directory();
        let registry = ThemeRegistry::open(directory.clone());
        let selected = ThemeChoice::from_id("user:deleted").unwrap();
        assert!(selected.is_user());
        assert!(!registry.has_user(&selected.id()));
        let fallback = registry.theme(&selected.id(), true);
        assert_eq!(fallback.name, "Dark");
        assert_eq!(fallback.errors[0].code, "theme_unavailable");
        let json = serde_json::to_string(&Settings {
            theme: selected.clone(),
            ..Settings::default()
        })
        .unwrap();
        assert_eq!(decode(&json).unwrap().theme, selected);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_or_deleted_active_theme_keeps_its_saved_choice_and_recovers() {
        let directory = temporary_directory();
        let path = directory.join("active.json");
        fs::write(
            &path,
            r##"{"schemaVersion":1,"name":"Active","author":"A","dark":false,"extends":"light","colors":{}}"##,
        )
        .unwrap();
        let registry = ThemeRegistry::open(directory.clone());
        let active = ThemeChoice::from_id("user:active").unwrap();
        assert!(registry.has_user(&active.id()));
        fs::write(&path, "{").unwrap();
        registry.refresh();
        assert_eq!(registry.theme(&active.id(), false).name, "Light");
        assert_eq!(active.id(), "user:active");
        assert_eq!(registry.response().errors[0].errors[0].code, "invalid_json");
        fs::remove_file(path).unwrap();
        registry.refresh();
        assert_eq!(registry.theme(&active.id(), true).name, "Dark");
        assert_eq!(active.id(), "user:active");
        fs::write(
            directory.join("active.json"),
            r##"{"schemaVersion":1,"name":"Active","author":"A","dark":false,"extends":"light","colors":{}}"##,
        )
        .unwrap();
        registry.refresh();
        assert!(registry.has_user(&active.id()));
        assert_eq!(registry.theme(&active.id(), true).name, "Active");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn response_contains_builtin_and_user_contrast_summaries_and_errors() {
        let directory = temporary_directory();
        fs::write(directory.join("custom.json"), r##"{"schemaVersion":1,"name":"Custom","author":"A","dark":false,"extends":"light","colors":{"text.primary":"#aaaaaa"}}"##).unwrap();
        fs::write(directory.join("broken.json"), "{").unwrap();
        let registry = ThemeRegistry::open(directory.clone());
        let response = registry.response();
        assert_eq!(response.themes.len(), 4);
        assert!(response
            .themes
            .iter()
            .any(|theme| theme.id == "user:custom" && !theme.contrast_issues.is_empty()));
        assert_eq!(response.errors[0].file_name, "broken.json");
        fs::remove_dir_all(directory).unwrap();
    }
}
