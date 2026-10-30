import { afterEach, expect, it, vi } from 'vitest';
import { EditorState, type RangeSet } from '@codemirror/state';
import { gutter, type EditorView, type GutterMarker } from '@codemirror/view';
import { editorExtensions, inclusionGutter, rollbackGutter, lineBoundary } from '../src/features/diff/editor';
import type { DiffResult } from '../src/features/diff/types';

// Observe public gutter configuration while still constructing the real CM extension.
vi.mock('@codemirror/view', async importOriginal => {
  const actual = await importOriginal<typeof import('@codemirror/view')>();
  return { ...actual, gutter: vi.fn(actual.gutter) };
});
afterEach(() => vi.unstubAllGlobals());
const result: DiffResult = {
  blocks: [
    { kind: 'modify', left: { start: 1, end: 2 }, right: { start: 3, end: 4 }, leftInline: [], rightInline: [] },
    { kind: 'delete', left: { start: 3, end: 5 }, right: { start: 5, end: 5 }, leftInline: [], rightInline: [] },
  ], folds: [], alignment: [], leftLines: 5, rightLines: 5,
};
function markers(side: 'left' | 'right', disabled = false) {
  const run = vi.fn();
  const extension = side === 'left' ? rollbackGutter(result, disabled, run) : inclusionGutter(result, [1], disabled, run);
  const state = EditorState.create({ doc: 'zero\none\ntwo\nthree\nfour', extensions: [editorExtensions(side), extension] });
  const config = vi.mocked(gutter).mock.lastCall![0];
  const ranges = config.markers!({ state } as EditorView) as RangeSet<GutterMarker>;
  const entries: { from: number; marker: GutterMarker }[] = [];
  for (const cursor = ranges.iter(); cursor.value; cursor.next()) entries.push({ from: cursor.from, marker: cursor.value });
  return { config, state, entries, run };
}
it('puts native left line numbers and rollback after the content, inclusion before right content', () => {
  const numbers = vi.mocked(gutter).mock.calls.find(([config]) => config.class === 'cm-lineNumbers')![0];
  expect(numbers.side).toBe('after');
  expect(numbers.lineMarker!({ state: EditorState.create({ doc: 'a\nb' }) } as EditorView, { from: 2 } as never, [])).not.toBeNull();
  const left = markers('left'), right = markers('right');
  expect(left.config.side).toBe('after'); expect(left.config.class).toBe('diff-rollback');
  expect(right.config.side ?? 'before').toBe('before'); expect(right.config.class).toBe('commit-inclusion');
  expect(left.entries.map(entry => entry.from)).toEqual([left.state.doc.line(2).from, left.state.doc.line(4).from]);
  expect(right.entries.map(entry => entry.from)).toEqual([right.state.doc.line(4).from, right.state.doc.line(5).from]);
});
it('keeps inclusion indices, chosen state and busy state when right spans differ or end at EOF', () => {
  vi.stubGlobal('document', { createElement: () => ({ setAttribute: vi.fn() }) });
  const right = markers('right', true);
  const first = right.entries[0].marker.toDOM!({} as EditorView) as HTMLInputElement;
  const last = right.entries[1].marker.toDOM!({} as EditorView) as HTMLInputElement;
  expect(first.checked).toBe(false); expect(last.checked).toBe(true); expect(last.disabled).toBe(true);
  last.onchange!({} as Event); expect(right.run).toHaveBeenCalledWith(1);
  const left = markers('left', true);
  const arrow = left.entries[0].marker.toDOM!({} as EditorView) as HTMLButtonElement;
  expect(arrow.textContent).toBe('»'); expect(arrow.disabled).toBe(true); expect(arrow.title).toBeTruthy();
  expect(arrow.setAttribute).toHaveBeenCalledWith('aria-label', expect.any(String));
});
it('uses measured line/fold boundaries without double-counting content padding at EOF', () => {
  const state = EditorState.create({ doc: 'one\ntwo' });
  const lineBlockAt = vi.fn((position: number) => position === 0 ? { top: 0, bottom: 20 } : { top: 20, bottom: 40 });
  const view = { state, lineBlockAt, contentHeight: 48, documentPadding: { top: 4, bottom: 4 } } as unknown as EditorView;
  expect(lineBoundary(view, 0)).toBe(0);
  expect(lineBoundary(view, 1)).toBe(20);
  expect(lineBoundary(view, 2)).toBe(40);
  expect(lineBlockAt).toHaveBeenLastCalledWith(state.doc.length);
  lineBlockAt.mockReturnValue({ top: 0, bottom: 20 }); // Two logical lines collapsed into one block widget.
  expect(lineBoundary(view, 2)).toBe(20);
});
