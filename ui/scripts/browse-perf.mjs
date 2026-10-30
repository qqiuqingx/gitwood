// Uses real history_rows output from browse_bench. This measures pure command
// construction in Node, not Canvas/WebView frame or Tauri transport latency.
import { readFileSync } from 'node:fs';
import { performance } from 'node:perf_hooks';
import { graphCommands } from '../src/features/browse/graph.ts';
const pages = JSON.parse(readFileSync(process.argv[2], 'utf8'));
for (const page of pages) {
  const rows = page.nodes.map(([index, column, color, start, end], i) => ({ index, column, color, segments: page.segments.slice(start, end), commit: page.commits[i] }));
  const top = rows[0].index * 24;
  for (let i = 0; i < 100; i++) graphCommands(rows, top, 24, 960, null);
  const timings = [];
  let count = 0;
  for (let i = 0; i < 1000; i++) {
    const start = performance.now(); count = graphCommands(rows, top, 24, 960, null).length;
    timings.push(performance.now() - start);
  }
  timings.sort((a, b) => a - b);
  console.log(JSON.stringify({ start: rows[0].index, rows: rows.length, segments: page.segments.length, commands: count,
    p50_ms: +timings[500].toFixed(4), p95_ms: +timings[950].toFixed(4), max_ms: +timings.at(-1).toFixed(4) }));
}
