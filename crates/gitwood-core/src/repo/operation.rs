//! In-progress Git operations read from the per-worktree Git directory without running Git.
//!
//! Order follows IDEA `GitRepositoryReader.readRepositoryState`: merge, rebase, cherry-pick,
//! revert, then the sequencer todo (a multi-commit pick/revert whose HEAD file is gone after
//! `git commit`). Bisect is reported last; it does not block history edits.
use serde::Serialize;
use std::{fs, io, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    Merge,
    Rebase,
    /// `git am` also uses `rebase-apply`; it is continued with `git am`, not `git rebase`.
    ApplyMailbox,
    CherryPick,
    Revert,
    Bisect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub kind: OperationKind,
    /// Branch being rebased (`head-name`), when not detached.
    pub branch: Option<String>,
    /// 1-based step and total for rebase / am.
    pub step: Option<u32>,
    pub total: Option<u32>,
}

impl Operation {
    fn new(kind: OperationKind) -> Self {
        Self {
            kind,
            branch: None,
            step: None,
            total: None,
        }
    }
    /// Commit-editing actions (Undo, Revert, Reset, Cherry-pick…) are unavailable meanwhile.
    pub fn blocks_history_edit(&self) -> bool {
        self.kind != OperationKind::Bisect
    }
}

/// `git_dir` is the worktree's own Git directory (`--git-dir`), not the common directory.
pub fn read(git_dir: &Path) -> io::Result<Option<Operation>> {
    if present(&git_dir.join("MERGE_HEAD"))? {
        return Ok(Some(Operation::new(OperationKind::Merge)));
    }
    for (dir, step, total) in [
        ("rebase-merge", "msgnum", "end"),
        ("rebase-apply", "next", "last"),
    ] {
        let dir = git_dir.join(dir);
        if !present(&dir)? {
            continue;
        }
        let kind = if present(&dir.join("applying"))? {
            OperationKind::ApplyMailbox
        } else {
            OperationKind::Rebase
        };
        return Ok(Some(Operation {
            kind,
            branch: small_text(&dir.join("head-name"))?
                .and_then(|name| name.strip_prefix("refs/heads/").map(Into::into)),
            step: number(&dir.join(step))?,
            total: number(&dir.join(total))?,
        }));
    }
    if present(&git_dir.join("CHERRY_PICK_HEAD"))? {
        return Ok(Some(Operation::new(OperationKind::CherryPick)));
    }
    if present(&git_dir.join("REVERT_HEAD"))? {
        return Ok(Some(Operation::new(OperationKind::Revert)));
    }
    let todo = git_dir.join("sequencer");
    if present(&todo)? {
        // A sequencer directory without a recognizable todo still blocks history edits.
        let first = small_text(&todo.join("todo"))?.unwrap_or_default();
        return Ok(Some(Operation::new(if first.starts_with("revert") {
            OperationKind::Revert
        } else {
            OperationKind::CherryPick
        })));
    }
    if present(&git_dir.join("BISECT_LOG"))? {
        return Ok(Some(Operation::new(OperationKind::Bisect)));
    }
    Ok(None)
}

/// Files whose creation or removal changes [`read`]; the watcher refreshes status for them.
pub fn is_state_path(relative: &Path) -> bool {
    [
        "MERGE_HEAD",
        "rebase-merge",
        "rebase-apply",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "sequencer",
        "BISECT_LOG",
    ]
    .iter()
    .any(|name| relative.starts_with(name))
}

/// Reads at most one small state file; directories and missing files read as `None`.
fn small_text(path: &Path) -> io::Result<Option<String>> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() && meta.len() <= 4096 => Ok(Some(
            String::from_utf8_lossy(&fs::read(path)?).trim().into(),
        )),
        Ok(_) => Ok(None),
        Err(e) if missing(&e) => Ok(None),
        Err(e) => Err(e),
    }
}
/// A marker may be a file or a directory; a path below a file counts as absent.
fn present(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if missing(&e) => Ok(false),
        Err(e) => Err(e),
    }
}
fn missing(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}
fn number(path: &Path) -> io::Result<Option<u32>> {
    Ok(small_text(path)?.and_then(|text| text.parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn order_and_details_follow_git_files() {
        let dir = tempfile::tempdir().unwrap();
        let git = dir.path();
        assert_eq!(read(git).unwrap(), None);
        fs::write(git.join("BISECT_LOG"), "").unwrap();
        assert_eq!(read(git).unwrap().unwrap().kind, OperationKind::Bisect);
        assert!(!read(git).unwrap().unwrap().blocks_history_edit());
        fs::create_dir(git.join("sequencer")).unwrap();
        fs::write(git.join("sequencer/todo"), "revert abc subject\n").unwrap();
        assert_eq!(read(git).unwrap().unwrap().kind, OperationKind::Revert);
        fs::write(git.join("CHERRY_PICK_HEAD"), "abc\n").unwrap();
        assert_eq!(read(git).unwrap().unwrap().kind, OperationKind::CherryPick);
        let rebase = git.join("rebase-merge");
        fs::create_dir(&rebase).unwrap();
        fs::write(rebase.join("head-name"), "refs/heads/topic\n").unwrap();
        fs::write(rebase.join("msgnum"), "2\n").unwrap();
        fs::write(rebase.join("end"), "5\n").unwrap();
        assert_eq!(
            read(git).unwrap().unwrap(),
            Operation {
                kind: OperationKind::Rebase,
                branch: Some("topic".into()),
                step: Some(2),
                total: Some(5),
            }
        );
        fs::write(git.join("MERGE_HEAD"), "abc\n").unwrap();
        assert_eq!(
            read(git).unwrap().unwrap(),
            Operation::new(OperationKind::Merge)
        );
        fs::remove_file(git.join("MERGE_HEAD")).unwrap();
        fs::remove_dir_all(&rebase).unwrap();
        let apply = git.join("rebase-apply");
        fs::create_dir(&apply).unwrap();
        fs::write(apply.join("applying"), "").unwrap();
        assert_eq!(
            read(git).unwrap().unwrap().kind,
            OperationKind::ApplyMailbox
        );
        assert!(is_state_path(Path::new("rebase-apply/next")));
        assert!(!is_state_path(Path::new("ORIG_HEAD")));
    }
}
