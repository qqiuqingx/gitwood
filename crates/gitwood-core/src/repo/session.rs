//! One active repository; immutable generations prevent cross-repository cache/results.
use super::{invalid, refs, status, Repository};
use crate::{
    git::{CancellationToken, GitError, GitRunner, RunOptions},
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
#[derive(Default)]
pub(super) struct Data {
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
    pub untracked: usize,
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
    fn ensure_current(&self) -> Result<(), GitError> {
        if self.options.cancellation.is_cancelled() {
            Err(invalid("repository.stale"))
        } else {
            Ok(())
        }
    }

    pub async fn status(&self) -> Result<StatusSummary, GitError> {
        let before = crate::ops::snapshot(&self.runner, &self.options).await?;
        let status = status::read(&self.runner, &self.options).await?;
        let after = crate::ops::snapshot(&self.runner, &self.options).await?;
        if before != after {
            return Err(invalid("repository changed while reading status; refresh"));
        }
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
            untracked: status.untracked.len(),
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
