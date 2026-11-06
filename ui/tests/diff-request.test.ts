import { expect, it } from 'vitest';
import { diffRequest, preservedScroll } from '../src/features/diff/request';
import { RequestGate } from '../src/features/browse/model';
const file = { path: { bytes: [255, 10], display: '�\n' }, hash: '', staged: true, unstaged: true };
it('maps byte paths and explicit areas without display conversion', () => {
 expect(diffRequest(file, 7).args).toEqual({ generation: 7, path: file.path, hash: '', area: 'unstaged' });
 expect(diffRequest(file, 7, 'staged').args.area).toBe('staged');
 expect(diffRequest({ ...file, staged: false, unstaged: false }, 7).args.area).toBe('untracked');
 expect(diffRequest({ ...file, hash: 'abc' }, 7).command).toBe('commit_diff');
});
it('invalidates an old request after selection or refresh', () => {
 const gate = new RequestGate(); const first = gate.next(); gate.next(); expect(gate.accepts(first)).toBe(false);
});
it('keeps scroll on content refresh, resets on file or area change', () => {
 expect(preservedScroll('a', 'a', 420)).toBe(420); expect(preservedScroll('a', 'b', 420)).toBe(0); expect(preservedScroll('a', 'b', 420, 96)).toBe(96); expect(preservedScroll('', 'b', 0, 96)).toBe(96);
});
