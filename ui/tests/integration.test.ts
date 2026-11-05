import { describe, expect, it, vi } from 'vitest';
import { render } from 'svelte/server';
import { emptyMergeOptions, integrationEnabled, mergeOptionEnabled, pullDefaults, validMergeOptions, type MergeFlag } from '../src/features/git/integration';
import { branchItem, type BranchPopupHandlers } from '../src/features/git/model';
import { integrationText as t } from '../src/i18n/integration';
import IntegrationDialog from '../src/features/git/IntegrationDialog.svelte';
import { RepositoryBrowser } from '../src/features/browse/browser.svelte';
import type { References } from '../src/features/browse/types';
import { createActionRegistry } from '../src/keymap/registry';
const local = (name: string, upstream: string | null = null) => ({ name: `refs/heads/${name}`, hash: 'a'.repeat(40), peeled: null, upstream, track: '' });
const refs: References = { local: [local('main', 'refs/remotes/origin/main'), local('topic')], remotes: { origin: [{ ...local('main'), name: 'refs/remotes/origin/main' }] }, tags: [], remote_heads: {}, head: { head: 'main', oid: 'a'.repeat(40), upstream: 'origin/main', ahead: 0, behind: 0 } };
describe('integration safety and IDEA option rules', () => {
  it('starts without flags and enforces IDEA mutual exclusions including an empty selected message', () => {
    const defaults = emptyMergeOptions(); expect(validMergeOptions(defaults)).toBe(true);
    const pairs: [MergeFlag, MergeFlag][] = [['noFf','ffOnly'],['noFf','squash'],['ffOnly','squash'],['ffOnly','message'],['squash','message'],['noCommit','message']];
    for (const [a,b] of pairs) {
      const selected = { ...defaults, [a]: a === 'message' ? 'message' : true };
      expect(mergeOptionEnabled(selected,b)).toBe(false);
      expect(mergeOptionEnabled({ ...defaults, [b]: b === 'message' ? '' : true },a)).toBe(false);
    }
    expect(validMergeOptions({ ...defaults, noFf: true, noCommit: true })).toBe(true);
    expect(validMergeOptions({ ...defaults, message: '' })).toBe(false);
    expect(validMergeOptions({ ...defaults, message: 'reviewed', noCommit: true })).toBe(false);
    for (const key of ['noFf','ffOnly','squash','noCommit','message'] as const) expect(mergeOptionEnabled(defaults,key,true,true)).toBe(false);
  });
  it('blocks integration during operations, conflicts, recovery, detached and unborn HEAD', () => {
    expect(integrationEnabled(refs,null,false)).toBe(true);
    for (const [operation,busy,conflicts,recovery] of [[{},false,0,null],[null,true,0,null],[null,false,1,null],[null,false,0,'stash']] as const) expect(integrationEnabled(refs,operation,busy,conflicts,recovery)).toBe(false);
    expect(integrationEnabled({ ...refs, head:{ ...refs.head,head:'(detached)' } },null,false)).toBe(false);
    expect(integrationEnabled({ ...refs, head:{ ...refs.head,oid:'(initial)' } },null,false)).toBe(false);
  });
  it('defaults Pull to tracked remote/branch and does not invent an unrelated branch', () => {
    expect(pullDefaults(refs)).toEqual({ remote:'origin',branches:['main'] });
    expect(pullDefaults({ ...refs, local:[local('other')],head:{ ...refs.head,head:'other' } })).toEqual({ remote:'origin',branches:[] });
  });
  it('wires real branch actions, upstream set/unset and current-branch restrictions', () => {
    const handlers: BranchPopupHandlers = { canIntegrate:()=>true,merge:vi.fn(),rebase:vi.fn(),checkoutRebase:vi.fn(),compare:vi.fn(),upstream:vi.fn(),unsetUpstream:vi.fn(),checkout:vi.fn(),checkoutRemote:vi.fn(),newFrom:vi.fn(),update:vi.fn(),push:vi.fn(),rename:vi.fn(),delete:vi.fn(),newBranch:vi.fn(),commit:vi.fn(),fetch:vi.fn() };
    const children = branchItem(refs.local[1],refs,handlers).children!;
    for (const [suffix,label,action] of [['merge','mergeCurrent','merge'],['rebase','rebaseCurrent','rebase'],['checkout-rebase','checkoutRebase','checkoutRebase']] as const) {
      const item = children.find(i=>i.id.endsWith(`:${suffix}`))!; expect(item.label).toBe(t(label)); expect(item.disabled).toBe(false); item.run!(); expect(handlers[action]).toHaveBeenCalledWith('refs/heads/topic');
    }
    children.find(i=>i.id.endsWith(':diff'))!.run!(); expect(handlers.compare).toHaveBeenCalledWith('refs/heads/topic',true);
    const current = branchItem(refs.local[0],refs,handlers).children!;
    expect(current.find(i=>i.id.endsWith(':merge'))?.disabled).toBe(true);
    current.find(i=>i.id.endsWith(':upstream'))!.run!(); expect(handlers.upstream).toHaveBeenCalledWith('main');
    current.find(i=>i.id.endsWith(':unset-upstream'))!.run!(); expect(handlers.unsetUpstream).toHaveBeenCalledWith('main');
    expect(children.find(i=>i.id.endsWith(':unset-upstream'))?.disabled).toBe(true);
  });
  it('renders localized options, disabled interactive rebase, and escaped revision text', () => {
    const browser = new RepositoryBrowser(); browser.refs = refs;
    const body = render(IntegrationDialog,{ props:{browser,kind:'rebase',seed:'<script>bad</script>',progress:'',onclose:()=>{},onresult:()=>{}} }).body;
    expect(body).toContain(t('interactive')); expect(body).toMatch(/type="checkbox" disabled/); expect(body).not.toContain('<script>bad</script>');
    const merge = render(IntegrationDialog,{ props:{browser,kind:'merge',progress:'',onclose:()=>{},onresult:()=>{}} }).body;
    for (const key of ['noFf','ffOnly','squash','noCommit','message','protect'] as const) expect(merge).toContain(t(key));
    expect(merge).toMatch(/<button[^>]*disabled[^>]*title=/);
  });
  it('registry dispatches menu actions only when enabled', () => {
    const registry = createActionRegistry(); const run=vi.fn(); let enabled=false;
    registry.register('Git.Merge',{run,enabled:()=>enabled}); expect(registry.run('Git.Merge')).toBe(false); expect(run).not.toHaveBeenCalled();
    enabled=true; expect(registry.run('Git.Merge')).toBe(true); expect(run).toHaveBeenCalledOnce();
  });
});
it('resolves tracking remotes containing slashes without splitting the remote name', () => {
  const upstream='refs/remotes/team/origin/main';
  expect(pullDefaults({ ...refs, local:[local('main',upstream)],remotes:{ 'team/origin':[{ ...local('main'), name:upstream }] } })).toEqual({remote:'team/origin',branches:['main']});
});
