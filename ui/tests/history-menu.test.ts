import { expect, it, vi } from 'vitest';
import { historyMenuItems } from '../src/features/browse/history-menu';
import type { BranchPopupHandlers } from '../src/features/git/model';
import type { Commit, References } from '../src/features/browse/types';
const commit: Commit = { hash: 'a'.repeat(40), parents: ['b'.repeat(40)], author: '', email: '', author_time: 0, commit_time: 0, subject: '', message: '', refs: '' };
const reference = (name: string, hash = commit.hash) => ({ name, hash, peeled: null, upstream: null, track: '' });
const refs: References = { local: [reference('refs/heads/main'), reference('refs/heads/topic'), reference('refs/heads/other', 'other')],
  remotes: { origin: [reference('refs/remotes/origin/topic')] }, tags: [reference('refs/tags/tag')], remote_heads: {},
  head: { oid: commit.hash, head: 'main', upstream: null, ahead: 0, behind: 0 } };
const handlers: BranchPopupHandlers = { checkout: vi.fn(), checkoutRemote: vi.fn(), newFrom: vi.fn(), update: vi.fn(), push: vi.fn(), rename: vi.fn(), delete: vi.fn(), newBranch: vi.fn(), commit: vi.fn(), fetch: vi.fn() };
const actions = { copy: vi.fn(), parent: vi.fn() };
const build = (parent = true, references: References | null = refs, branches: BranchPopupHandlers | undefined = handlers, selected = commit) => historyMenuItems(selected, references, branches, parent, actions, key => key);
it('uses the IDEA merged menu order and leaves planned operations disabled', () => {
  const items = build();
  expect(items.map(item => item.id)).toEqual(['copy', 'patch', 'separator:checkout', 'checkout', 'browse', 'separator:compare', 'compare', 'localDiff', 'separator:reset', 'reset', 'revert', 'undo', 'separator:edit', 'edit', 'fixup', 'squashInto', 'drop', 'squash', 'rebase', 'push', 'separator:branch', 'branch', 'newBranch', 'tag', 'separator:navigate', 'child', 'parent']);
  const live = new Set(['copy', 'checkout', 'branch', 'newBranch', 'parent']);
  for (const item of items.filter(item => !item.kind)) expect(!item.disabled, item.id).toBe(live.has(item.id));
  expect(items[0].shortcut).toBeUndefined(); items[0].run!(); expect(actions.copy).toHaveBeenCalled();
  items.find(item => item.id === 'newBranch')!.run!(); expect(handlers.newFrom).toHaveBeenCalledWith(commit.hash);
  items.find(item => item.id === 'parent')!.run!(); expect(actions.parent).toHaveBeenCalled();
});
it('uses only branches at the selected commit and reuses branch operation handlers', () => {
  const items = build(); const branches = items.find(item => item.id === 'branch')!.children!;
  expect(branches.map(item => item.id).sort()).toEqual(['refs/heads/main', 'refs/heads/topic', 'refs/remotes/origin/topic']);
  const checkout = items.find(item => item.id === 'checkout')!.children!;
  expect(checkout.find(item => item.id === 'checkout:refs/heads/main')?.disabled).toBe(true);
  checkout.find(item => item.id === 'checkout:refs/heads/topic')!.run!(); expect(handlers.checkout).toHaveBeenCalledWith('topic');
  checkout.find(item => item.id === 'checkout:refs/remotes/origin/topic')!.run!(); expect(handlers.checkoutRemote).toHaveBeenCalledWith('origin', 'topic');
  expect(checkout.find(item => item.id === 'revision')?.disabled).toBe(true);
  const topic = branches.find(item => item.id === 'refs/heads/topic')!.children!;
  topic.find(item => item.id.endsWith(':rename'))!.run!(); expect(handlers.rename).toHaveBeenCalledWith('topic');
  topic.find(item => item.id.endsWith(':delete'))!.run!(); expect(handlers.delete).toHaveBeenCalledWith('topic', false);
});
it('disables navigation for roots/unavailable parents and branches without refs/handlers', () => {
  expect(build(false).find(item => item.id === 'parent')?.disabled).toBe(true);
  expect(build(true, refs, handlers, { ...commit, parents: [] }).find(item => item.id === 'parent')?.disabled).toBe(true);
  for (const items of [build(true, null), build(true, refs, undefined, { ...commit, hash: 'unreferenced' })]) {
    expect(items.find(item => item.id === 'branch')?.disabled).toBe(true); expect(items.find(item => item.id === 'checkout')?.disabled).toBe(true);
  }
  expect(historyMenuItems(commit, refs, undefined, true, actions, key => key).find(item => item.id === 'newBranch')?.disabled).toBe(true);
});

it('enables undo only for an eligible HEAD with complete branch/state information', () => {
  const undo = (selected = commit, references: References | null = refs, info: import('../src/features/browse/history-menu').UndoInfo | null = { branches: [], total: 0, protected: false, inProgress: false }) =>
    historyMenuItems(selected, references, handlers, true, actions, key => key, { info, run: actions.copy, busy: false }).find(item => item.id === 'undo')!;
  expect(undo().disabled).toBe(false);
  expect(undo({ ...commit, hash: 'other' }).hint).toBe('undoHead');
  expect(undo(commit, { ...refs, head: { ...refs.head, oid: null } }).hint).toBe('undoHead');
  expect(undo({ ...commit, parents: [] }).hint).toBe('undoRoot');
  expect(undo({ ...commit, parents: ['a', 'b'] }).hint).toBe('undoMerge');
  expect(undo(commit, refs, null).hint).toBe('undoLoading');
  expect(undo(commit, null).hint).toBe('undoLoading');
  expect(undo(commit, refs, { branches: [], total: 0, protected: false, inProgress: true }).hint).toBe('undoState');
  for (const branch of ['refs/remotes/origin/main', 'refs/remotes/upstream/master']) {
    expect(undo(commit, refs, { branches: [branch], total: 1, protected: false, inProgress: false }).hint).toBe('undoProtected');
  }
  for (const branch of ['refs/heads/main', 'refs/remotes/origin/topic', 'refs/remotes/origin/not-main', 'refs/remotes/origin/topic/main']) {
    expect(undo(commit, refs, { branches: [branch], total: 1, protected: false, inProgress: false }).disabled).toBe(false);
  }
});
it('uses the full backend protection result even when the visible containing branches are capped', () => {
  for (const protectedBranch of [true, false]) {
    const item = historyMenuItems(commit, refs, handlers, true, actions, key => key, {
      info: { branches: [], total: 250, protected: protectedBranch, inProgress: false }, run: actions.copy, busy: false,
    }).find(item => item.id === 'undo')!;
    expect(item.disabled).toBe(protectedBranch);
  }
});
