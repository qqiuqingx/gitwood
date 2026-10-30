//! Frontend-session inclusion rules. Directory rules operate on raw path prefixes.
use super::{file_tree::FileNode, status, RepoPath};
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct Selection {
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub hunks: Vec<Hunks>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Rule {
    pub id: String,
    pub included: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Hunks {
    pub path: RepoPath,
    pub token: String,
    pub blocks: Vec<usize>,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Inclusion {
    All,
    Some,
    None,
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn matches(id: &str, path: &RepoPath, untracked: bool) -> bool {
    let group = if untracked { "unversioned" } else { "changes" };
    let file = if untracked { "untracked" } else { "change" };
    id == group
        || id == format!("{file}/{}", hex(&path.bytes))
        || id
            .strip_prefix(&format!("{group}/dir/"))
            .is_some_and(|prefix| hex(&path.bytes).starts_with(prefix))
}
impl Selection {
    pub fn file(&self, path: &RepoPath, untracked: bool) -> Inclusion {
        if let Some(h) = self.hunks.iter().find(|h| h.path.bytes == path.bytes) {
            return if h.blocks.is_empty() {
                Inclusion::None
            } else {
                Inclusion::Some
            };
        }
        let included = self
            .rules
            .iter()
            .rev()
            .find(|r| matches(&r.id, path, untracked))
            .is_some_and(|r| r.included);
        if included {
            Inclusion::All
        } else {
            Inclusion::None
        }
    }
    pub fn node(&self, node: &FileNode, entries: &[(status::Entry, bool)]) -> Inclusion {
        self.id(&node.id, entries)
    }
    pub fn id(&self, id: &str, entries: &[(status::Entry, bool)]) -> Inclusion {
        let mut all = true;
        let mut any = false;
        for (e, untracked) in entries.iter().filter(|(e, u)| matches(id, &e.path, *u)) {
            let state = self.file(&e.path, *untracked);
            all &= state == Inclusion::All;
            any |= state != Inclusion::None;
        }
        if all && any {
            Inclusion::All
        } else if any {
            Inclusion::Some
        } else {
            Inclusion::None
        }
    }
}
pub fn entries(status: &status::Status) -> Vec<(status::Entry, bool)> {
    let mut result = std::collections::BTreeMap::new();
    for e in status
        .unstaged
        .iter()
        .chain(&status.staged)
        .chain(&status.conflicts)
    {
        result.insert(e.path.bytes.clone(), (e.clone(), false));
    }
    for e in &status.untracked {
        result.insert(e.path.bytes.clone(), (e.clone(), true));
    }
    result.into_values().collect()
}
