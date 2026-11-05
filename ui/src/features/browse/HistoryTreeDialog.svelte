<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Tree from '../../components/Tree.svelte';
  import DiffView from '../diff/DiffView.svelte';
  import { browseText as b } from '../../i18n/browse';
  import type { RepositoryBrowser } from './browser.svelte';
  import type { FileRow } from './PagedFileTree.svelte';
  import type { RepoPath } from './types';
  import type { Content, DiffResult } from '../diff/types';
  let { browser, revision, onclose }: { browser: RepositoryBrowser; revision: string; onclose: () => void } = $props();
  interface Summary { id: string; revision: string; files: number }
  interface Preview { left: Content; right: Content; result: DiffResult | null; leftLabel: string; rightLabel: string }
  let element: HTMLDialogElement;
  let summary = $state<Summary | null>(null), rows = $state.raw(new Map<number, FileRow>());
  let total = $state(0), expanded = $state<ReadonlySet<string>>(new Set(['files']));
  let selected = $state<RepoPath | null>(null), preview = $state<Preview | null>(null), error = $state('');
  let range = { start: 0, end: 40 }, request = 0, selectionRequest = 0;
  const rowAt = (index: number): FileRow => rows.get(index) ?? { id: `loading:${index}`, label: b('loading'), hasChildren: false, parentIndex: -1, level: 1, path: null, status: null, note: null, staged: false, unstaged: false, conflicted: false, count: 0 };
  onMount(() => { element.showModal(); const generation = browser.generation; void invoke<Summary>('history_tree_open', { generation, revision }).then(value => { if (generation === browser.generation) summary = value; }).catch(fail); return () => { request++; selectionRequest++; }; });
  $effect(() => { summary; expanded; if (summary) void load({ start: 0, end: 40 }, true); });
  async function load(next = range, reset = false) {
    if (!summary) return;
    range = next; const ticket = ++request, generation = browser.generation;
    try {
      const page = await invoke<{ total: number; start: number; rows: FileRow[] }>('history_tree_files', { generation, id: summary.id, expanded: [...expanded], start: next.start, end: Math.min(next.end, next.start + 200) });
      if (ticket !== request || generation !== browser.generation) return;
      total = page.total; const nextRows = new Map(reset ? [] : rows); page.rows.forEach((row, index) => nextRows.set(page.start + index, row));
      rows = new Map([...nextRows].filter(([index]) => index >= next.start - 200 && index < next.end + 200));
    } catch (cause) { fail(cause); }
  }
  async function choose(path: RepoPath) {
    selected = path; preview = null; const ticket = ++selectionRequest;
    try { const value = await invoke<Preview>('history_tree_diff', { generation: browser.generation, id: summary!.id, path }); if (ticket === selectionRequest) preview = value; }
    catch (cause) { fail(cause); }
  }
  function fail(cause: unknown) { error = String((cause as { detail?: string }).detail ?? cause); }
</script>
<dialog bind:this={element} aria-labelledby="revision-title" oncancel={event => { event.preventDefault(); onclose(); }}>
  <header><h2 id="revision-title">{b('repositoryAtRevision')} {revision.slice(0, 8)}</h2><button onclick={onclose}>{b('close')}</button></header>
  {#if error}<p role="alert">{error}</p>{/if}
  {#if !summary}<p role="status">{b('loading')}</p>{:else}
    <p>{summary.files} {b('files')}</p>
    <div class="content"><div class="tree"><Tree label={b('files')} visibleRows={{ count: total, getRow: rowAt }} {expanded} onexpandedchange={value => expanded = value} onrangechange={value => { if (value.start !== range.start || value.end !== range.end) void load(value); }} onselectionchange={(_, row) => { const item = row as FileRow; if (item.path) void choose(item.path); }} onactivate={row => { const item = row as FileRow; if (item.path) void choose(item.path); }} /></div>
      <div class="diff">{#if preview}<DiffView fileKey={JSON.stringify(selected?.bytes)} left={preview.left} right={preview.right} result={preview.result ?? { blocks: [], alignment: [], folds: [], leftLines: 0, rightLines: 0 }} />{:else if selected}<p role="status">{b('loading')}</p>{/if}</div>
    </div>
  {/if}
</dialog>
<style>
  dialog { width: min(1100px, 92vw); height: 78vh; color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  header { display: flex; align-items: center; gap: var(--space-inline); } h2 { flex: 1; } button { font: inherit; }
  .content { display: flex; height: calc(100% - 90px); min-height: 0; border: var(--border-width) solid var(--separator); }
  .tree { flex: 0 0 30%; min-width: 0; overflow: hidden; } .diff { flex: 1; min-width: 0; min-height: 0; overflow: hidden; }
</style>
