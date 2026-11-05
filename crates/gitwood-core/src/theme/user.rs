use super::{parse_theme, Theme, ThemeError};
use serde::Serialize;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Read,
    path::Path,
};

pub const MAX_THEME_FILE_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTheme {
    pub id: String,
    pub file_name: String,
    pub theme: Theme,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeFileError {
    pub file_name: String,
    pub errors: Vec<ThemeError>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserThemeCatalog {
    pub themes: Vec<UserTheme>,
    pub errors: Vec<ThemeFileError>,
}

/// Load only direct, regular `.json` children; symbolic links and other files are ignored.
/// The directory itself may be a symbolic link (e.g. managed dotfiles); it is user-chosen.
pub fn load_user_themes(directory: &Path) -> UserThemeCatalog {
    match fs::metadata(directory) {
        Ok(metadata) if !metadata.is_dir() => {
            return UserThemeCatalog::default();
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return UserThemeCatalog::default();
        }
        Err(error) => {
            return UserThemeCatalog {
                themes: Vec::new(),
                errors: vec![file_error(
                    "<directory>",
                    "directory_read_error",
                    "$",
                    &error.to_string(),
                )],
            };
        }
    }
    let mut entries = match read_directory(directory) {
        Ok(Some(entries)) => entries,
        Ok(None) => return UserThemeCatalog::default(),
        Err(error) => {
            return UserThemeCatalog {
                themes: Vec::new(),
                errors: vec![error],
            }
        }
    };
    sort_entries(&mut entries);
    let mut catalog = UserThemeCatalog::default();
    let mut names = HashSet::from(["light".to_owned(), "dark".to_owned(), "walnut".to_owned()]);
    let mut ids = HashSet::new();
    for entry in entries {
        let theme = match load_entry(entry) {
            Ok(Some(theme)) => theme,
            Ok(None) => continue,
            Err(error) => {
                catalog.errors.push(error);
                continue;
            }
        };
        if !ids.insert(theme.id.clone()) {
            catalog.errors.push(file_error(
                &theme.file_name,
                "duplicate_id",
                "name",
                "Theme ID duplicates an earlier file",
            ));
            continue;
        }
        let normalized_name = theme.theme.name.trim().to_lowercase();
        if !names.insert(normalized_name) {
            catalog.errors.push(file_error(
                &theme.file_name,
                "duplicate_name",
                "name",
                "Theme name duplicates an earlier built-in or user theme",
            ));
            continue;
        }
        catalog.themes.push(theme);
    }
    catalog
}

fn read_directory(directory: &Path) -> Result<Option<Vec<fs::DirEntry>>, ThemeFileError> {
    match fs::read_dir(directory) {
        Ok(entries) => Ok(Some(entries.filter_map(Result::ok).collect())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(file_error(
            "<directory>",
            "directory_read_error",
            "$",
            &error.to_string(),
        )),
    }
}

fn sort_entries(entries: &mut [fs::DirEntry]) {
    entries.sort_by(|left, right| {
        let left_name = left.file_name().to_string_lossy().into_owned();
        let right_name = right.file_name().to_string_lossy().into_owned();
        left_name
            .to_lowercase()
            .cmp(&right_name.to_lowercase())
            .then_with(|| left_name.cmp(&right_name))
    });
}

fn load_entry(entry: fs::DirEntry) -> Result<Option<UserTheme>, ThemeFileError> {
    let file_name = entry.file_name().to_string_lossy().into_owned();
    if entry
        .path()
        .extension()
        .is_none_or(|extension| extension != "json")
    {
        return Ok(None);
    }
    match entry.file_type() {
        Ok(file_type) if file_type.is_file() => {}
        Ok(_) => return Ok(None),
        Err(error) => {
            return Err(file_error(
                &file_name,
                "metadata_error",
                "$",
                &error.to_string(),
            ))
        }
    }
    let bytes = read_bounded(&entry.path()).map_err(|error| file_read_error(&file_name, error))?;
    let json = String::from_utf8(bytes).expect("read_bounded validates UTF-8");
    let theme = parse_theme(&json);
    if !theme.errors.is_empty() {
        return Err(ThemeFileError {
            file_name,
            errors: theme.errors,
        });
    }
    let id = format!(
        "user:{}",
        entry
            .path()
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
    );
    Ok(Some(UserTheme {
        id,
        file_name,
        theme,
    }))
}

fn file_read_error(file_name: &str, error: ReadThemeError) -> ThemeFileError {
    match error {
        ReadThemeError::TooLarge => file_error(
            file_name,
            "file_too_large",
            "$",
            "Theme files must be at most 256 KiB",
        ),
        ReadThemeError::Io(error) => file_error(file_name, "read_error", "$", &error.to_string()),
        ReadThemeError::InvalidUtf8 => file_error(
            file_name,
            "invalid_utf8",
            "$",
            "Theme file must be UTF-8 JSON",
        ),
    }
}

enum ReadThemeError {
    TooLarge,
    Io(std::io::Error),
    InvalidUtf8,
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, ReadThemeError> {
    let mut file = File::open(path).map_err(ReadThemeError::Io)?;
    if file.metadata().map_err(ReadThemeError::Io)?.len() > MAX_THEME_FILE_BYTES {
        return Err(ReadThemeError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_THEME_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(ReadThemeError::Io)?;
    if bytes.len() as u64 > MAX_THEME_FILE_BYTES {
        return Err(ReadThemeError::TooLarge);
    }
    std::str::from_utf8(&bytes).map_err(|_| ReadThemeError::InvalidUtf8)?;
    Ok(bytes)
}

fn file_error(file_name: &str, code: &str, path: &str, message: &str) -> ThemeFileError {
    ThemeFileError {
        file_name: file_name.into(),
        errors: vec![ThemeError {
            code: code.into(),
            path: path.into(),
            message: message.into(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn document(name: &str, extends: &str, colors: serde_json::Value) -> String {
        json!({"schemaVersion":1,"name":name,"author":"Author","dark":true,"extends":extends,"colors":colors}).to_string()
    }

    #[test]
    fn loads_only_valid_direct_json_files_and_checks_limits_and_names() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join("valid.json"),
            document("Valid", "walnut", json!({"accent":"#123"})),
        )
        .unwrap();
        fs::write(directory.path().join("broken.json"), "{").unwrap();
        fs::write(
            directory.path().join("large.json"),
            vec![b' '; (MAX_THEME_FILE_BYTES + 1) as usize],
        )
        .unwrap();
        fs::write(
            directory.path().join("plain.txt"),
            document("Ignored", "light", json!({})),
        )
        .unwrap();
        fs::create_dir(directory.path().join("nested")).unwrap();
        fs::write(
            directory.path().join("nested/child.json"),
            document("Nested", "light", json!({})),
        )
        .unwrap();
        fs::write(
            directory.path().join("duplicate-a.json"),
            document("Same", "light", json!({})),
        )
        .unwrap();
        fs::write(
            directory.path().join("duplicate-b.json"),
            document(" same ", "dark", json!({})),
        )
        .unwrap();
        fs::write(
            directory.path().join("builtin-name.json"),
            document("Light", "light", json!({})),
        )
        .unwrap();
        let catalog = load_user_themes(directory.path());
        assert!(catalog
            .themes
            .iter()
            .any(|theme| theme.id == "user:valid" && theme.theme.variables["--accent"] == "#123"));
        assert!(!catalog
            .themes
            .iter()
            .any(|theme| theme.id.contains("nested") || theme.id.contains("duplicate-b")));
        assert!(catalog.errors.iter().any(
            |error| error.file_name == "broken.json" && error.errors[0].code == "invalid_json"
        ));
        assert!(catalog.errors.iter().any(
            |error| error.file_name == "large.json" && error.errors[0].code == "file_too_large"
        ));
        assert!(catalog
            .errors
            .iter()
            .any(|error| error.file_name == "duplicate-b.json"
                && error.errors[0].code == "duplicate_name"));
        assert!(catalog
            .errors
            .iter()
            .any(|error| error.file_name == "builtin-name.json"
                && error.errors[0].code == "duplicate_name"));
    }

    #[cfg(unix)]
    #[test]
    fn skips_symbolic_links() {
        use std::os::unix::fs::symlink;
        let directory = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let source = outside.path().join("source.json");
        fs::write(&source, document("Linked", "light", json!({}))).unwrap();
        symlink(source, directory.path().join("link.json")).unwrap();
        assert!(load_user_themes(directory.path()).themes.is_empty());
        assert!(load_user_themes(directory.path()).errors.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_theme_directory_is_followed() {
        use std::os::unix::fs::symlink;
        let target = tempdir().unwrap();
        let base = tempdir().unwrap();
        fs::write(
            target.path().join("linked.json"),
            document("Linked", "light", json!({})),
        )
        .unwrap();
        let directory = base.path().join("Themes");
        symlink(target.path(), &directory).unwrap();
        let catalog = load_user_themes(&directory);
        assert_eq!(catalog.themes.len(), 1);
        assert!(catalog.errors.is_empty());
    }

    #[test]
    fn missing_directory_means_no_user_themes() {
        let directory = tempdir().unwrap().path().join("not-created");
        assert!(load_user_themes(&directory).themes.is_empty());
        assert!(load_user_themes(&directory).errors.is_empty());
    }

    #[test]
    fn a_user_theme_can_extend_a_builtin_only() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join("derived.json"),
            document("Derived", "dark", json!({"accent":"#abc"})),
        )
        .unwrap();
        fs::write(
            directory.path().join("unsupported.json"),
            document("Unsupported", "user:derived", json!({})),
        )
        .unwrap();
        let catalog = load_user_themes(directory.path());
        let derived = catalog
            .themes
            .iter()
            .find(|theme| theme.id == "user:derived")
            .unwrap();
        assert_eq!(
            derived.theme.variables["--background-primary"],
            crate::theme::get_theme("dark").variables["--background-primary"]
        );
        assert_eq!(derived.theme.variables["--accent"], "#abc");
        assert_eq!(
            catalog
                .errors
                .iter()
                .find(|error| error.file_name == "unsupported.json")
                .unwrap()
                .errors[0]
                .code,
            "unknown_parent"
        );
    }

    #[test]
    fn extension_order_and_builtin_ids_are_deterministic() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join("z.json"),
            document("Z", "light", json!({})),
        )
        .unwrap();
        fs::write(
            directory.path().join("a.json"),
            document("A", "light", json!({})),
        )
        .unwrap();
        fs::write(
            directory.path().join("light.json"),
            document("User Light", "light", json!({})),
        )
        .unwrap();
        let catalog = load_user_themes(directory.path());
        assert_eq!(
            catalog
                .themes
                .iter()
                .map(|theme| theme.id.as_str())
                .collect::<Vec<_>>(),
            ["user:a", "user:light", "user:z"]
        );
        assert!(catalog
            .themes
            .iter()
            .all(|theme| theme.id.starts_with("user:")));
    }
}
