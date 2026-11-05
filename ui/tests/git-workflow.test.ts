import { describe, expect, it } from 'vitest';
import { branchPopupItems, trackCounts, commitPushFeedback, defaultPushOptions, defaultUpdateStrategy, isNewPushTarget, pushPageRanges, pushRejectionText, remoteErrorText, updateRequest } from '../src/features/git/model';
import { integrationText as t } from '../src/i18n/integration';
import type { References } from '../src/features/browse/types';

const refs: References = {
  local: [
    { name: 'refs/heads/main', hash: '1', peeled: null, upstream: 'refs/remotes/origin/main', track: 'ahead 2, behind 1' },
    { name: 'refs/heads/topic', hash: '2', peeled: null, upstream: null, track: '' },
  ],
  remotes: { origin: [{ name: 'refs/remotes/origin/main', hash: '1', peeled: null, upstream: null, track: '' }] },
  tags: [{ name: 'refs/tags/v0.1', hash: '1', peeled: null, upstream: null, track: '' }],
  remote_heads: {}, head: { oid: '1', head: 'main', upstream: 'refs/remotes/origin/main', ahead: 2, behind: 1 },
};

describe('Git workflow model', () => {
  it('groups recent, local, remote and tags and disables current checkout/delete and P1 actions', () => {
    const items = branchPopupItems(refs, { canIntegrate: () => true, merge: () => {}, rebase: () => {}, checkoutRebase: () => {}, compare: () => {}, upstream: () => {}, unsetUpstream: () => {}, checkout: () => {}, checkoutRemote: () => {}, newFrom: () => {}, update: () => {}, push: () => {}, rename: () => {}, delete: () => {}, newBranch: () => {}, commit: () => {}, fetch: () => {} });
    expect(items.map(item => item.id)).toContain('section:recent');
    const main = items.find(item => item.id === 'refs/heads/main')!;
    expect(main.children?.find(item => item.id.endsWith(':checkout'))?.disabled).toBe(true);
    expect(main.children?.find(item => item.id.endsWith(':delete'))?.disabled).toBe(true);
    expect(main.children?.find(item => item.id.endsWith(':compare'))?.label).toBe(t('compare'));
    expect(main.children?.find(item => item.id.endsWith(':merge'))?.disabled).toBe(true);
  });
  it('groups slash prefixes into IDEA folders that expand in place, current and favorites first', () => {
    const handlers = { canIntegrate: () => true, merge: () => {}, rebase: () => {}, checkoutRebase: () => {}, compare: () => {}, upstream: () => {}, unsetUpstream: () => {}, checkout: () => {}, checkoutRemote: () => {}, newFrom: () => {}, update: () => {}, push: () => {}, rename: () => {}, delete: () => {}, newBranch: () => {}, commit: () => {}, fetch: () => {} };
    const ref = (name: string) => ({ name, hash: '1', peeled: null, upstream: null, track: '' });
    const tree: References = { ...refs, local: [ref('refs/heads/zeta'), ref('refs/heads/codex/a'), ref('refs/heads/codex/b'), ref('refs/heads/master'), ref('refs/heads/main')] };
    const toggled: string[] = [];
    const collapsed = branchPopupItems(tree, handlers, new Set(), id => toggled.push(id));
    const local = collapsed.slice(collapsed.findIndex(item => item.id === 'section:local') + 1, collapsed.findIndex(item => item.id === 'section:remote'));
    expect(local.map(item => item.label)).toEqual(['main', 'master', 'codex', 'zeta']);
    const folder = local.find(item => item.label === 'codex')!;
    expect(folder.keepOpen).toBe(true);
    folder.run!();
    expect(toggled).toEqual(['folder:local/codex']);
    const open = branchPopupItems(tree, handlers, new Set(['folder:local/codex', 'folder:remote/origin']));
    expect(open.filter(item => item.level === 2 && item.id.startsWith('refs/heads/')).map(item => item.label)).toEqual(['a', 'b']);
    expect(open.find(item => item.id === 'refs/remotes/origin/main')?.label).toBe('main');
    expect(collapsed.some(item => item.id === 'refs/remotes/origin/main')).toBe(false);
    const folded = branchPopupItems(tree, handlers, new Set(['section:local']));
    expect(folded.some(item => item.id === 'refs/heads/zeta')).toBe(false);
    expect(folded.find(item => item.id === 'section:local')?.keepOpen).toBe(true);
  });
  it('parses upstream track counts', () => {
    expect(trackCounts('ahead 2, behind 105')).toEqual({ ahead: 2, behind: 105 });
    expect(trackCounts('gone')).toEqual({ ahead: 0, behind: 0 });
  });
  it('uses upstream remote/branch and leaves force lease and tags off', () => {
    expect(defaultPushOptions(refs)).toEqual({ remote: 'origin', branch: 'main', forceWithLease: false, tags: false, upTo: null });
    expect(isNewPushTarget(refs, { remote: 'origin', branch: 'topic', forceWithLease: false, tags: false })).toBe(true);
    expect(isNewPushTarget(refs, { remote: 'origin', branch: 'main', forceWithLease: false, tags: false })).toBe(false);
  });
  it('loads PushPreview in bounded pages', () => {
    expect(pushPageRanges(405)).toEqual([{ start: 0, limit: 200 }, { start: 200, limit: 200 }, { start: 400, limit: 5 }]);
  });
  it('defaults Update to merge unless backend configuration reports rebase', () => {
    expect(defaultUpdateStrategy()).toBe('merge'); expect(defaultUpdateStrategy('rebase')).toBe('rebase'); expect(defaultUpdateStrategy('true')).toBe('rebase');
  });
  it('maps the selected Update strategy to the IPC request', () => {
    expect(updateRequest('merge')).toEqual({ strategy: 'merge' });
    expect(updateRequest('rebase')).toEqual({ strategy: 'rebase' });
  });
  it('maps remote error categories to actionable v0.1 guidance', () => {
    expect(remoteErrorText({ key: 'remote.authentication', guidance: 'remote.tryFetchInTerminal' }).title).toBe('Authentication required');
    expect(remoteErrorText({ key: 'remote.hostKey' }).action).toContain('remote configuration');
  });
  it('maps rejected refs and two-stage commit/push results separately', () => {
    expect(pushRejectionText('needsUpdate')).toContain('update first');
    expect(commitPushFeedback({ kind: 'committed', hash: 'a', push: { kind: 'push', success: false, refs: [] } })).toEqual({ commit: 'a', push: 'failed' });
    expect(commitPushFeedback({ kind: 'recovery', hash: 'a', directory: '/tmp', instructions: 'x', reason: 'index' })).toEqual({ commit: 'a', push: 'recovery' });
  });
});
