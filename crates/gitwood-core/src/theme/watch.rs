use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

enum Message {
    Event(notify::Result<notify::Event>),
    Stop,
}

/// Event-driven, non-recursive watch that also follows creation of a missing directory.
pub struct ThemeWatch {
    stop: Sender<Message>,
    worker: Option<JoinHandle<()>>,
}

impl ThemeWatch {
    pub fn start(
        directory: PathBuf,
        on_change: impl Fn() + Send + 'static,
    ) -> notify::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let callback_sender = sender.clone();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = callback_sender.send(Message::Event(event));
        })?;
        let watched = existing_ancestor(&directory);
        watcher.watch(&watched, RecursiveMode::NonRecursive)?;
        let worker =
            thread::spawn(move || run(receiver, &mut watcher, directory, watched, on_change));
        Ok(Self {
            stop: sender,
            worker: Some(worker),
        })
    }
}

impl Drop for ThemeWatch {
    fn drop(&mut self) {
        let _ = self.stop.send(Message::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(
    receiver: Receiver<Message>,
    watcher: &mut RecommendedWatcher,
    directory: PathBuf,
    mut watched: PathBuf,
    on_change: impl Fn(),
) {
    let mut pending = false;
    loop {
        let message = if pending {
            match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(message) => message,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    on_change();
                    pending = false;
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match receiver.recv() {
                Ok(message) => message,
                Err(_) => return,
            }
        };
        match message {
            Message::Stop => return,
            Message::Event(Ok(event)) if relevant(&directory, &watched, &event) => {
                pending = true;
                let next = existing_ancestor(&directory);
                if next != watched {
                    let _ = watcher.unwatch(&watched);
                    if watcher.watch(&next, RecursiveMode::NonRecursive).is_ok() {
                        watched = next;
                    }
                }
            }
            Message::Event(_) => {}
        }
    }
}

fn relevant(directory: &Path, watched: &Path, event: &notify::Event) -> bool {
    if matches!(event.kind, notify::EventKind::Access(_)) {
        return false;
    }
    let directory = normalized_path(directory);
    let watched = normalized_path(watched);
    event.paths.iter().any(|path| {
        let path = normalized_path(path);
        path.starts_with(&directory) || (directory.starts_with(&path) && path != watched)
    })
}

fn existing_ancestor(directory: &Path) -> PathBuf {
    let normalized = normalized_path(directory);
    let mut path = normalized.as_path();
    loop {
        if fs::metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
            return fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        }
        match path.parent() {
            Some(parent) => path = parent,
            None => return Path::new(if cfg!(windows) { "C:\\" } else { "/" }).to_path_buf(),
        }
    }
}

/// Canonicalize the deepest existing directory, then append the missing suffix.
/// Doing this for both requested and reported paths also normalizes macOS `/var` and
/// Windows `\\?\` spellings before prefix comparisons.
fn normalized_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }
    let mut suffix = Vec::<OsString>::new();
    let mut ancestor = path;
    loop {
        if fs::metadata(ancestor).is_ok_and(|metadata| metadata.is_dir()) {
            if let Ok(mut canonical) = fs::canonicalize(ancestor) {
                for part in suffix.iter().rev() {
                    append_component(&mut canonical, part);
                }
                return canonical;
            }
        }
        if let Some(name) = ancestor.file_name() {
            suffix.push(name.to_os_string());
        }
        match ancestor.parent() {
            Some(parent) => ancestor = parent,
            None => return path.to_path_buf(),
        }
    }
}

fn append_component(path: &mut PathBuf, component: &OsStr) {
    if component == OsStr::new(".") {
        return;
    }
    if component == OsStr::new("..") {
        path.pop();
        return;
    }
    path.push(component);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::mpsc, time::Instant};
    use tempfile::tempdir;

    #[test]
    fn normalizes_existing_ancestor_and_missing_suffix() {
        let base = tempdir().unwrap();
        let existing = base.path().join("one");
        fs::create_dir(&existing).unwrap();
        let requested = existing.join("two/Themes/custom.json");
        let expected = fs::canonicalize(&existing)
            .unwrap()
            .join("two/Themes/custom.json");
        assert_eq!(normalized_path(&requested), expected);
    }

    #[cfg(unix)]
    #[test]
    fn normalizes_symlinked_parents_for_existing_and_deleted_event_paths() {
        use std::os::unix::fs::symlink;
        let base = tempdir().unwrap();
        let real = base.path().join("real");
        let alias = base.path().join("alias");
        fs::create_dir_all(real.join("Themes")).unwrap();
        symlink(&real, &alias).unwrap();
        let actual = alias.join("Themes");
        let expected = fs::canonicalize(real.join("Themes")).unwrap();
        assert_eq!(normalized_path(&actual), expected);

        let removed = alias.join("Themes/removed.json");
        assert_eq!(
            normalized_path(&removed),
            fs::canonicalize(real.join("Themes"))
                .unwrap()
                .join("removed.json")
        );
    }

    #[cfg(unix)]
    #[test]
    fn matches_canonical_event_paths_against_symlinked_watch_targets() {
        use std::os::unix::fs::symlink;
        let base = tempdir().unwrap();
        let real = base.path().join("real");
        let alias = base.path().join("alias");
        fs::create_dir(&real).unwrap();
        symlink(&real, &alias).unwrap();
        let directory = alias.join("one/two/Themes");
        let watched = existing_ancestor(&directory);
        let event =
            notify::Event::new(notify::EventKind::Create(notify::event::CreateKind::Folder))
                .add_path(real.join("one"));
        assert!(relevant(&directory, &watched, &event));
    }

    fn wait_for(
        receiver: &Receiver<(usize, Option<String>)>,
        expected: usize,
    ) -> (usize, Option<String>) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let event = receiver
                .recv_timeout(left)
                .expect("notify event should reach watcher");
            if event.0 >= expected {
                return event;
            }
        }
    }

    fn theme_json(name: &str) -> String {
        serde_json::json!({"schemaVersion":1,"name":name,"author":"Test","dark":false,"extends":"light","colors":{}}).to_string()
    }

    #[test]
    fn watches_late_directory_file_changes_deletions_and_debounces() {
        let base = tempdir().unwrap();
        let directory = base.path().join("Themes");
        let (sender, receiver) = mpsc::channel();
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let watch_count = count.clone();
        let callback_directory = directory.clone();
        let _watch = ThemeWatch::start(directory.clone(), move || {
            let next = watch_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let catalog = crate::theme::load_user_themes(&callback_directory);
            let active = catalog.themes.first().map(|theme| theme.theme.name.clone());
            let _ = sender.send((next, active));
        })
        .unwrap();

        fs::create_dir(&directory).unwrap();
        assert_eq!(wait_for(&receiver, 1), (1, None));
        let file = directory.join("custom.json");
        fs::write(&file, theme_json("First")).unwrap();
        assert_eq!(wait_for(&receiver, 2), (2, Some("First".into())));
        fs::write(&file, theme_json("Second")).unwrap();
        assert_eq!(wait_for(&receiver, 3), (3, Some("Second".into())));
        fs::write(&file, theme_json("Third")).unwrap();
        fs::write(&file, theme_json("Fourth")).unwrap();
        assert_eq!(wait_for(&receiver, 4), (4, Some("Fourth".into())));
        assert!(
            receiver.recv_timeout(Duration::from_millis(450)).is_err(),
            "rapid file writes should debounce"
        );

        fs::remove_file(&file).unwrap();
        assert_eq!(wait_for(&receiver, 5), (5, None));
        fs::remove_dir(&directory).unwrap();
        assert_eq!(wait_for(&receiver, 6), (6, None));
        fs::create_dir(&directory).unwrap();
        assert_eq!(wait_for(&receiver, 7), (7, None));
    }

    #[test]
    fn missing_ancestors_are_watched_as_they_are_created() {
        let base = tempdir().unwrap();
        let directory = base.path().join("one/two/Themes");
        let (sender, receiver) = mpsc::channel();
        let _watch = ThemeWatch::start(directory.clone(), move || {
            let _ = sender.send(());
        })
        .unwrap();
        fs::create_dir(base.path().join("one")).unwrap();
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        fs::create_dir(base.path().join("one/two")).unwrap();
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        fs::create_dir(&directory).unwrap();
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    }
}
