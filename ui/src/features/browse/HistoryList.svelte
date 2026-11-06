<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { UndoInfo } from './history-menu';
  import List from '../../components/List.svelte';
  import Highlight from '../../components/Highlight.svelte';
  import GraphCanvas from './GraphCanvas.svelte';
  import Filters from './Filters.svelte';
  import Popup from '../../components/Popup.svelte';
  import type { Rect } from '../../components/popup';
  import type { BranchPopupHandlers } from '../git/model';
  import type { Commit } from './types';
  import { forSelection, historyMenuItems } from './history-menu';
  import { placeholder } from './browser.svelte';
  import { historyMenuText } from '../../i18n/history-menu';
  import { formatDate, refLabelsByHash } from './model';
  import { hostingMenuItems } from '../git/hosting-menu';
  import { hostingHandlers, publishedHostingRemotes } from '../git/hosting-actions';
  import { browseText as t, prettyDateWords } from '../../i18n/browse';
  import type { RepositoryBrowser } from './browser.svelte';
  let { browser, handlers }: { browser: RepositoryBrowser; handlers?: BranchPopupHandlers } = $props();
  let context = $state<{ commit: Commit; anchor: Rect; generation: number; selection: string[] } | null>(null);
  let undoInfo = $state<UndoInfo | null>(null);
  let parentAvailable = $state(false);
  let selectionHasMerge = $state<boolean | null>(null);
  const contextItems = $derived.by(() => {
    const commit = context?.commit;
    const count = context?.selection.length ?? 0;
    if (!commit) return [];
    const hosted = hostingHandlers(() => browser);
    const extraHosting = hostingMenuItems(publishedHostingRemotes(browser), { kind: 'commit', revision: commit.hash }, hosted.open, hosted.copy);
    return forSelection(historyMenuItems(commit, browser.refs, handlers, parentAvailable,
      { copy: () => { void copyRevision(count > 1 ? context!.selection.join(' ') : commit.hash); },
        parent: () => { if (commit.parents[0]) void browser.goToCommit(commit.parents[0]); },
        run: (action, hash) => handlers?.historyAction?.(action, hash, count > 1 ? context!.selection : [hash]) }, historyMenuText,
      { info: undoInfo, busy: browser.commit.busy, writable: !!undoInfo && (!undoInfo.operation || undoInfo.operation.kind === 'bisect') && (handlers?.canHistoryWrite?.() ?? (!!browser.refs?.head.head && browser.refs.head.head !== '(detached)' && !!browser.refs.head.oid && browser.refs.head.oid !== '(initial)' && !browser.status?.operation && !browser.status?.conflicts && !browser.status?.update_stash && !browser.commit.busy)), applyAllowed: handlers?.canHistoryApply?.() ?? (!!browser.status && browser.status.staged === 0),
        run: () => { void browser.undoCommit(commit.hash); } }, Math.max(count, 1), selectionHasMerge, extraHosting), count);
  });
  /** Log order where rows are loaded (IDEA copies in table order); others keep selection order. */
  function orderedSelection() {
    const position = new Map([...browser.rows.values()].map(row => [row.commit.hash, row.index]));
    return [...browser.selection].sort((a, b) => (position.get(a) ?? Number.MAX_SAFE_INTEGER) - (position.get(b) ?? Number.MAX_SAFE_INTEGER));
  }
  async function copyRevision(hash: string) {
    try { await navigator.clipboard.writeText(hash); } catch (error) { browser.report({ detail: String(error) }); }
  }
  // A refresh/filter or repository switch invalidates the captured row and parent position.
  $effect(() => { if (context && context.generation !== browser.generation) context = null; });
  let contextRequest = 0;
  function openContext(event: MouseEvent, index: number) {
    event.preventDefault();
    const commit = browser.rows.get(index)?.commit;
    if (!commit) return;
    const request = ++contextRequest;
    const selection = browser.selection.has(commit.hash) && browser.selection.size > 1 ? orderedSelection() : [commit.hash];
    const opened = { commit, selection, anchor: { x: event.clientX, y: event.clientY, width: 0, height: 0 }, generation: browser.generation };
    context = opened; parentAvailable = false; selectionHasMerge = null; undoInfo = null;
    const selectedHashes = selection;
    void Promise.all([
      invoke<{ branches: string[]; total: number; protected: boolean }>('containing_branches', { generation: opened.generation, hash: commit.hash }),
      invoke<UndoInfo['operation']>('operation_state', { generation: opened.generation }),
      invoke<boolean>('history_revert_has_merge', { generation: opened.generation, hashes: selectedHashes }),
    ]).then(([branches, operation, hasMerge]) => {
      if (context !== null && request === contextRequest && browser.generation === opened.generation) {
        undoInfo = { ...branches, operation }; selectionHasMerge = hasMerge;
      }
    }).catch(error => { if (context !== null && request === contextRequest && browser.generation === opened.generation) browser.report(error); });
    if (commit.parents[0]) void browser.historyPosition(commit.parents[0]).then(position => {
      if (context?.commit.hash === opened.commit.hash && context.generation === opened.generation) parentAvailable = position !== null;
    }).catch(error => { if (context?.commit.hash === opened.commit.hash && context.generation === opened.generation) browser.report(error); });
  }
  const rows = $derived([...browser.rows.values()]);
  const graphWidth = $derived(Math.max(58, ...rows.map(row => Math.max(row.column, ...row.segments.flatMap(segment => [segment[1], segment[2]])) * 16 + 32)));
  const selected = $derived(browser.selection.size ? browser.selection : new Set(browser.selected ? [browser.selected] : []));
  const refsByHash = $derived(refLabelsByHash(browser.refs));
</script>
<section aria-label={t('log')}>
  <Filters value={browser.filter} refs={browser.refs} apply={value => { void browser.applyFilter(value); }} refresh={() => { void browser.refresh({ status: false, refs: false, history: true }); }} />
  <div class="list" aria-busy={browser.loading}>
    <List label={t('log')} multi rowCount={browser.total} minRowWidth={graphWidth + 430} search={false} selectedIds={selected} activeIndex={browser.activeIndex}
      bind:scrollTop={browser.scrollTop} getRow={index => ({ id: browser.rows.get(index)?.commit.hash ?? `${placeholder}${index}`, label: browser.rows.get(index)?.commit.subject ?? t('loading') })}
      onviewportchange={range => { void browser.viewport(range); }} onselectionchange={(ids, index) => { void browser.setSelection(ids, index); }}
      oncontextmenu={(event, _, index) => openContext(event, index)}>
      {#snippet row(_, index, query)}
        {@const commit = browser.rows.get(index)?.commit}
        <!-- IDEA CurrentBranchHighlighter / MergeCommitsHighlighter: current-branch background and dimmed merge text, never on selected rows. -->
        <div class="commit-row" class:current-branch={browser.rows.get(index)?.current && !selected.has(commit?.hash ?? '')}
          class:merge={(commit?.parents.length ?? 0) > 1 && !selected.has(commit?.hash ?? '')} style:grid-template-columns={`${graphWidth}px minmax(0,1fr) 100px 130px`}>
          <span></span><span class="subject">
            <span class="text"><Highlight text={commit?.subject ?? t('loading')} {query} /></span>
            {#if commit && refsByHash.get(commit.hash)?.length}{@const labels = refsByHash.get(commit.hash) ?? []}<span class="refs" title={labels.map(ref => ref.name).join(', ')}>{#each labels as ref, i}<svg class="tag" class:stacked={i > 0} style:color={`var(--label-${ref.kind})`} viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M2 3v4.6l6.4 6.4 5.6-5.6L7.6 2H3a1 1 0 0 0-1 1Z" fill="none" stroke="currentColor" /><circle cx="5" cy="5" r="1" fill="currentColor" /></svg>{/each}<span class="ref-name">{labels.map(ref => ref.name).join(', ')}</span></span>{/if}
          </span>
          <span class="secondary" title={commit?.email}>{commit?.author ?? ''}</span>
          <span class="secondary" title={commit ? new Date(commit.author_time * 1000).toLocaleString() : ''}>{commit ? formatDate(commit.author_time, navigator.language, new Date(), prettyDateWords()) : ''}</span>
        </div>
      {/snippet}
      {#snippet overlay(viewport)}<GraphCanvas {rows} {...viewport} width={Math.max(0, Math.min(graphWidth - viewport.scrollLeft, viewport.width))} head={browser.refs?.head.oid ?? null} />{/snippet}
    </List>
    {#if !browser.total}<div class="empty">{browser.loading ? t('loading') : t('noCommits')}</div>{/if}
  </div>
</section>
{#if context}<Popup items={contextItems} label={historyMenuText('menu')} anchor={context.anchor} onclose={() => { context = null; }} />{/if}
<style>
  section { height: 100%; display: flex; flex-direction: column; min-height: 0; }
  .list { position: relative; flex: 1; min-height: 0; }
  .commit-row { display: grid; align-items: center; gap: var(--space-inline); width: 100%; min-width: 0; }
  .commit-row { margin-inline-start: calc(-1 * var(--space-inline)); height: 100%; }
  .current-branch { background: var(--log-current-branch-bg); }
  .merge .text, .merge .secondary { color: var(--text-disabled); }
  .subject { display: flex; align-items: center; gap: calc(var(--space-inline) / 2); min-width: 0; overflow: hidden; }
  .text { flex: 1 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .refs { flex: 0 1 auto; display: inline-flex; align-items: center; min-width: calc(var(--font-ui-size) * 3); max-width: 40%; color: var(--text-secondary); }
  .tag { flex: none; } .tag.stacked { margin-inline-start: -8px; }
  .ref-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-inline-start: 2px; }
  .secondary { color: var(--text-secondary); font-size: var(--font-small-size); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .empty { position: absolute; inset: 25% 0 auto; text-align: center; color: var(--text-secondary); pointer-events: none; }
</style>
