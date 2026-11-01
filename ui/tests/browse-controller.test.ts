import { beforeEach, expect, it, vi } from 'vitest';
const ipc = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: ipc }));
import { RepositoryBrowser } from '../src/features/browse/browser.svelte';
import type { Rows } from '../src/features/browse/types';
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
const commit = (hash: string) => ({ hash, parents: [], author: '', email: '', author_time: 0, commit_time: 0, subject: hash, message: hash, refs: '' });
const rows = (generation: number, start: number, hash: string): Rows => ({ generation, nodes: [[start, 0, 0, 0, 0]], segments: [], commits: [commit(hash)], current: [false] });
beforeEach(() => { ipc.mockReset(); });
it('navigates to an uncached parent via history_position, loads the row and selects its hash', async () => {
  const browser = new RepositoryBrowser(); browser.project = { root: '/repo', linked: false }; browser.generation = 4; browser.total = 300;
  ipc.mockImplementation((name: string) => name === 'history_position' ? Promise.resolve(210)
    : name === 'history_rows' ? Promise.resolve(rows(4, 210, 'parent')) : Promise.resolve(commit('parent')));
  await browser.goToCommit('parent');
  expect(ipc).toHaveBeenCalledWith('history_position', { generation: 4, hash: 'parent' });
  expect(browser.selected).toBe('parent'); expect(browser.activeIndex).toBe(210); expect(browser.scrollTop).toBe(210 * 24);
});
it('does not navigate when a parent is filtered out or an old graph responds after refresh', async () => {
  const browser = new RepositoryBrowser(); browser.project = { root: '/repo', linked: false }; browser.generation = 4; browser.total = 300;
  browser.selected = 'selected'; browser.activeIndex = 10; browser.scrollTop = 240;
  ipc.mockResolvedValue(null); await browser.goToCommit('missing');
  expect(browser.selected).toBe('selected'); expect(browser.scrollTop).toBe(240);
  const pending = deferred<number | null>(); ipc.mockReturnValue(pending.promise);
  const navigation = browser.goToCommit('parent'); browser.generation = 5; pending.resolve(210); await navigation;
  expect(browser.selected).toBe('selected'); expect(browser.activeIndex).toBe(10); expect(browser.scrollTop).toBe(240);
  expect(ipc.mock.calls.every(([name]) => name === 'history_position')).toBe(true);
});
it('a late parent position does not override a newer row selection', async () => {
  const browser = new RepositoryBrowser(); browser.project = { root: '/repo', linked: false }; browser.generation = 4; browser.total = 300;
  const pending = deferred<number | null>();
  browser.rows = new Map([[10, { index: 10, column: 0, color: 0, segments: [], commit: commit('newer'), current: false }]]);
  ipc.mockImplementation((name: string) => name === 'history_position' ? pending.promise : Promise.resolve(commit('newer')));
  const navigation = browser.goToCommit('parent'); await browser.selectIndex(10); pending.resolve(210); await navigation;
  expect(browser.selected).toBe('newer'); expect(browser.activeIndex).toBe(10); expect(browser.scrollTop).toBe(0);
});
it('a late selected-commit detail cannot replace a later selection', async () => {
  const browser = new RepositoryBrowser(); browser.project = { root: '/repo', linked: false }; browser.generation = 4; browser.total = 2;
  browser.rows = new Map([0, 1].map(index => [index, { index, column: 0, color: 0, segments: [], commit: commit(String(index)), current: false }]));
  const first = deferred<ReturnType<typeof commit>>();
  ipc.mockImplementation((name: string, args: { hash: string }) => name === 'commit_details' ? args.hash === '0' ? first.promise : Promise.resolve(commit('1')) : Promise.reject(name));
  const old = browser.selectIndex(0); await browser.selectIndex(1); first.resolve(commit('0')); await old;
  expect(browser.selected).toBe('1'); expect(browser.details?.hash).toBe('1');
});
it('a late viewport response cannot replace a newer range or cross a generation', async () => {
  const browser = new RepositoryBrowser(); browser.project = { root: '/repo', linked: false }; browser.generation = 4; browser.total = 100;
  const old = deferred<Rows>();
  ipc.mockImplementation((_name: string, args: { start: number }) => args.start === 0 ? old.promise : Promise.resolve(rows(4, 40, 'new')));
  const pending = browser.viewport({ start: 0, end: 1, rowHeight: 24 });
  await browser.viewport({ start: 40, end: 41, rowHeight: 24 });
  old.resolve(rows(4, 0, 'old')); await pending;
  expect(browser.rows.has(0)).toBe(false); expect(browser.rows.get(40)?.commit.hash).toBe('new');
});
it('status-only refresh leaves graph, selection and scrolling intact', async () => {
  const browser = new RepositoryBrowser(); browser.project = { root: '/repo', linked: false }; browser.generation = 7;
  browser.selected = 'selected'; browser.activeIndex = 100; browser.scrollTop = 2407;
  ipc.mockResolvedValue({ branch: {}, staged: [], unstaged: [], conflicts: [], untracked: [] });
  await browser.refresh({ status: true, refs: false, history: false }, 7);
  expect(ipc).toHaveBeenCalledTimes(1); expect(ipc.mock.calls[0][0]).toBe('repository_status');
  expect(browser.selected).toBe('selected'); expect(browser.scrollTop).toBe(2407); expect(browser.activeIndex).toBe(100);
  await browser.refresh({ status: true, refs: true, history: true }, 6);
  expect(ipc).toHaveBeenCalledTimes(1);
});
it('close waits for in-flight discovery and prevents a late open from restoring the project', async () => {
  const browser = new RepositoryBrowser(), first = deferred<{ generation: number; repository: { root: string; linked: boolean } }>();
  ipc.mockImplementation((name: string) => name === 'open_repository' ? first.promise : Promise.resolve(name === 'recent_repositories' ? [] : 2));
  const opening = browser.open('/repo'); await Promise.resolve(); await Promise.resolve();
  const closing = browser.close();
  first.resolve({ generation: 1, repository: { root: '/repo', linked: false } });
  await Promise.all([opening, closing]);
  expect(ipc.mock.calls.map(call => call[0])).toEqual(['open_repository', 'close_repository']);
  expect(browser.project).toBeNull();
});
it('retains refresh events emitted with the new generation while layout IPC is in flight', async () => {
  const browser = new RepositoryBrowser(); browser.project = { root: '/repo', linked: false }; browser.generation = 7;
  const layout = deferred<{ generation: number; total: number }>();
  ipc.mockImplementation((name: string) => name === 'history_layout' ? layout.promise
    : name === 'repository_status' ? Promise.resolve({ branch: {}, changes: 1, untracked: 0 }) : Promise.reject(name));
  const filtering = browser.applyFilter({ ...browser.filter, text: 'new filter' });
  await browser.refresh({ status: true, refs: false, history: false }, 8);
  layout.resolve({ generation: 8, total: 0 }); await filtering;
  expect(browser.status?.changes).toBe(1);
  expect(ipc).toHaveBeenCalledWith('repository_status', { generation: 8 });
});
