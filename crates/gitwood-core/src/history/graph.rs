// Copyright 2026 Gitwood contributors. SPDX-License-Identifier: Apache-2.0
// Permanent graph / duplicate-parent representation adapted from intellij-community:
// platform/vcs-log/graph/src/com/intellij/vcs/log/graph/impl/permanent/
// {PermanentLinearGraphBuilder,DuplicateParentFixer}.java at
// 0698a5fe297dfa08069f015cc423a6a85ed7fc60.
// Copyright 2000-2024 JetBrains s.r.o. and contributors. Apache-2.0; see NOTICE.

use std::collections::HashMap;
use std::fmt;
use std::mem::size_of;

/// A row index, valid only in the immutable graph snapshot that produced it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CommitId(pub u32);

/// An index into the snapshot's edge array.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EdgeId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edge {
    pub child: CommitId,
    pub parent: CommitId,
}

#[derive(Debug, Eq, PartialEq)]
pub enum GraphError {
    InvalidHash { line: usize },
    DuplicateCommit { line: usize },
    MissingParent { line: usize },
    NotTopological { line: usize },
    InvalidHead(CommitId),
    TooLarge,
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHash { line } => {
                write!(f, "invalid or mixed-width object ID on line {line}")
            }
            Self::DuplicateCommit { line } => write!(f, "duplicate commit on line {line}"),
            Self::MissingParent { line } => write!(f, "parent outside graph on line {line}"),
            Self::NotTopological { line } => write!(f, "parent must follow child on line {line}"),
            Self::InvalidHead(id) => write!(f, "head {} is outside graph", id.0),
            Self::TooLarge => f.write_str("graph exceeds compact u32 index limits"),
        }
    }
}

impl std::error::Error for GraphError {}

/// Closed, topologically ordered DAG. Hash bytes occur once; all relations use u32 IDs.
/// SHA-1 occupies the first 20 bytes of each slot; SHA-256 uses the full 32 bytes.
#[derive(Debug)]
pub struct CommitGraph {
    hashes: Vec<[u8; 32]>,
    hash_bytes: usize,
    pub(super) parent_offsets: Vec<u32>,
    pub(super) edges: Vec<Edge>,
}

impl CommitGraph {
    /// Parse `git log --format=%H %P --topo-order` output (also accepts CRLF).
    /// Missing parents, duplicate commits and non-topological input are errors.
    /// A shallow Git log is supported: Git reports shallow boundary nodes as roots.
    pub fn parse_log(input: &[u8]) -> Result<Self, GraphError> {
        let mut hashes = Vec::new();
        let mut by_hash = HashMap::new();
        let mut hash_bytes = 0;
        for (index, line) in lines(input).enumerate() {
            let token = words(line)
                .next()
                .ok_or(GraphError::InvalidHash { line: index + 1 })?;
            let hash = parse_hash(token, &mut hash_bytes, index + 1)?;
            let id = CommitId(compact(index)?);
            if by_hash.insert(hash, id).is_some() {
                return Err(GraphError::DuplicateCommit { line: index + 1 });
            }
            hashes.push(hash);
        }
        let mut graph = Self {
            hashes,
            hash_bytes,
            parent_offsets: vec![0],
            edges: Vec::new(),
        };
        for (index, line) in lines(input).enumerate() {
            let start = graph.edges.len();
            for token in words(line).skip(1) {
                let hash = parse_hash(token, &mut graph.hash_bytes, index + 1)?;
                let parent = *by_hash
                    .get(&hash)
                    .ok_or(GraphError::MissingParent { line: index + 1 })?;
                if parent.0 as usize <= index {
                    return Err(GraphError::NotTopological { line: index + 1 });
                }
                // Like DuplicateParentFixer: preserve the first occurrence and parent order.
                if !graph.edges[start..]
                    .iter()
                    .any(|edge| edge.parent == parent)
                {
                    compact(graph.edges.len() + 1)?;
                    graph.edges.push(Edge {
                        child: CommitId(index as u32),
                        parent,
                    });
                }
            }
            graph.parent_offsets.push(compact(graph.edges.len())?);
        }
        graph.hashes.shrink_to_fit();
        graph.edges.shrink_to_fit();
        graph.parent_offsets.shrink_to_fit();
        Ok(graph)
    }

    pub fn len(&self) -> usize {
        self.hashes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn hash(&self, id: CommitId) -> Option<String> {
        self.hashes.get(id.0 as usize).map(|hash| {
            use std::fmt::Write;
            let mut text = String::with_capacity(self.hash_bytes * 2);
            for byte in &hash[..self.hash_bytes] {
                write!(text, "{byte:02x}").expect("write into String");
            }
            text
        })
    }

    /// Resolve an external hash once (for example HEAD); no resident string/hash map.
    pub fn find_commit(&self, hash: &str) -> Option<CommitId> {
        let mut width = self.hash_bytes;
        let hash = parse_hash(hash.as_bytes(), &mut width, 1).ok()?;
        self.hashes
            .iter()
            .position(|value| *value == hash)
            .map(|i| CommitId(i as u32))
    }

    pub fn edge(&self, id: EdgeId) -> Option<Edge> {
        self.edges.get(id.0 as usize).copied()
    }

    pub fn parents(&self, id: CommitId) -> impl Iterator<Item = CommitId> + '_ {
        self.down_edges(id)
            .map(|edge| self.edges[edge.0 as usize].parent)
    }

    pub(super) fn down_edges(&self, id: CommitId) -> impl Iterator<Item = EdgeId> {
        let i = id.0 as usize;
        let range = self.parent_offsets.get(i..i + 2).unwrap_or(&[0, 0]);
        (range[0]..range[1]).map(EdgeId)
    }

    /// Owned array capacities plus the struct, excluding allocator metadata/padding.
    pub fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self.hashes.capacity() * size_of::<[u8; 32]>()
            + self.parent_offsets.capacity() * size_of::<u32>()
            + self.edges.capacity() * size_of::<Edge>()
    }
}

fn lines(input: &[u8]) -> impl Iterator<Item = &[u8]> {
    // A final LF is framing; interior blank lines remain invalid records.
    input
        .strip_suffix(b"\n")
        .unwrap_or(input)
        .split(|b| *b == b'\n')
        .filter(move |_| !input.is_empty())
}

fn words(line: &[u8]) -> impl Iterator<Item = &[u8]> {
    line.split(u8::is_ascii_whitespace)
        .filter(|word| !word.is_empty())
}

fn parse_hash(token: &[u8], width: &mut usize, line: usize) -> Result<[u8; 32], GraphError> {
    if !matches!(token.len(), 40 | 64) || (*width != 0 && token.len() != *width * 2) {
        return Err(GraphError::InvalidHash { line });
    }
    let mut hash = [0; 32];
    for (slot, pair) in hash.iter_mut().zip(token.as_chunks::<2>().0) {
        let hi = hex(pair[0]).ok_or(GraphError::InvalidHash { line })?;
        let lo = hex(pair[1]).ok_or(GraphError::InvalidHash { line })?;
        *slot = hi * 16 + lo;
    }
    *width = token.len() / 2;
    Ok(hash)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(super) fn compact(value: usize) -> Result<u32, GraphError> {
    u32::try_from(value).map_err(|_| GraphError::TooLarge)
}
