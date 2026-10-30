// Copyright 2026 Gitwood contributors. SPDX-License-Identifier: Apache-2.0

use super::{Edge, EdgeId};

/// Max-parent segment tree over edges already sorted by child row. Unlike row
/// snapshots, a long edge occupies O(1) space regardless of its length.
#[derive(Debug)]
pub(super) struct EdgeIntervals {
    max_parent: Vec<u32>,
    leaves: usize,
}

impl EdgeIntervals {
    pub fn new(edges: &[Edge]) -> Self {
        let leaves = edges.len().next_power_of_two();
        let mut max_parent = vec![0; leaves * 2];
        for (i, edge) in edges.iter().enumerate() {
            max_parent[leaves + i] = edge.parent.0;
        }
        for i in (1..leaves).rev() {
            max_parent[i] = max_parent[i * 2].max(max_parent[i * 2 + 1]);
        }
        Self { max_parent, leaves }
    }

    pub fn crossing(&self, edges: &[Edge], row: u32, result: &mut Vec<EdgeId>) {
        self.collect(edges, row, 1, 0, self.leaves, result);
    }

    fn collect(
        &self,
        edges: &[Edge],
        row: u32,
        tree: usize,
        start: usize,
        end: usize,
        result: &mut Vec<EdgeId>,
    ) {
        if start >= edges.len() || edges[start].child.0 >= row || self.max_parent[tree] <= row {
            return;
        }
        if end - start == 1 {
            result.push(EdgeId(start as u32));
            return;
        }
        let middle = (start + end) / 2;
        self.collect(edges, row, tree * 2, start, middle, result);
        self.collect(edges, row, tree * 2 + 1, middle, end, result);
    }

    pub fn estimated_heap_bytes(&self) -> usize {
        self.max_parent.capacity() * std::mem::size_of::<u32>()
    }
}
