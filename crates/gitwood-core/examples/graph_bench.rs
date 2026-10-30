// Copyright 2026 Gitwood contributors. SPDX-License-Identifier: Apache-2.0
//! Run a release executable under `/usr/bin/time -l` (macOS) or
//! `/usr/bin/time -v` (Linux) for process peak RSS, separately from the estimates.
//! `--log-file <path>` isolates Rust RSS from the Git subprocess's exit statistics.

use gitwood_core::git::{GitRunner, RunOptions};
use gitwood_core::history::{read, CommitGraph, GraphLayout};
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let usage = "usage: graph_bench <repository> [git-executable] | --log-file <path>";
    let first = args.next().ok_or(usage)?;
    let from_file = first == "--log-file";
    let repository = PathBuf::from(if from_file {
        args.next().ok_or(usage)?
    } else {
        first
    });
    let executable = args.next();
    if args.next().is_some() || (from_file && executable.is_some()) {
        return Err(usage.into());
    }
    let git = executable.map(PathBuf::from);
    let read_start = Instant::now();
    let bytes = if from_file {
        std::fs::read(repository)?
    } else {
        read::read_log(
            &GitRunner::discover(git.as_deref()).await?,
            &RunOptions::new(&repository),
        )
        .await?
    };
    let read_ms = read_start.elapsed().as_secs_f64() * 1000.0;
    let parse_start = Instant::now();
    let graph = CommitGraph::parse_log(&bytes)?;
    let parse_ms = parse_start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "source={} commits={} edges={} log_bytes={}",
        if from_file { "log-file" } else { "git" },
        graph.len(),
        graph.edge_count(),
        bytes.len()
    );
    drop(bytes);
    let layout_start = Instant::now();
    let layout = GraphLayout::new(graph, &[])?;
    let layout_ms = layout_start.elapsed().as_secs_f64() * 1000.0;
    let viewport_rows = 120.min(layout.graph().len());
    let first_start = Instant::now();
    let first = black_box(layout.visible_rows(0, viewport_rows)?);
    let first_us = first_start.elapsed().as_secs_f64() * 1_000_000.0;
    let mut samples = Vec::with_capacity(1000);
    let mut elements = first.segments.len() + first.rows.len();
    let mut widest_column = first.rows.iter().map(|row| row.column).max().unwrap_or(0);
    drop(first);
    let span = layout.graph().len().saturating_sub(viewport_rows) + 1;
    for i in 0..1000 {
        let start = i * 7919 % span;
        let clock = Instant::now();
        let visible = black_box(layout.visible_rows(start, start + viewport_rows)?);
        samples.push(clock.elapsed().as_secs_f64() * 1_000_000.0);
        elements += visible.segments.len() + visible.rows.len();
        widest_column =
            widest_column.max(visible.rows.iter().map(|row| row.column).max().unwrap_or(0));
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "read_ms={read_ms:.3} parse_ms={parse_ms:.3} read_parse_ms={:.3} layout_ms={layout_ms:.3}",
        read_ms + parse_ms
    );
    println!("resident_estimated_bytes={}", layout.estimated_bytes());
    println!("viewport_rows={viewport_rows} queries={} first_query_us={first_us:.3} median_query_us={:.3} p95_query_us={:.3} max_query_us={:.3}", samples.len(), samples[500], samples[949], samples[999]);
    println!(
        "sampled_max_node_column={widest_column} consumed_elements={}",
        black_box(elements)
    );
    Ok(())
}
