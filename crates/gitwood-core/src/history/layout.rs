// Copyright 2026 Gitwood contributors. SPDX-License-Identifier: Apache-2.0
// Rust port of intellij-community platform/vcs-log GraphLayoutBuilder.kt,
// GraphLayoutImpl.kt, GraphColorGetterByHead.kt, GraphColorManagerImpl.kt and
// PrintElementPresentationManagerImpl.kt at 0698a5fe297dfa08069f015cc423a6a85ed7fc60.
// Copyright 2000-2024 JetBrains s.r.o. and contributors (color manager: 2000-2023).
// Apache-2.0; see NOTICE for full upstream paths and modifications.

use super::graph::{CommitGraph, CommitId, EdgeId, GraphError};
use super::interval::EdgeIntervals;
use std::num::NonZeroU32;
use std::{collections::HashSet, mem::size_of};

/// Stable chain color, independent of screen column and theme palette length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorId(pub i32);

impl ColorId {
    /// Canvas maps this index into `graph.lanes`. Theme changes need no relayout.
    pub fn palette_index(self, palette_len: NonZeroU32) -> u32 {
        self.0.unsigned_abs() % palette_len.get()
    }
}

/// Caller-selected head priority and representative ref, in branch layout order.
/// HEAD is an explicit ID, never assumed to be row zero. Unlisted DAG tips follow
/// in log order. Internal branch heads are allowed, just as in GraphLayoutBuilder.
#[derive(Clone, Copy, Debug)]
pub struct Head<'a> {
    pub commit: CommitId,
    pub reference: Option<&'a str>,
}

/// Immutable permanent layout plus a compact index for on-demand row printing.
#[derive(Debug)]
pub struct GraphLayout {
    pub(super) graph: CommitGraph,
    pub(super) dotted_edges: HashSet<EdgeId>,
    pub(super) indices: Vec<u32>,
    pub(super) colors: Vec<ColorId>,
    pub(super) up_offsets: Vec<u32>,
    pub(super) up_edges: Vec<EdgeId>,
    pub(super) intervals: EdgeIntervals,
}

impl GraphLayout {
    pub fn new(graph: CommitGraph, heads: &[Head<'_>]) -> Result<Self, GraphError> {
        for head in heads {
            if head.commit.0 as usize >= graph.len() {
                return Err(GraphError::InvalidHead(head.commit));
            }
        }
        let (up_offsets, up_edges) = incoming_edges(&graph);
        let mut indices = vec![0; graph.len()];
        let mut colors = vec![ColorId(0); graph.len()];
        let mut next_index: u32 = 1;
        let tips = (0..graph.len()).filter(|&i| up_offsets[i] == up_offsets[i + 1]);
        let ordered_heads = heads.iter().copied().chain(tips.map(|i| Head {
            commit: CommitId(i as u32),
            reference: None,
        }));
        let mut stack = Vec::new();
        for head in ordered_heads {
            if indices[head.commit.0 as usize] != 0 {
                continue;
            }
            let head_index = next_index;
            stack.push(Frame::new(&graph, head.commit));
            while let Some(frame) = stack.last_mut() {
                let node = frame.node.0 as usize;
                if frame.first_visit {
                    indices[node] = next_index;
                    colors[node] = ColorId(if next_index == head_index {
                        head.reference.map(java_string_hash).unwrap_or(0)
                    } else {
                        next_index as i32
                    });
                }
                let parent = frame.remaining.find_map(|edge| {
                    let parent = graph.edges[edge as usize].parent;
                    (indices[parent.0 as usize] == 0).then_some(parent)
                });
                if let Some(parent) = parent {
                    frame.first_visit = false;
                    stack.push(Frame::new(&graph, parent));
                } else {
                    if frame.first_visit {
                        next_index = next_index.checked_add(1).ok_or(GraphError::TooLarge)?;
                    }
                    stack.pop();
                }
            }
        }
        let intervals = EdgeIntervals::new(&graph.edges);
        Ok(Self {
            dotted_edges: HashSet::new(),
            graph,
            indices,
            colors,
            up_offsets,
            up_edges,
            intervals,
        })
    }

    /// True when filtering skipped one or more original commits.
    pub fn edge_is_dotted(&self, edge: EdgeId) -> bool {
        self.dotted_edges.contains(&edge)
    }

    pub fn graph(&self) -> &CommitGraph {
        &self.graph
    }

    /// Permanent DFS chain index (one-based), not the rendered screen column.
    pub fn layout_index(&self, commit: CommitId) -> Option<u32> {
        self.indices.get(commit.0 as usize).copied()
    }

    pub fn color(&self, commit: CommitId) -> Option<ColorId> {
        self.colors.get(commit.0 as usize).copied()
    }

    pub(super) fn edge_color(&self, edge: EdgeId) -> ColorId {
        let edge = self.graph.edges[edge.0 as usize];
        let node = if self.indices[edge.child.0 as usize] >= self.indices[edge.parent.0 as usize] {
            edge.child
        } else {
            edge.parent
        };
        self.colors[node.0 as usize]
    }

    pub(super) fn incoming(&self, node: CommitId) -> &[EdgeId] {
        let i = node.0 as usize;
        &self.up_edges[self.up_offsets[i] as usize..self.up_offsets[i + 1] as usize]
    }

    /// Resident owned-array estimate; excludes temporary parsing/DFS/query buffers,
    /// allocator overhead, process code, Git subprocess and the future WebView.
    pub fn estimated_bytes(&self) -> usize {
        size_of::<Self>() + self.graph.estimated_bytes() - size_of::<CommitGraph>()
            + self.indices.capacity() * size_of::<u32>()
            + self.colors.capacity() * size_of::<ColorId>()
            + self.up_offsets.capacity() * size_of::<u32>()
            + self.up_edges.capacity() * size_of::<EdgeId>()
            + self.intervals.estimated_heap_bytes()
    }
}

struct Frame {
    node: CommitId,
    remaining: std::ops::Range<u32>,
    first_visit: bool,
}

impl Frame {
    fn new(graph: &CommitGraph, node: CommitId) -> Self {
        let i = node.0 as usize;
        Self {
            node,
            remaining: graph.parent_offsets[i]..graph.parent_offsets[i + 1],
            first_visit: true,
        }
    }
}

fn incoming_edges(graph: &CommitGraph) -> (Vec<u32>, Vec<EdgeId>) {
    let mut offsets = vec![0; graph.len() + 1];
    for edge in &graph.edges {
        offsets[edge.parent.0 as usize + 1] += 1;
    }
    for i in 1..offsets.len() {
        offsets[i] += offsets[i - 1];
    }
    let mut cursors = offsets.clone();
    let mut incoming = vec![EdgeId(0); graph.edges.len()];
    for (i, edge) in graph.edges.iter().enumerate() {
        let cursor = &mut cursors[edge.parent.0 as usize];
        incoming[*cursor as usize] = EdgeId(i as u32);
        *cursor += 1;
    }
    (offsets, incoming)
}

fn java_string_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0_i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}
