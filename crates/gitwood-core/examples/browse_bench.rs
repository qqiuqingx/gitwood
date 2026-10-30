//! Real repository read path timings; no WebView or IPC transport is included.
use gitwood_core::{
    git::{GitRunner, RunOptions},
    history::details::Filter,
    repo::session::Sessions,
};
use std::{path::PathBuf, time::Instant};
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(std::env::args_os().nth(1).expect("repository path"));
    let output = std::env::args_os().nth(2).map(PathBuf::from);
    let mut samples = vec![];
    let runner = GitRunner::discover(None).await?;
    let sessions = Sessions::default();
    for run in 0..3 {
        let start = Instant::now();
        let lease = sessions
            .open(runner.clone(), RunOptions::new(&path))
            .await?;
        let opened = start.elapsed();
        let layout_start = Instant::now();
        let info = lease.layout(&Filter::default()).await?;
        let layout = layout_start.elapsed();
        let rows_start = Instant::now();
        let first = lease.rows(0, 40).await?;
        let first_rows = rows_start.elapsed();
        println!("run={run} total={} open_ms={:.3} layout_ms={:.3} first_rows_ms={:.3} backend_first_screen_ms={:.3} rows={} segments={}", info.total, opened.as_secs_f64()*1000., layout.as_secs_f64()*1000., first_rows.as_secs_f64()*1000., start.elapsed().as_secs_f64()*1000., first.nodes.len(), first.segments.len());
        for offset in [40, 80, 1000, 25000, 49960, 0] {
            let start = Instant::now();
            let rows = lease.rows(offset, offset + 40).await?;
            println!(
                "rows start={offset} ms={:.3} nodes={} segments={}",
                start.elapsed().as_secs_f64() * 1000.,
                rows.nodes.len(),
                rows.segments.len()
            );
            if run == 2 {
                samples.push(rows);
            }
        }
    }
    if let Some(output) = output {
        std::fs::write(output, serde_json::to_vec(&samples)?)?;
    }
    Ok(())
}
