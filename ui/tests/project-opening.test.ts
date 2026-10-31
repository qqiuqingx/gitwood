import { beforeEach, expect, it, vi } from 'vitest';
import { render } from 'svelte/server';
const ipc = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: ipc }));
import { RepositoryBrowser } from '../src/features/browse/browser.svelte';
import { ProjectOpening } from '../src/features/projects/opening.svelte';
import OpenProjectDialog from '../src/features/projects/OpenProjectDialog.svelte';

beforeEach(() => { ipc.mockReset(); });
function currentProject() {
  const browser = new RepositoryBrowser();
  browser.project = { root: '/original', linked: false };
  browser.commit.message = 'unfinished draft';
  browser.selected = 'selected-commit';
  browser.scrollTop = 2412;
  return { browser, opening: new ProjectOpening(browser) };
}
it('asks before replacing a project and cancel retains the draft, selection and viewport', async () => {
  const { browser, opening } = currentProject();
  ipc.mockResolvedValue({ kind: 'choose', path: '/canonical/new' });
  await opening.request('/alias/new');
  expect(opening.pending).toBe('/canonical/new');
  expect(browser.project?.root).toBe('/original');
  opening.cancel();
  expect(opening.pending).toBeNull();
  expect(browser.commit.message).toBe('unfinished draft');
  expect(browser.selected).toBe('selected-commit'); expect(browser.scrollTop).toBe(2412);
  expect(ipc.mock.calls.map(([name]) => name)).toEqual(['open_project']);
});
it.each(['created', 'focused'])('%s leaves the originating project and draft intact', async kind => {
  const { browser, opening } = currentProject();
  ipc.mockResolvedValueOnce({ kind: 'choose', path: '/new' }).mockResolvedValueOnce({ kind });
  await opening.request('/new'); await opening.choose('new');
  expect(ipc).toHaveBeenLastCalledWith('open_project', { path: '/new', target: 'new' });
  expect(opening.pending).toBeNull(); expect(opening.busy).toBe(false);
  expect(browser.project?.root).toBe('/original'); expect(browser.commit.message).toBe('unfinished draft');
  expect(browser.selected).toBe('selected-commit'); expect(browser.scrollTop).toBe(2412);
});
it('opens the chosen project through the existing repository browser in the current window', async () => {
  const { browser, opening } = currentProject();
  ipc.mockImplementation((name: string, args: { target?: string }) => {
    if (name === 'open_project') return Promise.resolve(args.target ? { kind: 'current', path: '/new' } : { kind: 'choose', path: '/new' });
    if (name === 'open_repository') return Promise.resolve({ generation: 1, repository: { root: '/new', linked: false } });
    if (name === 'history_layout') return Promise.resolve({ generation: 2, total: 0 });
    if (name === 'recent_repositories') return Promise.resolve([]);
    return Promise.resolve({});
  });
  await opening.request('/new'); await opening.choose('current');
  expect(browser.project?.root).toBe('/new'); expect(browser.commit.message).toBe('');
  expect(ipc).toHaveBeenCalledWith('open_repository', { path: '/new' });
});
it('focuses an already open project without showing a choice or reopening the repository', async () => {
  const { browser, opening } = currentProject();
  ipc.mockResolvedValue({ kind: 'focused' });
  await opening.request('/existing');
  expect(opening.pending).toBeNull(); expect(browser.project?.root).toBe('/original');
  expect(ipc).toHaveBeenCalledTimes(1);
});
it('retains the project on discovery failure and blocks duplicate requests and writes in progress', async () => {
  const { browser, opening } = currentProject();
  let resolve!: (value: unknown) => void;
  ipc.mockReturnValue(new Promise(done => { resolve = done; }));
  const first = opening.request('/new'); await opening.request('/another');
  expect(ipc).toHaveBeenCalledTimes(1);
  resolve({ kind: 'choose', path: '/new' }); await first;
  await opening.request('/another'); expect(ipc).toHaveBeenCalledTimes(1);
  opening.cancel();
  ipc.mockImplementation((name: string) => name === 'recent_repositories' ? Promise.resolve([])
    : Promise.reject({ key: 'repository.unsafeDirectory', detail: 'unsafe' }));
  await opening.request('/unsafe');
  expect(browser.error?.key).toBe('repository.unsafeDirectory');
  expect(browser.project?.root).toBe('/original'); expect(browser.commit.message).toBe('unfinished draft');
  browser.commit.busy = true; await opening.request('/another');
  expect(ipc).toHaveBeenCalledWith('recent_repositories');
  expect(ipc).toHaveBeenCalledTimes(3);
});
it('shows both window choices and cancel, with escaped repository text', () => {
  Object.defineProperty(globalThis.navigator, 'language', { configurable: true, value: 'en-US' });
  const body = render(OpenProjectDialog, { props: { path: '/repo/<unsafe>', choose: () => {}, cancel: () => {} } }).body;
  expect(body).toContain('This Window'); expect(body).toContain('New Window'); expect(body).toContain('Cancel');
  expect(body).toContain('&lt;unsafe>'); expect(body).not.toContain('<unsafe>');
  expect(body).toContain('aria-describedby="open-project-message"');
});
