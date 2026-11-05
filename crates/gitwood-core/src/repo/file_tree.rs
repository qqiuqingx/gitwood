//! Byte-preserving directory index. Only a bounded visible page crosses IPC.
use super::{status, RepoPath};
use crate::history::details::FileChange;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileNode {
    pub id: String,
    pub label: String,
    pub has_children: bool,
    pub path: Option<RepoPath>,
    pub status: Option<String>,
    pub note: Option<String>,
    pub staged: bool,
    pub unstaged: bool,
    pub conflicted: bool,
    pub count: usize,
}
impl FileNode {
    fn folder(id: String, label: String) -> Self {
        Self {
            id,
            label,
            has_children: true,
            path: None,
            status: None,
            note: None,
            staged: false,
            unstaged: false,
            conflicted: false,
            count: 0,
        }
    }
    fn file(group: &str, path: &RepoPath, status: String, original: Option<&RepoPath>) -> Self {
        Self {
            id: format!("{group}/{}", hex(&path.bytes)),
            label: path.display.clone(),
            has_children: false,
            path: Some(path.clone()),
            status: Some(status),
            note: original.map(|path| path.display.clone()),
            staged: false,
            unstaged: false,
            conflicted: false,
            count: 0,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRow {
    #[serde(flatten)]
    pub node: FileNode,
    pub parent_index: Option<usize>,
    pub level: usize,
    pub expanded: Option<bool>,
    pub pos_in_set: usize,
    pub set_size: usize,
}
#[derive(Serialize)]
pub struct FileTreePage {
    pub total: usize,
    pub files: usize,
    pub start: usize,
    pub rows: Vec<FileRow>,
    pub positions: Vec<Option<usize>>,
}
#[derive(Default)]
pub struct FileTree {
    nodes: Vec<FileNode>,
    children: Vec<Vec<usize>>,
    roots: Vec<usize>,
    files: usize,
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::new();
    for byte in bytes {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}
impl FileTree {
    pub fn new(files: &[FileChange]) -> Self {
        let mut tree = Self::default();
        tree.group(
            "files",
            files
                .iter()
                .map(|file| {
                    FileNode::file(
                        "file",
                        &file.path,
                        file.status.clone(),
                        file.original.as_ref(),
                    )
                })
                .collect(),
            false,
        );
        tree
    }
    pub fn changes(status: &status::Status, flat: bool) -> Self {
        let mut files: BTreeMap<Vec<u8>, FileNode> = BTreeMap::new();
        for (entries, side) in [(&status.staged, 0), (&status.unstaged, 1)] {
            for entry in entries {
                let file = files.entry(entry.path.bytes.clone()).or_insert_with(|| {
                    FileNode::file(
                        "change",
                        &entry.path,
                        entry.xy.clone(),
                        entry.original.as_ref(),
                    )
                });
                match side {
                    0 => file.staged = true,
                    1 => file.unstaged = true,
                    _ => file.conflicted = true,
                }
            }
        }
        let mut tree = Self::default();
        tree.group("changes", files.into_values().collect(), flat);
        if !status.conflicts.is_empty() {
            tree.group(
                "conflicts",
                status
                    .conflicts
                    .iter()
                    .map(|e| {
                        let mut file = FileNode::file("conflict", &e.path, e.xy.clone(), None);
                        file.conflicted = true;
                        file
                    })
                    .collect(),
                flat,
            );
        }
        tree.group(
            "unversioned",
            status
                .untracked
                .iter()
                .map(|entry| FileNode::file("untracked", &entry.path, "?".into(), None))
                .collect(),
            flat,
        );
        tree
    }
    fn group(&mut self, id: &str, files: Vec<FileNode>, flat: bool) {
        let mut root = FileNode::folder(id.into(), String::new());
        root.has_children = !files.is_empty();
        root.count = files.len();
        self.files += files.len();
        let root_index = self.add(root, None);
        let mut directories = HashMap::new();
        for mut file in files {
            let path = file.path.as_ref().unwrap();
            let parts: Vec<_> = path.bytes.split(|byte| *byte == b'/').collect();
            let mut parent = root_index;
            let mut prefix = vec![];
            if !flat {
                for part in &parts[..parts.len() - 1] {
                    prefix.extend_from_slice(part);
                    prefix.push(b'/');
                    parent = *directories.entry(prefix.clone()).or_insert_with(|| {
                        self.add(
                            FileNode::folder(
                                format!("{id}/dir/{}", hex(&prefix)),
                                String::from_utf8_lossy(part).into(),
                            ),
                            Some(parent),
                        )
                    });
                }
                file.label = String::from_utf8_lossy(parts.last().unwrap()).into();
            }
            self.add(file, Some(parent));
        }
        self.compact_directories(root_index);
    }
    fn compact_directories(&mut self, root: usize) {
        for index in (root + 1..self.nodes.len()).rev() {
            self.nodes[index].count = if self.nodes[index].has_children {
                self.children[index]
                    .iter()
                    .map(|&child| self.nodes[child].count)
                    .sum()
            } else {
                1
            };
        }
        let mut stack = self.children[root].clone();
        while let Some(index) = stack.pop() {
            while self.children[index].len() == 1 {
                let child = self.children[index][0];
                if !self.nodes[child].has_children {
                    break;
                }
                // The displayed directory uses the final raw-path ID so selection rules
                // still address exactly the directory represented by the compact label.
                self.nodes[index].id = std::mem::take(&mut self.nodes[child].id);
                let label = std::mem::take(&mut self.nodes[child].label);
                self.nodes[index].label.push('/');
                self.nodes[index].label.push_str(&label);
                self.children[index] = std::mem::take(&mut self.children[child]);
            }
            stack.extend_from_slice(&self.children[index]);
        }
    }
    fn add(&mut self, node: FileNode, parent: Option<usize>) -> usize {
        let index = self.nodes.len();
        self.nodes.push(node);
        self.children.push(vec![]);
        if let Some(parent) = parent {
            self.children[parent].push(index);
        } else {
            self.roots.push(index);
        }
        index
    }
    pub fn page(
        &self,
        expanded: &HashSet<String>,
        start: usize,
        end: usize,
        locate: &[String],
    ) -> FileTreePage {
        let mut stack: Vec<_> = self
            .roots
            .iter()
            .enumerate()
            .rev()
            .map(|(position, &index)| (index, None, 1, position + 1, self.roots.len()))
            .collect();
        let mut total = 0;
        let mut rows = vec![];
        let mut positions = vec![None; locate.len()];
        while let Some((index, parent_index, level, pos_in_set, set_size)) = stack.pop() {
            let node = &self.nodes[index];
            for (slot, id) in locate.iter().enumerate() {
                if id == &node.id {
                    positions[slot] = Some(total);
                }
            }
            let row_index = total;
            total += 1;
            let is_expanded = expanded.contains(&node.id)
                || (expanded.contains("*") && !expanded.contains(&format!("!{}", node.id)));
            if row_index >= start && row_index < end {
                rows.push(FileRow {
                    node: node.clone(),
                    parent_index,
                    level,
                    expanded: node.has_children.then_some(is_expanded),
                    pos_in_set,
                    set_size,
                });
            }
            if is_expanded {
                let children = &self.children[index];
                for (position, &child) in children.iter().enumerate().rev() {
                    stack.push((
                        child,
                        Some(row_index),
                        level + 1,
                        position + 1,
                        children.len(),
                    ));
                }
            }
        }
        FileTreePage {
            total,
            files: self.files,
            start,
            rows,
            positions,
        }
    }
}
