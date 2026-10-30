import { describe, it, expect } from 'vitest';
import { EditorState } from '@codemirror/state';
import { collapsedFolds, connectorGeometry, expandFold, mapLine, nextDifference, visibleBlockRange } from '../src/features/diff/model';
import { diffDecorations } from '../src/features/diff/editor';
import type { ChangeBlock, DiffResult, FoldRange } from '../src/features/diff/types';
const range = (start: number, end: number) => ({ start, end });
const block = (ls: number, le: number, rs: number, re: number): ChangeBlock => ({ kind: ls === le ? 'insert' : rs === re ? 'delete' : 'modify', left: range(ls, le), right: range(rs, re), leftInline: [], rightInline: [] });
const blocks = [block(3, 3, 3, 6), block(10, 14, 13, 14), block(17, 18, 17, 18)];
const alignment = [
  { left: range(0, 3), right: range(0, 3) }, blocks[0],
  { left: range(3, 10), right: range(6, 13) }, blocks[1],
  { left: range(14, 17), right: range(14, 17) }, blocks[2],
  { left: range(18, 30), right: range(18, 30) },
];
describe('line mapping uses changed block boundaries', () => {
  it('maps equal lines after insertions, pins deleted surplus, and handles endpoints', () => {
    expect(mapLine(4, 'left', alignment)).toBe(7);
    expect(mapLine(7, 'right', alignment)).toBe(4);
    expect(mapLine(11, 'left', alignment)).toBe(14);
    expect(mapLine(12, 'left', alignment)).toBe(14);
    expect(mapLine(14, 'left', alignment)).toBe(14);
    expect(mapLine(4, 'right', alignment)).toBe(3);
    expect(mapLine(30, 'left', alignment)).toBe(30);
    expect(mapLine(50, 'left', alignment)).toBe(50);
    expect(mapLine(-1, 'left', alignment)).toBe(0);
    expect(mapLine(4, 'left', [])).toBe(4);
  });
  it('maps a leading insertion anchor and a wholly deleted document', () => {
    expect(mapLine(0, 'left', [block(0, 0, 0, 5)])).toBe(0);
    expect(mapLine(2, 'left', [block(0, 5, 0, 0)])).toBe(0);
  });
});
it('navigates from caret or selected difference without wrapping', () => {
  expect(nextDifference(blocks, 0, 'left', 1)).toBe(0);
  expect(nextDifference(blocks, 5, 'right', 1)).toBe(1);
  expect(nextDifference(blocks, 20, 'left', -1)).toBe(2);
  expect(nextDifference(blocks, 2, 'left', -1)).toBe(-1);
  expect(nextDifference(blocks, 0, 'left', 1, 0)).toBe(1);
  expect(nextDifference(blocks, 0, 'left', 1, 2)).toBe(2);
  expect(nextDifference(blocks, 0, 'left', -1, 0)).toBe(0);
  expect(nextDifference([], 0, 'left', 1)).toBe(-1);
});
it('expands matched folds on both sides without mutating caller state', () => {
  const folds: FoldRange[] = [{ left: range(0, 10), right: range(0, 10) }, { left: range(15, 30), right: range(18, 33) }];
  const before = new Set<number>(); const expanded = expandFold(before, 0);
  expect(before.size).toBe(0);
  expect(collapsedFolds(folds, expanded)).toEqual([{ id: 1, range: folds[1] }]);
  expect(expandFold(expanded, 0).size).toBe(1);
  expect(collapsedFolds(folds, expandFold(expanded, 1))).toEqual([]);
});
it('connects measured endpoints including insertion/deletion anchors and offscreen bands', () => {
  const geometry = connectorGeometry([20, 20], [30, 70], 40);
  expect(geometry.path).toBe('M0,20 C20,20 20,30 40,30 L40,70 C20,70 20,20 0,20 Z');
  expect(geometry).toMatchObject({ top: 20, bottom: 70, leftY: 20, rightY: 50 });
  expect(connectorGeometry([-100, -20], [10, 10], 40)).toMatchObject({ top: -100, bottom: 10 });
});
it('builds real CodeMirror line/inline/fold decorations from Rust-compatible ranges', () => {
  const state = EditorState.create({ doc: '🙂 old\r\nunchanged\r\ntail\n' });
  const result: DiffResult = { blocks: [{ ...block(0, 1, 0, 1), leftInline: [{ line: 0, start: 3, end: 6 }] }],
    folds: [{ left: range(1, 3), right: range(1, 3) }], alignment: [], leftLines: 3, rightLines: 3 };
  const decorations = diffDecorations(state, 'left', result, new Set(), () => {});
  const iter = decorations.iter(); const values = [];
  while (iter.value) { values.push({ from: iter.from, to: iter.to, spec: iter.value.spec }); iter.next(); }
  expect(values.some(d => d.from === 3 && d.to === 6 && d.spec.class === 'diff-inline diff-inline-left')).toBe(true);
  expect(values.some(d => d.spec.block === true)).toBe(true);
  const expanded = diffDecorations(state, 'left', result, new Set([0]), () => {});
  expect(expanded.size).toBe(2);
});

it('bounds connector work to visible sorted blocks including empty anchors', () => {
  expect(visibleBlockRange(blocks, range(4, 8), range(7, 11))).toEqual({ start: 1, end: 1 });
  expect(visibleBlockRange(blocks, range(3, 3), range(3, 6))).toEqual({ start: 0, end: 1 });
  expect(visibleBlockRange(blocks, range(14, 18), range(14, 18))).toEqual({ start: 1, end: 3 });
  expect(visibleBlockRange([], range(0, 1), range(0, 1))).toEqual({ start: 0, end: 0 });
});
