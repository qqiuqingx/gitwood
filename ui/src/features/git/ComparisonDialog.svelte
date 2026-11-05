<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Tree from '../../components/Tree.svelte';
  import List from '../../components/List.svelte';
  import DiffView from '../diff/DiffView.svelte';
  import { integrationText as t } from '../../i18n/integration';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import type { FileRow } from '../browse/PagedFileTree.svelte';
  import type { Commit, RepoPath } from '../browse/types';
  import type { Content, DiffResult } from '../diff/types';
  let { browser, left, right = null, working, onclose }: { browser: RepositoryBrowser; left: string; right?: string | null; working: boolean; onclose: () => void } = $props();
  interface Summary { id: string; left: string; right: string | null; commits: number }
  interface Preview { left: Content; right: Content; result: DiffResult | null; leftLabel: string; rightLabel: string }
  let element: HTMLDialogElement;
  let summary = $state<Summary | null>(null), preview = $state<Preview | null>(null);
  let error = $state(''), tab = $state<'files' | 'commits'>(untrack(() => working ? 'files' : 'commits'));
  let expanded = $state<ReadonlySet<string>>(new Set(['files']));
  let rows = $state.raw(new Map<number, FileRow>()), commits = $state.raw(new Map<number, Commit & { current: boolean }>());
  let total = $state(0), chosen = $state<RepoPath | null>(null);
  let range = { start: 0, end: 40 }, ticket = 0, selectionTicket = 0;
  const getRow = (index: number): FileRow => rows.get(index) ?? { id: `loading:${index}`, label: t('loading'), hasChildren: false, parentIndex: -1, level: 1, path: null, status: null, note: null, staged: false, unstaged: false, conflicted: false, count: 0 };
  onMount(() => { element.showModal(); return () => { ticket++; selectionTicket++; }; });
  $effect(() => {
    const generation = browser.generation;
    let alive = true;
    summary = null; preview = null; selectionTicket++; chosen = null; rows = new Map(); commits = new Map(); total = 0; error = ''; ticket++;
    void invoke<Summary>('comparison_open', { generation, left, right, working }).then(value => { if (alive) summary = value; }).catch(cause => { if (alive) error = String((cause as { detail?: string }).detail ?? cause); });
    return () => { alive = false; };
  });
  $effect(() => { summary; expanded; tab; untrack(() => { if (summary) void load({ start: 0, end: 40 }, true); }); });
  async function load(next = range, reset = false) {
    if (!summary || next.end <= next.start) return;
    range = next;
    const request = ++ticket, generation = browser.generation, id = summary.id;
    try {
      if (tab === 'files') {
        const page = await invoke<{ total: number; start: number; rows: FileRow[] }>('comparison_files', { generation, id, expanded: [...expanded], start: next.start, end: Math.min(next.end, next.start + 200) });
        if (request !== ticket) return;
        total = page.total;
        const values = new Map(reset ? [] : rows); page.rows.forEach((row, i) => values.set(page.start + i, { ...row, parentIndex: row.parentIndex ?? -1 }));
        rows = new Map([...values].filter(([i]) => i >= next.start - 200 && i < next.end + 200));
      } else {
        const page = await invoke<{ commits: Commit[]; current: boolean[] }>('comparison_commits', { generation, id, start: Math.min(next.start, summary.commits), end: Math.min(next.end, next.start + 200, summary.commits) });
        if (request !== ticket) return;
        const values = new Map(reset ? [] : commits); page.commits.forEach((row, i) => values.set(next.start + i, { ...row, current: page.current[i] }));
        commits = new Map([...values].filter(([i]) => i >= next.start - 200 && i < next.end + 200));
      }
    } catch (cause) { if (request === ticket) error = String((cause as { detail?: string }).detail ?? cause); }
  }
  async function select(path: RepoPath) {
    if (!summary) return;
    const request = ++selectionTicket; chosen = path; preview = null;
    try { const value = await invoke<Preview>('comparison_diff', { generation: browser.generation, id: summary.id, path }); if (request === selectionTicket) preview = value; }
    catch (cause) { if (request === selectionTicket) error = String((cause as { detail?: string }).detail ?? cause); }
  }
</script>
<dialog bind:this={element} aria-labelledby="comparison-title" oncancel={event => { event.preventDefault(); onclose(); }}>
  <header><h2 id="comparison-title">{t('comparison')}: {left.replace(/^refs\/(heads|remotes)\//, '')}</h2><button onclick={onclose}>{t('close')}</button></header>
  <p>{summary?.left.slice(0, 8) ?? ''} ↔ {working ? t('workingTree') : summary?.right?.slice(0, 8) ?? ''}</p>
  {#if error}<p role="alert">{error}</p>{/if}
  {#if !summary}<p role="status">{t('loading')}</p>{:else}
    <nav aria-label={t('comparison')}><button aria-pressed={tab === 'files'} onclick={() => { tab = 'files'; }}>{t('files')}</button>{#if !working}<button aria-pressed={tab === 'commits'} onclick={() => { tab = 'commits'; }}>{t('commits')} ({summary.commits})</button>{/if}</nav>
    <div class="content">
      {#if tab === 'files'}
        <div class="files"><Tree label={t('files')} visibleRows={{ count: total, getRow }} {expanded} onexpandedchange={ids => { expanded = ids; }} onrangechange={next => { if (next.start !== range.start || next.end !== range.end) void load(next); }} onselectionchange={(_, row) => { if ((row as FileRow)?.path) void select((row as FileRow).path!); }} onactivate={row => { if ((row as FileRow).path) void select((row as FileRow).path!); }} />{#if !total}<p>{t('empty')}</p>{/if}</div>
        <div class="diff">{#if chosen}<p>{chosen.display}</p>{/if}{#if preview}<div class="viewer"><DiffView fileKey={JSON.stringify(chosen?.bytes)} left={preview.left} right={preview.right} result={preview.result ?? { blocks: [], alignment: [], folds: [], leftLines: 0, rightLines: 0 }} /></div>{:else if chosen}<p>{t('loading')}</p>{/if}</div>
      {:else}
        <div class="commits"><List rowCount={summary.commits} label={t('commits')} getRow={index => ({ id: commits.get(index)?.hash ?? `loading:${index}`, label: commits.has(index) ? `${t(commits.get(index)!.current ? 'current' : 'selected')}: ${commits.get(index)!.hash.slice(0, 8)} ${commits.get(index)!.subject}` : t('loading') })} onrangechange={next => { if (next.start !== range.start || next.end !== range.end) void load(next); }} />{#if !summary.commits}<p>{t('empty')}</p>{/if}</div>
      {/if}
    </div>
  {/if}
</dialog>
<style>
  dialog { width: min(1200px, 94vw); height: 80vh; color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  header,nav { display: flex; gap: var(--space-inline); align-items: center; } h2 { font-size: calc(var(--font-ui-size) * 1.15); flex: 1; } button { font: inherit; padding: var(--space-panel-y) var(--space-inline); } .content { height: calc(100% - 130px); min-height: 0; display: flex; margin-top: var(--space-panel-y); } .files { flex: 0 0 28%; min-width: 0; } .diff { display: flex; flex-direction: column; flex: 1; min-width: 0; min-height: 0; } .viewer { flex: 1; min-height: 0; } .commits { flex: 1; } [role='alert'] { color: var(--status-conflicted); white-space: pre-wrap; }
</style>
