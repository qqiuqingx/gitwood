<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { tick, untrack } from 'svelte';
  import Tabs, { type Tab } from '../../components/Tabs.svelte';
  import { ideaIcon } from '../../components/icons/idea';
  import { appearance } from '../../theme/appearance.svelte';
  import { filePathParts, fileTypeIcon } from '../browse/fileTree';
  import { blockSelection, toggleBlock, toggleRule, pathKey, type Hunks } from '../commit/model';
  import { commitText as c } from '../../i18n/commit';
  import { browseText as b } from '../../i18n/browse';
  import { diffText as t } from '../../i18n/diff';
  import { diffRequest } from './request';
  import DiffView, { type DiffNavigation } from './DiffView.svelte';
  import type { Content, DiffResult } from './types';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import { activeDiffTabs, diffTabKey, type DiffArea, type DiffTab } from './tabs.svelte';

  let { browser, ratio = $bindable(0.5) }: { browser: RepositoryBrowser; ratio?: number } = $props();
  interface PreviewValue { left: Content; right: Content; result: DiffResult | null; leftLabel: string; rightLabel: string; conflicted: boolean; token?: string; selectable?: boolean }
  let value = $state<PreviewValue | null>(null);
  let navigation = $state<DiffNavigation | null>(null);
  let rollbackDialog = $state<HTMLDialogElement>(null!);
  let pending = $state<Hunks | null>(null);
  let loadedKey = $state('');
  const session = $derived(browser.commit);
  const activeTab = $derived(activeDiffTabs(browser.diffTabs.tabs, browser.diffTabs.activeId)[0] ?? null);
  const chosen = $derived(activeTab && value?.token ? blockSelection(session.selection, activeTab.path, value.token,
    value.result?.blocks.length ?? 0, !activeTab.staged && !activeTab.unstaged) : []);
  const items = $derived(browser.diffTabs.tabs.map(tab => ({
    id: tab.id,
    label: `${fileName(tab.path.display)} (${displayRevision(tab.leftRevision)} ↔ ${displayRevision(tab.rightRevision)})`,
    title: `${tab.path.display} (${tooltipRevision(tab.leftRevision)} ↔ ${tooltipRevision(tab.rightRevision)})`,
    icon: ideaIcon(fileTypeIcon(filePathParts(tab.path.display).name), appearance.dark),
    preview: tab.preview,
    closeLabel: t('closeTab'),
  } satisfies Tab)));

  function fileName(path: string) { return path.slice(path.lastIndexOf('/') + 1); }
  function shortRevision(revision: string) { return /^[a-f0-9]{12,64}$/i.test(revision) ? revision.slice(0, 8) : revision; }
  function displayRevision(revision: string) {
    if (revision === 'Working tree') return b('workingTree');
    if (revision === 'Staged') return b('staged');
    if (revision === 'Empty') return t('emptyRevision');
    return shortRevision(revision);
  }
  function tooltipRevision(revision: string) {
    if (revision === 'Working tree') return b('workingTree');
    if (revision === 'Staged') return b('staged');
    if (revision === 'Empty') return t('emptyRevision');
    return revision;
  }
  function textCount(count: number) { return t('differences').replace('{count}', String(count)); }
  async function confirmRollback(index: number) {
    const tab = activeTab;
    if (!tab || !value?.token || session.busy) return;
    pending = { path: tab.path, token: value.token, blocks: [index] };
    await tick(); rollbackDialog.showModal();
  }
  async function rollback() {
    if (!pending) return;
    session.busy = true; session.task = c('revert');
    try { await invoke('rollback_hunk', { generation: browser.generation, hunk: pending }); rollbackDialog.close(); }
    catch (error) { browser.report(error); }
    finally { session.busy = false; await browser.refresh({ status: true, refs: true, history: true }); }
  }

  $effect(() => {
    const tab = activeTab, project = browser.project, generation = browser.generation;
    const statusRevision = tab?.source === 'worktree' ? browser.statusRevision : -1;
    if (!tab || !project) { value = null; loadedKey = ''; return; }
    const id = tab.id, key = `${id}:${diffTabKey(tab)}`;
    let alive = true;
    void statusRevision;
    // A status refresh reloads the same tab in place; only a new tab or area clears the view.
    if (untrack(() => loadedKey) !== key) { value = null; navigation = null; loadedKey = key; }
    const request = tab.source === 'worktree' && tab.area === 'commit'
      ? { command: 'selection_preview', args: { generation, path: tab.path } }
      : diffRequest({ path: tab.path, hash: tab.source === 'commit' ? tab.hash : '', staged: tab.staged, unstaged: tab.unstaged }, generation,
        tab.source === 'commit' ? '' : tab.area);
    void invoke<PreviewValue>(request.command, request.args)
      .then(result => { if (alive && browser.project === project && browser.generation === generation && browser.diffTabs.activeId === id) { value = result; browser.diffTabs.update(id, { empty: false, error: '' }); } })
      .catch(error => {
        if (!alive || browser.project !== project || browser.generation !== generation || browser.diffTabs.activeId !== id) return;
        value = null;
        const key = typeof error === 'string' ? error : typeof error === 'object' && error !== null ? (error as { key?: string }).key ?? '' : '';
        if (key === 'repository.stale') return;
        if (tab.preview && tab.source === 'worktree' && key === 'repository.noLongerChanged') {
          const current = browser.previewFile;
          if (current && current.path.bytes.join() === tab.path.bytes.join()) browser.previewFailed(current, error);
          browser.diffTabs.close(id);
        } else if (key === 'repository.noLongerChanged') browser.diffTabs.update(id, { empty: true, error: '' });
        else browser.diffTabs.update(id, { empty: false, error: typeof error === 'object' && error !== null ? String((error as { detail?: string }).detail ?? key ?? error) : String(error) });
      });
    return () => { alive = false; };
  });

  function close(id: string) {
    browser.diffTabs.close(id);
    if (!browser.diffTabs.tabs.some(tab => tab.preview)) browser.previewFile = null;
  }
  function pin(id: string) { browser.diffTabs.pin(id); }
  function onKey(event: KeyboardEvent) {
    if (event.defaultPrevented || !(event.target instanceof HTMLElement) || !event.target.closest('.diff-tab,[role="tablist"]')) return;
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'd' && activeTab) {
      event.preventDefault(); document.querySelector<HTMLElement>('.diff-view .cm-content')?.focus();
    }
  }
</script>

<svelte:window onkeydown={onKey} />
<Tabs label={t('diff')} tabs={items} activeId={browser.diffTabs.activeId} onchange={id => browser.diffTabs.select(id)} onclose={close} onpin={pin}>
  {#snippet panel(id)}
    {@const tab = browser.diffTabs.tabs.find(entry => entry.id === id)}
    {#if tab && tab.id === browser.diffTabs.activeId}
      <section class="diff-tab" aria-label={t('diff')}>
        <div class="toolbar" role="toolbar" aria-label={t('diff')}>
          <button type="button" aria-label={t('previous')} title={`${t('previous')} (Shift+F7)`} disabled={!navigation?.canPrevious()} onclick={() => navigation?.previous()}>
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 13V3M4 7l4-4 4 4" /></svg>
          </button>
          <button type="button" aria-label={t('next')} title={`${t('next')} (F7)`} disabled={!navigation?.canNext()} onclick={() => navigation?.next()}>
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 3v10M4 9l4 4 4-4" /></svg>
          </button>
          {#if tab.source === 'worktree'}
            <select aria-label={t('diff')} value={tab.area} onchange={event => browser.diffTabs.setArea(tab.id, event.currentTarget.value as DiffArea)}>
              <option value="commit">HEAD ↔ {b('changes')}</option>
              <option value="staged" disabled={!tab.staged}>{b('staged')}</option>
              <option value="unstaged" disabled={!tab.unstaged}>{b('unstaged')}</option>
            </select>
          {/if}
          <span class="grow"></span><span class="count" aria-live="polite">{textCount(value?.result?.blocks.length ?? 0)}</span>
        </div>
        {#if tab.error}<p role="alert">{tab.error}</p>
        {:else if tab.empty}<p>{t('noDiff')}</p>
        {:else if value && loadedKey === `${tab.id}:${diffTabKey(tab)}`}
          {#if value.conflicted}<p>{b('conflicted')}</p>
          {:else}
            <div class="body"><DiffView compactNavigation bind:ratio bind:navigation bind:position={tab.position}
              leftLabel={displayRevision(value.leftLabel)} rightLabel={displayRevision(value.rightLabel)}
              included={chosen} disabled={session.busy}
              onblock={value.token && tab.source === 'worktree' && tab.area === 'commit' ? index => {
                if (session.busy || !value?.token) return;
                if (value.selectable) session.selection = toggleBlock(session.selection, tab.path, value.token, value.result?.blocks.length ?? 0, index);
                else session.selection = toggleRule(session.selection, `${tab.staged || tab.unstaged ? 'change' : 'untracked'}/${pathKey(tab.path)}`, chosen.includes(index) ? 'all' : 'none');
              } : undefined}
              onrevert={value.selectable && tab.source === 'worktree' && tab.area === 'commit' ? confirmRollback : undefined}
              fileKey={loadedKey} left={value.left} right={value.right}
              result={value.result ?? { blocks: [], alignment: [], folds: [], leftLines: 0, rightLines: 0 }} /></div>
          {/if}
        {:else}<p>{b('loading')}</p>{/if}
      </section>
    {:else if !tab}
      <p class="placeholder">{b('diffPlaceholder')}</p>
    {/if}
  {/snippet}
</Tabs>
{#if pending}<dialog bind:this={rollbackDialog} aria-labelledby="rollback-lines-title" oncancel={event => { if (session.busy) event.preventDefault(); }}><h2 id="rollback-lines-title">{c('linesQuestion')}</h2><p>{pending.path.display}</p><button disabled={session.busy} onclick={() => rollbackDialog.close()}>{c('cancel')}</button><button disabled={session.busy} onclick={rollback}>{c('confirm')}</button></dialog>{/if}
<style>
  .diff-tab { height: 100%; display: flex; flex-direction: column; min-height: 0; }
  .toolbar { display: flex; align-items: center; gap: var(--space-inline); min-height: calc(var(--font-ui-size) + 2 * var(--space-panel-y)); padding: var(--space-panel-y) var(--space-panel-x); border-bottom: var(--border-width) solid var(--separator); }
  .toolbar button { display: grid; place-items: center; width: calc(var(--font-ui-size) + 2 * var(--space-inline)); height: calc(var(--font-ui-size) + 2 * var(--space-inline)); padding: 0; border: 0; color: var(--text-secondary); background: transparent; cursor: pointer; }
  .toolbar button:hover:not(:disabled) { color: var(--text-primary); background: var(--selection-bg); }
  .toolbar button:disabled { opacity: var(--tab-disabled-opacity, 0.5); }
  .toolbar button:focus-visible, select:focus-visible { outline: var(--focus-width) solid var(--focus-border); outline-offset: calc(-1 * var(--focus-width)); }
  .toolbar svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 1.5; stroke-linecap: round; stroke-linejoin: round; }
  .toolbar select { font: inherit; color: var(--text-primary); background: var(--input-bg); border: var(--border-width) solid var(--input-border); }
  .grow { flex: 1; }
  .count { color: var(--text-secondary); white-space: nowrap; }
  .body { flex: 1; min-height: 0; }
  p, .placeholder { padding: var(--space-panel-x); color: var(--text-secondary); }
  dialog { color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); }
</style>
