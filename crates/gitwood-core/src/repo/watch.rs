use super::Repository;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::{path::Path, sync::mpsc, time::Duration};
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Refresh {
    pub status: bool,
    pub refs: bool,
    pub history: bool,
}
impl Refresh {
    pub fn all() -> Self {
        Self {
            status: true,
            refs: true,
            history: true,
        }
    }
    fn merge(&mut self, other: Self) {
        self.status |= other.status;
        self.refs |= other.refs;
        self.history |= other.history;
    }
    fn any(self) -> bool {
        self.status || self.refs || self.history
    }
}
pub struct RepositoryWatch {
    watcher: Option<RecommendedWatcher>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl RepositoryWatch {
    pub fn start(
        repo: Repository,
        emit: impl Fn(Refresh) + Send + 'static,
    ) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |e| {
            let _ = tx.send(e);
        })?;
        watcher.watch(&repo.root, RecursiveMode::Recursive)?;
        let mut dirs = vec![repo.git_dir.clone(), repo.common_dir.clone()];
        dirs.sort();
        dirs.dedup();
        for dir in dirs {
            if !dir.starts_with(&repo.root) {
                watcher.watch(&dir, RecursiveMode::Recursive)?;
            }
        }
        let worker = std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                let mut pending = classify_event(&repo, event);
                loop {
                    match rx.recv_timeout(Duration::from_millis(300)) {
                        Ok(e) => pending.merge(classify_event(&repo, e)),
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                if pending.any() {
                    emit(pending);
                }
            }
        });
        Ok(Self {
            watcher: Some(watcher),
            worker: Some(worker),
        })
    }
}
impl Drop for RepositoryWatch {
    fn drop(&mut self) {
        drop(self.watcher.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn classify_event(repo: &Repository, event: notify::Result<notify::Event>) -> Refresh {
    match event {
        Ok(e) => {
            if matches!(e.kind, notify::EventKind::Access(_)) {
                return Refresh::default();
            }
            let mut r = Refresh::default();
            for path in e.paths {
                r.merge(classify(repo, &path));
            }
            r
        }
        Err(_) => Refresh::all(),
    }
}
fn classify(repo: &Repository, path: &Path) -> Refresh {
    if path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().ends_with(".lock"))
    {
        return Refresh::default();
    }
    for dir in [&repo.git_dir, &repo.common_dir] {
        if let Ok(p) = path.strip_prefix(dir) {
            if p.starts_with("objects") || p.starts_with("logs") {
                return Refresh::default();
            }
            if p == Path::new("HEAD") || p.starts_with("refs") || p == Path::new("packed-refs") {
                return Refresh::all();
            }
            if p == Path::new("index") {
                return Refresh {
                    status: true,
                    ..Refresh::default()
                };
            }
            if p == Path::new("config") || p == Path::new("config.worktree") {
                return Refresh::all();
            }
            // Merge / rebase / cherry-pick state is shown with the status summary.
            if dir == &repo.git_dir && super::operation::is_state_path(p) {
                return Refresh {
                    status: true,
                    ..Refresh::default()
                };
            }
            return Refresh::default();
        }
    }
    if path == repo.root.join(".git") {
        return Refresh::all();
    }
    Refresh {
        status: true,
        ..Refresh::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn git_regions_and_worktree_are_separated() {
        let root = std::env::temp_dir().join("gitwood-classify");
        let repo = Repository {
            root: root.clone(),
            git_dir: root.join(".git/worktrees/linked"),
            common_dir: root.join(".git"),
            linked: true,
        };
        for p in [
            ".git/objects/ab/cd",
            ".git/logs/HEAD",
            ".git/index.lock",
            "file.lock",
        ] {
            assert!(!classify(&repo, &root.join(p)).any(), "{p}");
        }
        for p in [
            ".git/refs/heads/main",
            ".git/packed-refs",
            ".git/worktrees/linked/HEAD",
        ] {
            let r = classify(&repo, &root.join(p));
            assert!(r.status && r.refs && r.history, "{p}");
        }
        let r = classify(&repo, &root.join(".git/worktrees/linked/index"));
        assert!(r.status && !r.refs && !r.history);
        let r = classify(&repo, &root.join(".git/worktrees/linked/MERGE_HEAD"));
        assert!(r.status && !r.refs && !r.history);
        assert!(!classify(&repo, &root.join(".git/MERGE_HEAD")).any());
        assert!(classify(&repo, &root.join("source.rs")).status);
    }
}
