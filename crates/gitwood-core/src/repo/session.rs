//! One active repository; immutable generations prevent cross-repository cache/results.
use super::{invalid, refs, status, Repository};
use crate::{
    git::{CancellationToken, GitError, GitRunner, ReadCommand, ReadOperation, RunOptions},
    history::{
        details::{self, Commit, Filter},
        CommitId, Direction, GraphLayout,
    },
};
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
#[derive(Clone, Default)]
pub struct Sessions(Arc<Mutex<Active>>);
#[derive(Default)]
struct Active {
    generation: u64,
    lease: Option<Lease>,
    watch: Option<super::watch::RepositoryWatch>,
    watch_epoch: u64,
}
#[derive(Clone)]
pub struct Lease {
    pub generation: u64,
    pub repository: Repository,
    pub(super) runner: GitRunner,
    pub(super) options: RunOptions,
    pub(super) data: Arc<tokio::sync::Mutex<Data>>,
    pub(super) writes: Arc<super::write::Writes>,
}

impl Lease {
    /// Read one configured remote URL without exposing it in errors or logs.
    pub async fn hosting_remote_url(&self, remote: &str) -> Result<String, GitError> {
        if remote.is_empty()
            || remote.len() > 128
            || !remote
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        {
            return Err(invalid("hosting.remoteUnavailable"));
        }
        let result = self
            .runner
            .run(
                ReadCommand::new(ReadOperation::RemoteUrl).args(["get-url", remote]),
                &self.options,
            )
            .await
            .map_err(|_| invalid("hosting.remoteUnavailable"))?;
        let value =
            String::from_utf8(result.stdout).map_err(|_| invalid("hosting.remoteUnavailable"))?;
        let mut urls = value.lines();
        let Some(url) = urls.next() else {
            return Err(invalid("hosting.remoteUnavailable"));
        };
        if urls.next().is_some() || url.is_empty() {
            return Err(invalid("hosting.remoteUnavailable"));
        }
        Ok(url.to_owned())
    }

    /// Check local remote-tracking refs only; this never contacts the hosting service.
    pub async fn hosting_revision_published(&self, remote: &str, revision: &str) -> bool {
        if !valid_hosting_remote(remote) || !valid_object_id(revision) {
            return false;
        }
        let contains = format!("--contains={revision}");
        let prefix = format!("refs/remotes/{remote}/");
        self.runner
            .run(
                ReadCommand::new(ReadOperation::HostingContains).args([
                    "--count=1",
                    "--format=%(refname)",
                    contains.as_str(),
                    prefix.as_str(),
                ]),
                &self.options,
            )
            .await
            .is_ok_and(|output| !output.stdout.is_empty())
    }

    pub async fn hosting_branch_published(&self, remote: &str, branch: &str) -> bool {
        if !valid_hosting_remote(remote) || !valid_hosting_branch(branch) {
            return false;
        }
        let Some(revision) = self.hosting_ref_oid(&format!("refs/heads/{branch}")).await else {
            return false;
        };
        self.hosting_revision_published(remote, &revision).await
    }

    pub async fn hosting_remote_branch_exists(&self, remote: &str, branch: &str) -> bool {
        if !valid_hosting_remote(remote) || !valid_hosting_branch(branch) {
            return false;
        }
        self.hosting_ref_oid(&format!("refs/remotes/{remote}/{branch}"))
            .await
            .is_some()
    }

    async fn hosting_ref_oid(&self, reference: &str) -> Option<String> {
        let output = self
            .runner
            .run(
                ReadCommand::new(ReadOperation::HostingRef).args([
                    "--count=1",
                    "--format=%(objectname)",
                    reference,
                ]),
                &self.options,
            )
            .await
            .ok()?;
        let value = String::from_utf8(output.stdout).ok()?;
        let mut lines = value.lines();
        let oid = lines.next()?;
        if lines.next().is_some() || !valid_object_id(oid) {
            return None;
        }
        Some(oid.to_owned())
    }
}

fn valid_hosting_remote(remote: &str) -> bool {
    !remote.is_empty()
        && remote.len() <= 128
        && !remote.starts_with('-')
        && !remote.starts_with('.')
        && !remote.ends_with('.')
        && !remote.contains("..")
        && remote
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn valid_hosting_branch(branch: &str) -> bool {
    !branch.is_empty()
        && branch.len() <= 1024
        && !branch.contains("..")
        && !branch.contains("@{")
        && !branch.ends_with('.')
        && !branch
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || "~^:?*[\\".contains(ch))
        && branch.split('/').all(|part| {
            !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock") && part != "@"
        })
}

fn valid_object_id(value: &str) -> bool {
    [40, 64].contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
#[derive(Default)]
pub(super) struct Data {
    pub(super) comparison: Option<Arc<super::comparison::Comparison>>,
    pub(super) history_tree: Option<Arc<super::history_browser::HistoryTree>>,
    pub(super) file_history: Option<Arc<super::file_history::FileHistory>>,
    pub(super) annotation: Option<Arc<super::annotate::Annotation>>,
    pub(super) annotation_cancel: Option<CancellationToken>,
    layout: Option<GraphLayout>,
    /// Per layout row: reachable from the current branch (empty = no highlight).
    current_branch: Vec<bool>,
    layout_generation: Option<u64>,
    pub(super) status: Option<status::Status>,
    status_trees: [Option<super::file_tree::FileTree>; 2],
    cache: VecDeque<Commit>,
    files: Option<(
        String,
        Arc<Vec<details::FileChange>>,
        super::file_tree::FileTree,
    )>,
}
#[derive(Serialize)]
pub struct StatusSummary {
    pub branch: status::Branch,
    pub changes: usize,
    pub staged: usize,
    pub untracked: usize,
    pub conflicts: usize,
    pub conflict_files: Vec<crate::ops::conflict::ConflictFile>,
    pub merge_message: Option<String>,
    pub update_stash: Option<String>,
    /// Merge, rebase, cherry-pick, revert or bisect in progress.
    pub operation: Option<super::operation::Operation>,
    pub fingerprint: String,
}
#[derive(Serialize)]
pub struct LayoutInfo {
    pub generation: u64,
    pub total: usize,
}
#[derive(Serialize)]
pub struct Rows {
    pub generation: u64,
    pub nodes: Vec<(u32, u32, i32, u32, u32)>,
    pub segments: Vec<(u32, u32, u32, i32, i8, bool)>,
    pub commits: Vec<Commit>,
    /// Parallel to `nodes`: commit is contained in the current branch.
    pub current: Vec<bool>,
}
impl Sessions {
    pub fn generation(&self) -> u64 {
        self.0.lock().unwrap().generation
    }
    pub fn close(&self) -> u64 {
        let mut a = self.0.lock().unwrap();
        if let Some(l) = a.lease.take() {
            l.options.cancellation.cancel();
            l.writes.cancel();
        }
        let watch = a.watch.take();
        a.watch_epoch = 0;
        a.generation += 1;
        let generation = a.generation;
        drop(a);
        drop(watch);
        generation
    }
    pub async fn open(
        &self,
        runner: GitRunner,
        mut options: RunOptions,
    ) -> Result<Lease, GitError> {
        let generation = self.close();
        options.cancellation = CancellationToken::new();
        // Install the cancellation token before discovery so a concurrent switch cancels it.
        let placeholder = Repository {
            root: options.cwd.clone(),
            git_dir: Default::default(),
            common_dir: Default::default(),
            linked: false,
        };
        let mut lease = Lease {
            generation,
            repository: placeholder,
            runner,
            options,
            data: Default::default(),
            writes: Default::default(),
        };
        {
            let mut a = self.0.lock().unwrap();
            if a.generation != generation {
                return Err(invalid("repository.stale"));
            }
            a.lease = Some(lease.clone());
        }
        let opened = Repository::open(&lease.runner, &lease.options).await;
        let mut active = self.0.lock().unwrap();
        if active.generation != generation {
            return Err(invalid("repository.stale"));
        }
        match opened {
            Ok(repo) => {
                lease.options.cwd = repo.root.clone();
                lease.repository = repo;
                active.lease = Some(lease.clone());
                Ok(lease)
            }
            Err(e) => {
                active.lease = None;
                Err(e)
            }
        }
    }
    pub fn lease(&self, generation: u64) -> Result<Lease, GitError> {
        let a = self.0.lock().unwrap();
        if a.generation != generation {
            return Err(invalid("repository.stale"));
        }
        a.lease.clone().ok_or_else(|| invalid("repository.notOpen"))
    }
    pub fn check(&self, l: &Lease) -> Result<(), GitError> {
        if self.generation() != l.generation {
            Err(invalid("repository.stale"))
        } else {
            Ok(())
        }
    }
    pub fn refresh(&self, generation: u64) -> Result<u64, GitError> {
        let mut a = self.0.lock().unwrap();
        if a.generation != generation {
            return Err(invalid("repository.stale"));
        }
        let mut l = a
            .lease
            .take()
            .ok_or_else(|| invalid("repository.notOpen"))?;
        if l.writes.busy() {
            a.lease = Some(l);
            return Err(invalid("repository.writeRunning"));
        }
        l.options.cancellation.cancel();
        l.options.cancellation = CancellationToken::new();
        a.generation += 1;
        l.generation = a.generation;
        a.lease = Some(l);
        Ok(a.generation)
    }
    pub fn watch(
        &self,
        generation: u64,
        emit: impl Fn(u64, super::watch::Refresh) + Send + 'static,
    ) -> Result<(), GitError> {
        let l = self.lease(generation)?;
        let weak = Arc::downgrade(&self.0);
        let watch = super::watch::RepositoryWatch::start(l.repository, move |areas| {
            if let Some(active) = weak.upgrade() {
                let a = active.lock().unwrap();
                if a.watch_epoch != generation {
                    return;
                }
                let current = a.generation;
                drop(a);
                emit(current, areas);
            }
        })
        .map_err(|e| invalid(&e.to_string()))?;
        let mut a = self.0.lock().unwrap();
        if a.generation != generation {
            return Err(invalid("repository.stale"));
        }
        a.watch_epoch = generation;
        let old = a.watch.replace(watch);
        drop(a);
        drop(old);
        Ok(())
    }
}

impl Lease {
    pub(super) fn ensure_current(&self) -> Result<(), GitError> {
        if self.options.cancellation.is_cancelled() {
            Err(invalid("repository.stale"))
        } else {
            Ok(())
        }
    }

    pub async fn status(&self) -> Result<StatusSummary, GitError> {
        let before = crate::ops::snapshot(&self.runner, &self.options).await?;
        let status = status::read(&self.runner, &self.options).await?;
        let conflict_files =
            crate::ops::conflict::files(&self.runner, &self.options, &status.conflicts).await?;
        let merge_message = match std::fs::read_to_string(self.repository.git_dir.join("MERGE_MSG"))
        {
            Ok(message) => Some(message),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let after = crate::ops::snapshot(&self.runner, &self.options).await?;
        if before != after {
            return Err(invalid("repository changed while reading status; refresh"));
        }
        let operation = super::operation::read(&self.repository.git_dir)
            .map_err(|e| invalid(&format!("repository.operationState: {e}")))?;
        let changes = status
            .staged
            .iter()
            .chain(&status.unstaged)
            .chain(&status.conflicts)
            .map(|entry| &entry.path.bytes)
            .collect::<std::collections::HashSet<_>>()
            .len();
        let summary = StatusSummary {
            branch: status::Branch {
                oid: status.branch.oid.clone(),
                head: status.branch.head.clone(),
                upstream: status.branch.upstream.clone(),
                ahead: status.branch.ahead,
                behind: status.branch.behind,
            },
            changes,
            staged: status.staged.len(),
            untracked: status.untracked.len(),
            conflicts: status.conflicts.len(),
            conflict_files,
            merge_message,
            update_stash: crate::ops::update_recovery::pending(&self.repository.git_dir)?,
            operation,
            fingerprint: after.fingerprint,
        };
        let mut data = self.data.lock().await;
        self.ensure_current()?;
        data.status = Some(status);
        data.status_trees = [None, None];
        Ok(summary)
    }
    pub async fn changes_tree(
        &self,
        expanded: &std::collections::HashSet<String>,
        flat: bool,
        start: usize,
        end: usize,
        locate: &[String],
    ) -> Result<super::file_tree::FileTreePage, GitError> {
        if end < start || end - start > 200 || locate.len() > 2 {
            return Err(invalid("page exceeds 200 tree rows"));
        }
        let mut data = self.data.lock().await;
        self.ensure_current()?;
        let index = usize::from(flat);
        if data.status_trees[index].is_none() {
            let status = data
                .status
                .as_ref()
                .ok_or_else(|| invalid("repository.statusRequired"))?;
            data.status_trees[index] = Some(super::file_tree::FileTree::changes(status, flat));
        }
        Ok(data.status_trees[index]
            .as_ref()
            .unwrap()
            .page(expanded, start, end, locate))
    }
    pub async fn refs(&self) -> Result<refs::References, GitError> {
        refs::read(&self.runner, &self.options).await
    }
    pub async fn layout(&self, filter: &Filter) -> Result<LayoutInfo, GitError> {
        let layout = details::layout(&self.runner, &self.options, filter).await?;
        let current_branch =
            details::current_branch_rows(&self.runner, &self.options, filter, &layout).await?;
        let total = layout.graph().len();
        let mut d = self.data.lock().await;
        self.ensure_current()?;
        d.layout_generation = Some(self.generation);
        d.layout = Some(layout);
        d.current_branch = current_branch;
        d.cache.clear();
        Ok(LayoutInfo {
            generation: self.generation,
            total,
        })
    }
    pub async fn rows(&self, start: usize, end: usize) -> Result<Rows, GitError> {
        if end.saturating_sub(start) > 200 {
            return Err(invalid("page exceeds 200 rows"));
        }
        let mut d = self.data.lock().await;
        self.ensure_current()?;
        if d.layout_generation != Some(self.generation) {
            return Err(invalid("history.layoutRequired"));
        }
        let layout = d
            .layout
            .as_ref()
            .ok_or_else(|| invalid("history.layoutRequired"))?;
        let visible = layout
            .visible_rows(start, end)
            .map_err(|e| invalid(&e.to_string()))?;
        let hashes: Vec<_> = visible
            .rows
            .iter()
            .filter_map(|row| layout.graph().hash(CommitId(row.commit.0)))
            .collect();
        let current = visible
            .rows
            .iter()
            .map(|r| {
                d.current_branch
                    .get(r.commit.0 as usize)
                    .copied()
                    .unwrap_or(false)
            })
            .collect();
        let nodes = visible
            .rows
            .iter()
            .map(|r| {
                (
                    r.commit.0,
                    r.column,
                    r.color.0,
                    r.segments_start,
                    r.segments_end,
                )
            })
            .collect();
        let segments = visible
            .segments
            .iter()
            .map(|s| {
                (
                    s.edge.0,
                    s.from_column,
                    s.to_column,
                    s.color.0,
                    if s.direction == Direction::Up { -1 } else { 1 },
                    layout.edge_is_dotted(s.edge),
                )
            })
            .collect();
        let missing: Vec<_> = hashes
            .iter()
            .filter(|h| !d.cache.iter().any(|c| &c.hash == *h))
            .cloned()
            .collect();
        let fetched = details::page(&self.runner, &self.options, &missing, false).await?;
        self.ensure_current()?;
        d.cache.extend(fetched);
        let mut commits = vec![];
        for h in hashes {
            if let Some(i) = d.cache.iter().position(|c| c.hash == h) {
                let c = d.cache.remove(i).unwrap();
                commits.push(c.clone());
                d.cache.push_back(c);
            }
        }
        while d.cache.len() > 1000 {
            d.cache.pop_front();
        }
        Ok(Rows {
            generation: self.generation,
            nodes,
            segments,
            commits,
            current,
        })
    }
    pub async fn position(&self, hash: &str) -> Result<Option<u32>, GitError> {
        details::validate_hash(hash)?;
        let data = self.data.lock().await;
        self.ensure_current()?;
        if data.layout_generation != Some(self.generation) {
            return Err(invalid("history.layoutRequired"));
        }
        let layout = data
            .layout
            .as_ref()
            .ok_or_else(|| invalid("history.layoutRequired"))?;
        Ok(layout.graph().find_commit(hash).map(|id| id.0))
    }
    /// Commit IDs of half-open row ranges, for a log multi-selection over rows the UI has
    /// not loaded. Ranges come from one layout generation and must be in bounds.
    pub async fn hashes(&self, ranges: &[(u32, u32)]) -> Result<Vec<String>, GitError> {
        let data = self.data.lock().await;
        self.ensure_current()?;
        if data.layout_generation != Some(self.generation) {
            return Err(invalid("history.layoutRequired"));
        }
        let layout = data
            .layout
            .as_ref()
            .ok_or_else(|| invalid("history.layoutRequired"))?;
        let total = layout.graph().len();
        let mut result = vec![];
        for &(start, end) in ranges {
            if start > end || end as usize > total {
                return Err(invalid("history.rowOutOfRange"));
            }
            for index in start..end {
                let hash = layout
                    .graph()
                    .hash(CommitId(index))
                    .ok_or_else(|| invalid("history.rowOutOfRange"))?;
                result.push(hash);
            }
        }
        Ok(result)
    }
    pub async fn details(&self, hash: &str) -> Result<Commit, GitError> {
        details::page(&self.runner, &self.options, &[hash.into()], true)
            .await?
            .pop()
            .ok_or_else(|| invalid("missing commit"))
    }
    pub async fn files(&self, hash: &str) -> Result<Arc<Vec<details::FileChange>>, GitError> {
        let mut data = self.data.lock().await;
        if data
            .files
            .as_ref()
            .is_none_or(|(cached, _, _)| cached != hash)
        {
            let files = details::files(&self.runner, &self.options, hash).await?;
            self.ensure_current()?;
            let tree = super::file_tree::FileTree::new(&files);
            data.files = Some((hash.into(), Arc::new(files), tree));
        }
        Ok(Arc::clone(&data.files.as_ref().unwrap().1))
    }
    pub async fn file_tree(
        &self,
        hash: &str,
        expanded: &std::collections::HashSet<String>,
        start: usize,
        end: usize,
        locate: &[String],
    ) -> Result<super::file_tree::FileTreePage, GitError> {
        if end < start || end - start > 200 || locate.len() > 2 {
            return Err(invalid("page exceeds 200 tree rows"));
        }
        self.files(hash).await?;
        let data = self.data.lock().await;
        let (cached, _, tree) = data
            .files
            .as_ref()
            .ok_or_else(|| invalid("repository.stale"))?;
        if cached != hash {
            return Err(invalid("repository.stale"));
        }
        Ok(tree.page(expanded, start, end, locate))
    }
}

#[cfg(test)]
mod hosting_publication_tests {
    use super::{Lease, Repository, RunOptions};
    use crate::git::GitRunner;
    use std::{path::Path, process::Command};

    fn git(cwd: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn create_fixture(directory: &Path) -> (std::path::PathBuf, String, String) {
        let repository_path = directory.join("repo");
        let origin_path = directory.join("origin.git");
        let mirror_path = directory.join("mirror.git");
        std::fs::create_dir(&repository_path).unwrap();
        std::fs::create_dir(&origin_path).unwrap();
        std::fs::create_dir(&mirror_path).unwrap();
        git(&origin_path, &["init", "--bare"]);
        git(&mirror_path, &["init", "--bare"]);
        git(&repository_path, &["init", "--initial-branch=main"]);
        git(&repository_path, &["config", "user.name", "Gitwood Test"]);
        git(
            &repository_path,
            &["config", "user.email", "gitwood@example.invalid"],
        );
        std::fs::write(repository_path.join("file.txt"), "first\n").unwrap();
        git(&repository_path, &["add", "file.txt"]);
        git(&repository_path, &["commit", "-m", "first"]);
        let first = git(&repository_path, &["rev-parse", "HEAD"]);
        git(
            &repository_path,
            &["remote", "add", "origin", origin_path.to_str().unwrap()],
        );
        git(
            &repository_path,
            &["remote", "add", "mirror", mirror_path.to_str().unwrap()],
        );
        git(&repository_path, &["push", "-u", "origin", "main"]);
        git(&repository_path, &["fetch", "origin"]);
        git(&repository_path, &["branch", "side", &first]);
        git(&repository_path, &["push", "origin", "side"]);
        git(&repository_path, &["fetch", "origin"]);
        std::fs::write(repository_path.join("file.txt"), "second\n").unwrap();
        git(&repository_path, &["commit", "-am", "second"]);
        let second = git(&repository_path, &["rev-parse", "HEAD"]);
        git(
            &repository_path,
            &["push", "mirror", &format!("{first}:refs/heads/main")],
        );
        git(&repository_path, &["fetch", "mirror"]);
        (repository_path, first, second)
    }

    async fn open_lease(repository_path: &Path) -> Lease {
        let runner = GitRunner::discover(None).await.unwrap();
        let options = RunOptions::new(repository_path);
        let repository = Repository::open(&runner, &options).await.unwrap();
        Lease {
            generation: 1,
            repository,
            runner,
            options,
            data: Default::default(),
            writes: Default::default(),
        }
    }

    #[tokio::test]
    async fn checks_published_revisions_with_multiple_local_bare_remote_branches() {
        let directory = tempfile::tempdir().unwrap();
        let (repository_path, first, second) = create_fixture(directory.path());
        let lease = open_lease(&repository_path).await;
        assert!(lease.hosting_revision_published("origin", &first).await);
        assert!(!lease.hosting_revision_published("origin", &second).await);
        assert!(lease.hosting_revision_published("mirror", &first).await);
        assert!(!lease.hosting_revision_published("mirror", &second).await);
        assert!(lease.hosting_remote_branch_exists("origin", "side").await);
        assert!(lease.hosting_branch_published("origin", "side").await);
        assert!(
            !lease
                .hosting_remote_branch_exists("origin", "missing")
                .await
        );
        git(&repository_path, &["push", "origin", "main"]);
        git(&repository_path, &["fetch", "origin"]);
        assert!(lease.hosting_branch_published("origin", "main").await);
    }
}
