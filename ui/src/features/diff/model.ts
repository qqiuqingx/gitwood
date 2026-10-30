import type { Alignment, ChangeBlock, FoldRange, LineRange, Side } from './types';

/** IDEA BaseSyncScrollable.transferLine: preserve offset until target block ends.
 * Empty source spans choose their leading boundary. Never scale the whole document. */
export function mapLine(line: number, side: Side, alignment: readonly Alignment[]): number {
  const other = side === 'left' ? 'right' : 'left';
  const value = Math.max(0, line);
  let lo = 0, hi = alignment.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (alignment[mid][side].end < value) lo = mid + 1; else hi = mid;
  }
  const span = alignment[lo];
  if (span) {
    const a = span[side], b = span[other];
    if (value < a.start) return Math.max(0, b.start - (a.start - value));
    if (value === a.start) return b.start;
    if (value === a.end) return b.end;
    if (value < a.end) return Math.min(b.start + value - a.start, b.end);
  }
  const last = alignment.at(-1);
  return last ? last[other].end + value - last[side].end : value;
}
/** No wrap: reaching either edge keeps the current difference selected. */
export function nextDifference(blocks: readonly ChangeBlock[], line: number, side: Side, direction: 1 | -1, current = -1): number {
  if (!blocks.length) return -1;
  if (current >= 0) return Math.max(0, Math.min(blocks.length - 1, current + direction));
  if (direction === 1) return blocks.findIndex(block => block[side].start >= line);
  for (let i = blocks.length - 1; i >= 0; i--) if (blocks[i][side].start <= line) return i;
  return -1;
}
export function collapsedFolds(folds: readonly FoldRange[], expanded: ReadonlySet<number>): { id: number; range: FoldRange }[] {
  return folds.flatMap((range, id) => expanded.has(id) ? [] : [{ id, range }]);
}
export function expandFold(expanded: ReadonlySet<number>, id: number): ReadonlySet<number> {
  return new Set([...expanded, id]);
}
export interface Connector { path: string; top: number; bottom: number; leftY: number; rightY: number }
/** Curved band endpoints are measured CodeMirror line boundaries (folds included). */
export function connectorGeometry(left: readonly [number, number], right: readonly [number, number], width: number): Connector {
  const [a, b] = left, [c, d] = right;
  const mid = width / 2;
  return { path: `M0,${a} C${mid},${a} ${mid},${c} ${width},${c} L${width},${d} C${mid},${d} ${mid},${b} 0,${b} Z`,
    top: Math.min(a, c), bottom: Math.max(b, d), leftY: (a + b) / 2, rightY: (c + d) / 2 };
}

/** Sorted block ranges let scrolling measure only candidates intersecting either
 * viewport. Include spanning bands and zero-length insertion/deletion anchors. */
export function visibleBlockRange(blocks: readonly ChangeBlock[], left: LineRange, right: LineRange) {
  const lower = (side: Side, line: number) => {
    let lo = 0, hi = blocks.length;
    while (lo < hi) { const mid = (lo + hi) >>> 1; if (blocks[mid][side].end < line) lo = mid + 1; else hi = mid; }
    return lo;
  };
  const upper = (side: Side, line: number) => {
    let lo = 0, hi = blocks.length;
    while (lo < hi) { const mid = (lo + hi) >>> 1; if (blocks[mid][side].start <= line) lo = mid + 1; else hi = mid; }
    return lo;
  };
  return { start: Math.min(lower('left', left.start), lower('right', right.start)),
    end: Math.max(upper('left', left.end), upper('right', right.end)) };
}
