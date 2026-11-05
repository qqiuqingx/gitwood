//! Application settings, recent repository records and layout/window constraints.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
    Walnut,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Ratios {
    pub workspace_log: f64,
    pub commit_diff: f64,
    pub diff_versions: f64,
    pub branches_commits: f64,
    pub commits_information: f64,
    pub files_details: f64,
}
impl Default for Ratios {
    fn default() -> Self {
        Self {
            workspace_log: 0.59,
            commit_diff: 0.22,
            diff_versions: 0.5,
            branches_commits: 0.18,
            commits_information: 0.7,
            files_details: 0.6,
        }
    }
}
impl Ratios {
    pub fn normalize(&mut self) {
        let defaults = Self::default();
        for (value, default) in [
            (&mut self.workspace_log, defaults.workspace_log),
            (&mut self.commit_diff, defaults.commit_diff),
            (&mut self.diff_versions, defaults.diff_versions),
            (&mut self.branches_commits, defaults.branches_commits),
            (&mut self.commits_information, defaults.commits_information),
            (&mut self.files_details, defaults.files_details),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(value) {
                *value = default;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Layout {
    pub ratios: Ratios,
    pub commit_visible: bool,
    pub log_visible: bool,
    /// Commit panel "Group by Directory" and file preview toggles.
    pub commit_flat: bool,
    pub commit_preview: bool,
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            ratios: Ratios::default(),
            commit_visible: true,
            log_visible: true,
            commit_flat: true,
            commit_preview: true,
        }
    }
}

/// Physical outer-window bounds, matching monitor work areas (including negative origins).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentRepository {
    pub path: PathBuf,
    pub failed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub schema_version: u32,
    pub theme: ThemeChoice,
    pub layout: Layout,
    pub window: Option<Rect>,
    pub recent_repositories: Vec<RecentRepository>,
    pub cherry_pick_add_suffix_to_published_commits: bool,
    pub empty_cherry_pick: gitwood_core::ops::history::EmptyCherryPick,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            theme: ThemeChoice::System,
            layout: Layout::default(),
            window: None,
            recent_repositories: Vec::new(),
            cherry_pick_add_suffix_to_published_commits: true,
            empty_cherry_pick: gitwood_core::ops::history::EmptyCherryPick::Skip,
        }
    }
}

pub fn decode(json: &str) -> Result<Settings, String> {
    let mut value: Settings = serde_json::from_str(json).map_err(|e| e.to_string())?;
    // Missing or unsupported versions are damaged/foreign files, not migrations.
    let document: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if document["schemaVersion"].as_u64() != Some(1) {
        return Err("Unsupported settings schemaVersion".into());
    }
    value.layout.ratios.normalize();
    let mut seen = std::collections::HashSet::new();
    value
        .recent_repositories
        .retain(|r| seen.insert(r.path.clone()));
    value.recent_repositories.truncate(20);
    if value.window.is_some_and(|r| r.width == 0 || r.height == 0) {
        value.window = None;
    }
    Ok(value)
}

pub fn load(path: &Path) -> (Settings, Option<String>) {
    match fs::read_to_string(path) {
        Ok(json) => match decode(&json) {
            Ok(value) => (value, None),
            Err(e) => (Settings::default(), Some(e)),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => (Settings::default(), None),
        Err(e) => (Settings::default(), Some(e.to_string())),
    }
}

/// Select greatest overlap, otherwise nearest display, then fit entirely inside its work area.
/// Minimums are physical pixels on that display; smaller displays take precedence.
pub fn constrain_window(window: Rect, screens: &[(Rect, (u32, u32))]) -> Rect {
    let overlap = |a: Rect, b: Rect| {
        let width = ((a.x as i64 + a.width as i64).min(b.x as i64 + b.width as i64)
            - (a.x as i64).max(b.x as i64))
        .max(0);
        let height = ((a.y as i64 + a.height as i64).min(b.y as i64 + b.height as i64)
            - (a.y as i64).max(b.y as i64))
        .max(0);
        width as u64 * height as u64
    };
    let distance = |r: Rect| {
        let x = window.x as f64 + window.width as f64 / 2.0 - (r.x as f64 + r.width as f64 / 2.0);
        let y = window.y as f64 + window.height as f64 / 2.0 - (r.y as f64 + r.height as f64 / 2.0);
        x * x + y * y
    };
    let Some(&(screen, minimum)) = screens
        .iter()
        .filter(|(r, _)| r.width > 0 && r.height > 0)
        .max_by(|(a, _), (b, _)| {
            overlap(window, *a)
                .cmp(&overlap(window, *b))
                .then_with(|| distance(*b).total_cmp(&distance(*a)))
        })
    else {
        return window;
    };
    let width = window.width.max(minimum.0).min(screen.width);
    let height = window.height.max(minimum.1).min(screen.height);
    Rect {
        x: (window.x as i64).clamp(
            screen.x as i64,
            screen.x as i64 + screen.width as i64 - width as i64,
        ) as i32,
        y: (window.y as i64).clamp(
            screen.y as i64,
            screen.y as i64 + screen.height as i64 - height as i64,
        ) as i32,
        width,
        height,
    }
}

struct Data {
    settings: Settings,
    dirty: bool,
    load_error: Option<String>,
    save_error: Option<String>,
}
struct Shared {
    path: PathBuf,
    data: Mutex<Data>,
    write_lock: Mutex<()>,
}
impl Shared {
    fn flush(&self) -> Result<(), String> {
        let _write = self.write_lock.lock().unwrap();
        // Hold the data lock through the rename so an older snapshot cannot overwrite newer data.
        let mut data = self.data.lock().unwrap();
        if !data.dirty {
            return Ok(());
        }
        let result = (|| -> io::Result<()> {
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent)?;
            }
            let temporary = self.path.with_extension("json.tmp");
            let mut file = File::create(&temporary)?;
            file.write_all(serde_json::to_string_pretty(&data.settings)?.as_bytes())?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            drop(file);
            // std rename replaces atomically on supported desktop platforms.
            fs::rename(&temporary, &self.path)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                data.dirty = false;
                data.save_error = None;
                Ok(())
            }
            Err(e) => {
                let message = e.to_string();
                data.save_error = Some(message.clone());
                Err(message)
            }
        }
    }
}

#[derive(Clone)]
pub struct SettingsStore {
    shared: Arc<Shared>,
    changed: mpsc::Sender<()>,
}
impl SettingsStore {
    pub fn open(path: PathBuf, on_error: impl Fn(String) + Send + 'static) -> Self {
        let (settings, error) = load(&path);
        let shared = Arc::new(Shared {
            path,
            data: Mutex::new(Data {
                settings,
                dirty: false,
                load_error: error,
                save_error: None,
            }),
            write_lock: Mutex::new(()),
        });
        let (changed, receiver) = mpsc::channel();
        let weak = Arc::downgrade(&shared);
        std::thread::spawn(move || {
            while receiver.recv().is_ok() {
                // Block while idle; only pending changes start a 250 ms debounce deadline.
                loop {
                    match receiver.recv_timeout(Duration::from_millis(250)) {
                        Ok(()) => continue,
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                let Some(shared) = weak.upgrade() else {
                    return;
                };
                if let Err(e) = shared.flush() {
                    on_error(e);
                }
            }
        });
        Self { shared, changed }
    }
    pub fn snapshot(&self) -> Settings {
        self.shared.data.lock().unwrap().settings.clone()
    }
    pub fn error(&self) -> Option<String> {
        let data = self.shared.data.lock().unwrap();
        data.save_error.clone().or_else(|| data.load_error.clone())
    }
    pub fn save_error(&self) -> Option<String> {
        self.shared.data.lock().unwrap().save_error.clone()
    }
    pub fn update(&self, change: impl FnOnce(&mut Settings)) {
        let mut data = self.shared.data.lock().unwrap();
        change(&mut data.settings);
        data.settings.layout.ratios.normalize();
        data.dirty = true;
        drop(data);
        let _ = self.changed.send(());
    }
    pub fn flush(&self) -> Result<(), String> {
        self.shared.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_roundtrip_preserves_hidden_ratios_and_window() {
        assert!(Settings::default().cherry_pick_add_suffix_to_published_commits);
        assert_eq!(
            Settings::default().empty_cherry_pick,
            gitwood_core::ops::history::EmptyCherryPick::Skip
        );
        let original = Settings {
            theme: ThemeChoice::Walnut,
            layout: Layout {
                commit_visible: false,
                log_visible: false,
                commit_flat: false,
                commit_preview: false,
                ratios: Ratios {
                    commit_diff: 0.37,
                    workspace_log: 0.63,
                    diff_versions: 0.42,
                    ..Ratios::default()
                },
            },
            window: Some(Rect {
                x: -1200,
                y: 100,
                width: 1000,
                height: 700,
            }),
            cherry_pick_add_suffix_to_published_commits: false,
            empty_cherry_pick: gitwood_core::ops::history::EmptyCherryPick::CreateEmpty,
            ..Settings::default()
        };
        assert_eq!(
            decode(&serde_json::to_string(&original).unwrap()).unwrap(),
            original
        );
    }
    #[test]
    fn corrupt_file_loads_defaults_with_diagnostic() {
        let path = std::env::temp_dir().join(format!(
            "gitwood-settings-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, b"{broken").unwrap();
        let (value, error) = load(&path);
        fs::remove_file(&path).unwrap();
        assert_eq!(value, Settings::default());
        assert!(error.is_some());
        assert!(decode(r#"{"schemaVersion":2}"#).is_err());
        assert!(decode("{}").is_err());
    }
    #[test]
    fn synchronous_exit_flush_saves_latest_snapshot_without_debounce_wait() {
        let directory = std::env::temp_dir().join(format!(
            "gitwood-flush-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("settings.json");
        let store = SettingsStore::open(path.clone(), |_| {});
        store.update(|s| {
            s.layout.ratios.commit_diff = 0.31;
            s.theme = ThemeChoice::Dark;
        });
        store.update(|s| {
            s.layout.ratios.commit_diff = 0.44;
            s.layout.commit_visible = false;
        });
        store.flush().unwrap();
        let (saved, error) = load(&path);
        assert!(error.is_none());
        assert_eq!(saved, store.snapshot());
        assert_eq!(saved.layout.ratios.commit_diff, 0.44);
        assert!(!path.with_extension("json.tmp").exists());
        drop(store);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn failed_save_returns_error_and_retains_dirty_state_for_retry() {
        let directory = std::env::temp_dir().join(format!(
            "gitwood-retry-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&directory, b"not a directory").unwrap();
        let path = directory.join("settings.json");
        let store = SettingsStore::open(path.clone(), |_| {});
        store.update(|s| s.theme = ThemeChoice::Walnut);
        assert!(store.flush().is_err());
        assert!(store.save_error().is_some());
        fs::remove_file(&directory).unwrap();
        store.flush().unwrap();
        assert!(store.save_error().is_none());
        assert_eq!(load(&path).0.theme, ThemeChoice::Walnut);
        drop(store);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn invalid_ratios_recover_individually() {
        let value = decode(
            r#"{"schemaVersion":1,"layout":{"ratios":{"commitDiff":-5,"workspaceLog":0.7}}}"#,
        )
        .unwrap();
        assert_eq!(value.layout.ratios.commit_diff, 0.22);
        assert_eq!(value.layout.ratios.workspace_log, 0.7);
    }
    #[test]
    fn removed_monitor_and_shrunken_work_area_fit_visible_display() {
        let screen = Rect {
            x: 0,
            y: 24,
            width: 1280,
            height: 750,
        };
        let window = Rect {
            x: 3000,
            y: -100,
            width: 1800,
            height: 1000,
        };
        assert_eq!(constrain_window(window, &[(screen, (900, 600))]), screen);
        let small = Rect {
            x: -500,
            y: 0,
            width: 500,
            height: 400,
        };
        assert_eq!(constrain_window(window, &[(small, (900, 600))]), small);
    }
    #[test]
    fn keeps_negative_monitor_origins_and_enforces_minimum() {
        let left = Rect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let right = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let window = Rect {
            x: -1700,
            y: 100,
            width: 1000,
            height: 800,
        };
        assert_eq!(
            constrain_window(window, &[(right, (900, 600)), (left, (900, 600))]),
            window
        );
        let tiny = Rect {
            x: 1800,
            y: 1000,
            width: 5,
            height: 5,
        };
        assert_eq!(
            constrain_window(tiny, &[(right, (900, 600))]),
            Rect {
                x: 1020,
                y: 480,
                width: 900,
                height: 600
            }
        );
        assert_eq!(constrain_window(window, &[]), window);
    }
}

#[cfg(test)]
mod recent_tests {
    use super::*;
    #[test]
    fn old_schema_and_recent_records_roundtrip() {
        assert!(decode(r#"{"schemaVersion":1}"#)
            .unwrap()
            .recent_repositories
            .is_empty());
        let settings = Settings {
            recent_repositories: (0..25)
                .map(|n| RecentRepository {
                    path: PathBuf::from(format!("repo-{n}")),
                    failed: n == 0,
                })
                .collect(),
            ..Settings::default()
        };
        let restored = decode(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.recent_repositories.len(), 20);
        assert!(restored.recent_repositories[0].failed);
    }
}
