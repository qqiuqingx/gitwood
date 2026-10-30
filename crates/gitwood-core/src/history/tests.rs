// Copyright 2026 Gitwood contributors. SPDX-License-Identifier: Apache-2.0
// Upstream geometry/head-order cases adapted from intellij-community at
// 0698a5fe297dfa08069f015cc423a6a85ed7fc60, platform/vcs-log/graph/
// testData/elementGenerator/manyNodes_{in,out}.txt and
// test/com/intellij/vcs/log/graph/impl/permanent/GraphLayoutBuilderHeadOrderTest.kt.
// Copyright 2000-2024 JetBrains s.r.o. and contributors. Apache-2.0; see NOTICE.

use super::*;
use std::num::NonZeroU32;

fn hash(id: usize) -> String {
    format!("{:040x}", id + 1)
}

fn log(parents: &[&[usize]]) -> String {
    parents
        .iter()
        .enumerate()
        .map(|(id, parents)| {
            let mut line = hash(id);
            for parent in *parents {
                line.push(' ');
                line.push_str(&hash(*parent));
            }
            line.push('\n');
            line
        })
        .collect()
}

fn layout(parents: &[&[usize]]) -> GraphLayout {
    GraphLayout::new(
        CommitGraph::parse_log(log(parents).as_bytes()).unwrap(),
        &[],
    )
    .unwrap()
}

fn columns(layout: &GraphLayout) -> Vec<u32> {
    layout
        .visible_rows(0, usize::MAX)
        .unwrap()
        .rows
        .iter()
        .map(|r| r.column)
        .collect()
}

fn segments(layout: &GraphLayout, row: usize) -> Vec<(usize, usize, Direction, u32, u32)> {
    layout
        .visible_rows(row, row + 1)
        .unwrap()
        .segments
        .iter()
        .map(|segment| {
            let edge = layout.graph().edge(segment.edge).unwrap();
            (
                edge.child.0 as usize,
                edge.parent.0 as usize,
                segment.direction,
                segment.from_column,
                segment.to_column,
            )
        })
        .collect()
}

#[test]
fn linear_history_has_one_lane_and_continuous_color() {
    let graph = layout(&[&[1], &[2], &[]]);
    assert_eq!(columns(&graph), [0, 0, 0]);
    assert_eq!(graph.indices, [1, 1, 1]);
    assert_eq!(
        segments(&graph, 1),
        [(0, 1, Direction::Up, 0, 0), (1, 2, Direction::Down, 0, 0)]
    );
    assert_eq!(graph.colors, [ColorId(0); 3]);
}

#[test]
fn fork_merge_keeps_first_parent_chain_left_and_side_chain_color() {
    // GraphLayoutBuilder follows parents in input order. The comparator sorts by
    // max(endpoint layout index); PrintElementGenerator connects adjacent positions.
    let graph = layout(&[&[1, 2], &[3], &[3], &[]]);
    assert_eq!(graph.indices, [1, 1, 2, 1]);
    assert_eq!(columns(&graph), [0, 0, 1, 0]);
    assert_eq!(
        graph.colors,
        [ColorId(0), ColorId(0), ColorId(2), ColorId(0)]
    );
    assert_eq!(
        segments(&graph, 0),
        [(0, 1, Direction::Down, 0, 0), (0, 2, Direction::Down, 0, 1)]
    );
    assert_eq!(
        segments(&graph, 2),
        [
            (1, 3, Direction::Down, 0, 0),
            (1, 3, Direction::Up, 0, 0),
            (0, 2, Direction::Up, 1, 1),
            (2, 3, Direction::Down, 1, 0),
        ]
    );
    let edge = graph
        .visible_rows(2, 3)
        .unwrap()
        .segments
        .last()
        .copied()
        .unwrap();
    assert_eq!(edge.color, ColorId(2)); // Larger layout index supplies the merge edge color.
}

#[test]
fn octopus_merge_preserves_all_parent_edges() {
    let graph = layout(&[&[1, 2, 3], &[4], &[4], &[4], &[]]);
    assert_eq!(graph.indices, [1, 1, 2, 3, 1]);
    assert_eq!(columns(&graph), [0, 0, 1, 2, 0]);
    assert_eq!(
        segments(&graph, 0),
        [
            (0, 1, Direction::Down, 0, 0),
            (0, 2, Direction::Down, 0, 1),
            (0, 3, Direction::Down, 0, 2),
        ]
    );
    assert_eq!(
        segments(&graph, 4),
        [
            (1, 4, Direction::Up, 0, 0),
            (2, 4, Direction::Up, 0, 1),
            (3, 4, Direction::Up, 0, 2),
        ]
    );
}

#[test]
fn independent_roots_release_screen_columns() {
    let graph = layout(&[&[2], &[3], &[], &[]]);
    assert_eq!(graph.indices, [1, 2, 1, 2]);
    assert_eq!(columns(&graph), [0, 1, 0, 0]);
    assert_eq!(
        segments(&graph, 1),
        [
            (0, 2, Direction::Down, 0, 0),
            (0, 2, Direction::Up, 0, 0),
            (1, 3, Direction::Down, 1, 1),
        ]
    );
    assert_eq!(segments(&graph, 3), [(1, 3, Direction::Up, 0, 1)]);
}

#[test]
fn long_branch_survives_random_viewport_jumps_without_row_expansion() {
    let count = 12_000;
    let mut parents = vec![vec![1, 2], vec![count - 1]];
    parents.extend((2..count - 1).map(|i| vec![i + 1]));
    parents.push(vec![]);
    let refs: Vec<_> = parents.iter().map(Vec::as_slice).collect();
    let graph = layout(&refs);
    let middle = segments(&graph, 7_000);
    assert!(middle.contains(&(1, count - 1, Direction::Up, 0, 0)));
    assert!(middle.contains(&(1, count - 1, Direction::Down, 0, 0)));
    assert_eq!(graph.visible_rows(7_000, 7_001).unwrap().rows[0].column, 1);
    assert_eq!(
        segments(&graph, count - 1),
        [
            (1, count - 1, Direction::Up, 0, 0),
            (count - 2, count - 1, Direction::Up, 0, 1),
        ]
    );
    assert!(graph.estimated_bytes() < count * 100);
}

#[test]
fn explicit_head_priority_can_start_below_first_log_row() {
    // GraphLayoutBuilderHeadOrderTest.linearGraph: order(2, 0) assigns head 2 to
    // nodes 2/3 and head 0 to nodes 0/1. Ref hash uses Java's UTF-16 String.hashCode.
    let parsed = CommitGraph::parse_log(log(&[&[1], &[2], &[3], &[]]).as_bytes()).unwrap();
    let graph = GraphLayout::new(
        parsed,
        &[
            Head {
                commit: CommitId(2),
                reference: Some("main"),
            },
            Head {
                commit: CommitId(0),
                reference: Some("feature"),
            },
        ],
    )
    .unwrap();
    assert_eq!(graph.indices, [2, 2, 1, 1]);
    assert_eq!(columns(&graph), [0, 0, 0, 0]);
    assert_eq!(
        graph.colors,
        [
            ColorId(-979207434),
            ColorId(-979207434),
            ColorId(3343801),
            ColorId(3343801)
        ]
    );
    assert_eq!(
        ColorId(i32::MIN).palette_index(NonZeroU32::new(7).unwrap()),
        2
    );
}

#[test]
fn prioritized_second_tip_controls_joined_ancestors() {
    let parsed = CommitGraph::parse_log(log(&[&[2], &[2], &[]]).as_bytes()).unwrap();
    let graph = GraphLayout::new(
        parsed,
        &[Head {
            commit: CommitId(1),
            reference: Some("main"),
        }],
    )
    .unwrap();
    assert_eq!(graph.indices, [2, 1, 1]);
    assert_eq!(columns(&graph), [0, 0, 0]);
    assert_eq!(
        segments(&graph, 1),
        [
            (1, 2, Direction::Down, 0, 0),
            (0, 2, Direction::Down, 1, 0),
            (0, 2, Direction::Up, 1, 0),
        ]
    );
}

#[test]
fn upstream_print_fixture_has_identical_normal_edge_geometry() {
    // elementGenerator/manyNodes_{in,out}.txt node columns and selected segments.
    // Its one dashed normal edge (1 -> 6) is solid here; sorting is identical.
    // All edges are shorter than upstream's long-edge hiding threshold.
    let graph = layout(&[&[1, 6, 3], &[6], &[3], &[], &[5], &[], &[7], &[]]);
    assert_eq!(graph.indices, [1, 1, 3, 2, 4, 4, 1, 1]);
    assert_eq!(columns(&graph), [0, 1, 3, 2, 2, 2, 0, 0]);
    assert_eq!(
        segments(&graph, 0),
        [
            (0, 1, Direction::Down, 0, 1),
            (0, 6, Direction::Down, 0, 0),
            (0, 3, Direction::Down, 0, 2),
        ]
    );
    assert!(segments(&graph, 5).contains(&(1, 6, Direction::Down, 1, 0)));
}

#[test]
fn parser_interns_hashes_deduplicates_parents_and_accepts_sha256_crlf() {
    let input = log(&[&[1, 1], &[]]).replace('\n', "\r\n");
    let graph = CommitGraph::parse_log(input.as_bytes()).unwrap();
    assert_eq!(graph.edge_count(), 1);
    assert_eq!(
        graph.parents(CommitId(0)).collect::<Vec<_>>(),
        [CommitId(1)]
    );
    assert_eq!(graph.hash(CommitId(1)).unwrap(), hash(1));
    assert_eq!(graph.find_commit(&hash(1)), Some(CommitId(1)));
    assert_eq!(graph.hash(CommitId(2)), None);
    let sha256 = format!("{:064x} {:064x}\n{:064x}\n", 1, 2, 2);
    let graph = CommitGraph::parse_log(sha256.as_bytes()).unwrap();
    assert_eq!(graph.hash(CommitId(0)).unwrap().len(), 64);
    assert_eq!(graph.find_commit(&hash(0)), None);
}

#[test]
fn parser_rejects_broken_or_non_closed_relationships() {
    for input in [b"bad".as_slice(), b"\n", b"\r\n"] {
        assert!(matches!(
            CommitGraph::parse_log(input),
            Err(GraphError::InvalidHash { .. })
        ));
    }
    assert!(matches!(
        CommitGraph::parse_log(log(&[&[1]]).as_bytes()),
        Err(GraphError::MissingParent { .. })
    ));
    assert!(matches!(
        CommitGraph::parse_log(log(&[&[0]]).as_bytes()),
        Err(GraphError::NotTopological { .. })
    ));
    assert!(matches!(
        CommitGraph::parse_log(log(&[&[], &[0]]).as_bytes()),
        Err(GraphError::NotTopological { .. })
    ));
    let duplicate = format!("{}\n{}\n", hash(0), hash(0));
    assert!(matches!(
        CommitGraph::parse_log(duplicate.as_bytes()),
        Err(GraphError::DuplicateCommit { .. })
    ));
    let invalid = format!("{}\n\n", hash(0));
    assert!(CommitGraph::parse_log(invalid.as_bytes()).is_err());
    let mixed = format!("{}\n{:064x}\n", hash(0), 2);
    assert!(CommitGraph::parse_log(mixed.as_bytes()).is_err());
}

#[test]
fn empty_and_clipped_queries_have_explicit_boundaries() {
    let empty = layout(&[]);
    assert!(empty.visible_rows(0, 10).unwrap().rows.is_empty());
    assert!(matches!(
        GraphLayout::new(
            CommitGraph::parse_log(b"").unwrap(),
            &[Head {
                commit: CommitId(0),
                reference: None,
            }]
        ),
        Err(GraphError::InvalidHead(CommitId(0)))
    ));
    let graph = layout(&[&[1], &[]]);
    assert!(graph.visible_rows(2, usize::MAX).unwrap().rows.is_empty());
    assert!(graph.visible_rows(1, 0).unwrap().rows.is_empty());
    assert_eq!(graph.visible_rows(1, usize::MAX).unwrap().rows.len(), 1);
}

#[test]
fn varied_dags_have_all_edges_and_viewport_independent_continuity() {
    // Deterministic DAGs exercise multi-head, redundant merge and crossing edges.
    // Assert the externally visible drawing contract rather than internal index layout.
    let mut seed = 17_u64;
    for _ in 0..40 {
        let count = 35;
        let mut parents = vec![Vec::new(); count];
        for (i, row) in parents.iter_mut().enumerate().take(count - 1) {
            for _ in 0..3 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let parent = i + 1 + (seed as usize % (count - i - 1));
                if !row.contains(&parent) {
                    row.push(parent);
                }
            }
        }
        let refs: Vec<_> = parents.iter().map(Vec::as_slice).collect();
        let graph = layout(&refs);
        let full = graph.visible_rows(0, count).unwrap();
        for i in 0..count {
            let window = graph.visible_rows(i, i + 1).unwrap();
            let row = full.rows[i];
            let all_segments =
                &full.segments[row.segments_start as usize..row.segments_end as usize];
            assert_eq!(window.rows[0].column, row.column);
            assert_eq!(window.segments, all_segments);
            for (edge_id, edge) in graph.graph.edges.iter().enumerate() {
                let expected =
                    usize::from(edge.child.0 as usize <= i && i < edge.parent.0 as usize)
                        + usize::from((edge.child.0 as usize) < i && i <= edge.parent.0 as usize);
                assert_eq!(
                    all_segments
                        .iter()
                        .filter(|s| s.edge == EdgeId(edge_id as u32))
                        .count(),
                    expected
                );
            }
            for segment in all_segments
                .iter()
                .filter(|s| s.direction == Direction::Down)
            {
                let next = full.rows[i + 1];
                let next_segments =
                    &full.segments[next.segments_start as usize..next.segments_end as usize];
                assert!(next_segments.iter().any(|s| s.edge == segment.edge
                    && s.direction == Direction::Up
                    && s.from_column == segment.to_column
                    && s.to_column == segment.from_column
                    && s.color == segment.color));
            }
        }
    }
}
