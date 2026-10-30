// Copyright 2026 Gitwood contributors. SPDX-License-Identifier: Apache-2.0
// Rust port of intellij-community platform/vcs-log/graph/src/com/intellij/vcs/log/
// graph/impl/print/{GraphElementComparatorByLayoutIndex.java,PrintElementGeneratorImpl.kt}
// at 0698a5fe297dfa08069f015cc423a6a85ed7fc60.
// Copyright 2000-2024 JetBrains s.r.o. and contributors. Apache-2.0; see NOTICE.

use super::graph::compact;
use super::{ColorId, CommitId, EdgeId, GraphError, GraphLayout};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Up,
    Down,
}

/// One row-local half of an edge. `from_column` is at this row's center;
/// `to_column` is at the adjacent row's center in `direction`. Canvas draws from
/// (from, row) to ((from + to) / 2, row +/- 0.5), so adjacent halves meet exactly.
/// The edge's color remains the same at both ends, even across viewport boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EdgeSegment {
    pub edge: EdgeId,
    pub from_column: u32,
    pub to_column: u32,
    pub color: ColorId,
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Row {
    pub commit: CommitId,
    pub column: u32,
    pub color: ColorId,
    /// Half-open range into `VisibleRows::segments`.
    pub segments_start: u32,
    pub segments_end: u32,
}

/// Flat, bounded drawing payload. No hashes, strings, permanent graph or hidden rows.
#[derive(Debug, Eq, PartialEq)]
pub struct VisibleRows {
    pub rows: Vec<Row>,
    pub segments: Vec<EdgeSegment>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Element {
    Node(CommitId),
    Edge(EdgeId),
}

struct Positions {
    row: CommitId,
    node_column: u32,
    elements: Vec<Element>,
    edge_columns: Vec<(EdgeId, u32)>,
}

impl Positions {
    fn endpoint(&self, layout: &GraphLayout, edge: EdgeId) -> u32 {
        let relation = layout.graph.edges[edge.0 as usize];
        if self.row == relation.child || self.row == relation.parent {
            self.node_column
        } else {
            let i = self
                .edge_columns
                .binary_search_by_key(&edge, |&(id, _)| id)
                .expect("normal edge must cross its neighboring row");
            self.edge_columns[i].1
        }
    }
}

impl GraphLayout {
    /// Half-open row range; end is clamped, reversed/outside ranges are empty.
    /// Only three rows of temporary positions are retained, regardless of viewport
    /// height. Neighbor positions are included in calculation, never in the payload.
    pub fn visible_rows(&self, start: usize, end: usize) -> Result<VisibleRows, GraphError> {
        let end = end.min(self.graph.len());
        let mut output = VisibleRows {
            rows: Vec::new(),
            segments: Vec::new(),
        };
        if start >= end {
            return Ok(output);
        }
        let mut previous = start.checked_sub(1).map(|i| self.positions(i));
        let mut current = self.positions(start);
        let mut next = (start + 1 < self.graph.len()).then(|| self.positions(start + 1));
        for row in start..end {
            self.print_row(&current, previous.as_ref(), next.as_ref(), &mut output)?;
            if row + 1 < end {
                previous = Some(current);
                current = next.take().expect("next visible row exists");
                next = (row + 2 < self.graph.len()).then(|| self.positions(row + 2));
            }
        }
        Ok(output)
    }

    fn positions(&self, row: usize) -> Positions {
        let node = CommitId(row as u32);
        let mut crossing = Vec::new();
        self.intervals
            .crossing(&self.graph.edges, node.0, &mut crossing);
        let mut elements = Vec::with_capacity(crossing.len() + 1);
        elements.push(Element::Node(node));
        elements.extend(crossing.into_iter().map(Element::Edge));
        elements.sort_by(|&left, &right| self.compare_elements(left, right));
        let mut node_column = 0;
        let mut edge_columns = Vec::with_capacity(elements.len() - 1);
        for (column, element) in elements.iter().enumerate() {
            match element {
                Element::Node(_) => node_column = column as u32,
                Element::Edge(edge) => edge_columns.push((*edge, column as u32)),
            }
        }
        edge_columns.sort_unstable_by_key(|&(edge, _)| edge);
        Positions {
            row: node,
            node_column,
            elements,
            edge_columns,
        }
    }

    fn print_row(
        &self,
        current: &Positions,
        previous: Option<&Positions>,
        next: Option<&Positions>,
        output: &mut VisibleRows,
    ) -> Result<(), GraphError> {
        let start = compact(output.segments.len())?;
        for (column, &element) in current.elements.iter().enumerate() {
            match element {
                Element::Node(node) => {
                    for &edge in self.incoming(node) {
                        self.push_segment(edge, column, previous, Direction::Up, output);
                    }
                    for edge in self.graph.down_edges(node) {
                        self.push_segment(edge, column, next, Direction::Down, output);
                    }
                }
                Element::Edge(edge) => {
                    self.push_segment(edge, column, next, Direction::Down, output);
                    self.push_segment(edge, column, previous, Direction::Up, output);
                }
            }
        }
        output.rows.push(Row {
            commit: current.row,
            column: current.node_column,
            color: self.colors[current.row.0 as usize],
            segments_start: start,
            segments_end: compact(output.segments.len())?,
        });
        Ok(())
    }

    fn push_segment(
        &self,
        edge: EdgeId,
        column: usize,
        neighbor: Option<&Positions>,
        direction: Direction,
        output: &mut VisibleRows,
    ) {
        if let Some(neighbor) = neighbor {
            output.segments.push(EdgeSegment {
                edge,
                from_column: column as u32,
                to_column: neighbor.endpoint(self, edge),
                color: self.edge_color(edge),
                direction,
            });
        }
    }

    fn compare_elements(&self, left: Element, right: Element) -> Ordering {
        if left == right {
            return Ordering::Equal;
        }
        match (left, right) {
            (Element::Edge(a), Element::Edge(b)) => {
                let a_edge = self.graph.edges[a.0 as usize];
                let b_edge = self.graph.edges[b.0 as usize];
                if a_edge.child == b_edge.child {
                    if a_edge.parent < b_edge.parent {
                        self.compare_edge_node(b, a_edge.parent).reverse()
                    } else {
                        self.compare_edge_node(a, b_edge.parent)
                    }
                } else if a_edge.child < b_edge.child {
                    self.compare_edge_node(a, b_edge.child)
                } else {
                    self.compare_edge_node(b, a_edge.child).reverse()
                }
            }
            (Element::Edge(edge), Element::Node(node)) => self.compare_edge_node(edge, node),
            (Element::Node(node), Element::Edge(edge)) => {
                self.compare_edge_node(edge, node).reverse()
            }
            (Element::Node(a), Element::Node(b)) => a.cmp(&b),
        }
    }

    fn compare_edge_node(&self, edge: EdgeId, node: CommitId) -> Ordering {
        let edge = self.graph.edges[edge.0 as usize];
        let edge_index =
            self.indices[edge.child.0 as usize].max(self.indices[edge.parent.0 as usize]);
        match edge_index.cmp(&self.indices[node.0 as usize]) {
            Ordering::Equal => edge.child.cmp(&node),
            order => order,
        }
    }
}
