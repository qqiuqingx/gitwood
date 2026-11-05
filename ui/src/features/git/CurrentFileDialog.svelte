<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import List from '../../components/List.svelte';
  import DiffView from '../diff/DiffView.svelte';
  import AnnotatedCode from './AnnotatedCode.svelte';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import type { RepoPath } from '../browse/types';
  import type { Commit } from '../browse/types';
  import type { Content, DiffResult } from '../diff/types';
  import { currentFileText as t } from '../../i18n/current-file';
  import { untrack } from 'svelte';

  type Action = 'history' | 'annotate' | 'annotatePrevious' | 'compareRevision' | 'compareBranch';
  interface FileRevision { hash: string; parents: string[]; authorTime: number; author: string; subject: string; path: RepoPath }
  interface Summary { id: string; total: number; revision?: string; oldest?: number; newest?: number }
  interface AnnotationLine { hash: string; previous: string | null; previousPath: RepoPath | null }
  interface Preview { left: Content; right: Content; result: DiffResult | null; leftLabel: string; rightLabel: string; conflicted?: boolean }
  let { browser, action, path, revision = 'HEAD', onclose, oncommit, onprevious }: { browser: RepositoryBrowser; action: Action; path: RepoPath; revision?: string; onclose: () => void; oncommit: (hash: string) => void; onprevious: (path: RepoPath, revision: string) => void } = $props();
  let element: HTMLDialogElement;
  let mode = $state(untrack(() => action)), summary = $state<Summary | null>(null), rows = $state.raw(new Map<number, FileRevision>());
  let preview = $state<Preview | null>(null), selectedTarget = $state(''), selectedLine = $state<AnnotationLine | null>(null);
  let chosenPath = $state(untrack(() => path)), error = $state(''), busy = $state(false);
  let range = { start: 0, end: 40 }, request = 0;
  let openRequest = 0, annotationGeneration = untrack(() => browser.generation);
  let openedAction = '';
  const commits = $derived([...browser.rows.values()].map(row => row.commit));
  const branches = $derived(browser.refs ? [...browser.refs.local, ...Object.values(browser.refs.remotes).flat()] : []);
  const refs = $derived(browser.refs ? [...branches, ...browser.refs.tags] : []);
  const targets = $derived(mode === 'compareBranch'
    ? branches.map(ref => ({ id: ref.name, label: ref.name.replace(/^refs\/(heads|remotes)\//, '') }))
    : [...new Map([
      ...commits.map(commit => [commit.hash, { id: commit.hash, label: `${commit.hash.slice(0, 8)} ${commit.subject}` }] as const),
      ...refs.map(ref => [ref.hash, { id: ref.hash, label: `${ref.name.replace(/^refs\/(heads|remotes|tags)\//, '')} (${ref.hash.slice(0, 8)})` }] as const),
    ]).values()]);
  const rowAt = (index: number) => { const row = rows.get(index); return { id: row?.hash ?? `loading:${index}`, label: row ? `${row.hash.slice(0, 8)} ${row.subject}` : t('loading') }; };
  $effect(() => { if (mode.startsWith('compare') && !selectedTarget && targets.length) selectedTarget = targets[0].id; });
  onMount(() => { element.showModal(); });
  $effect(() => {
    const nextAction = action, nextPath = path, nextRevision = revision;
    const identity = `${nextAction}:${nextPath.display}:${nextRevision}`;
    if (identity === openedAction) return;
    openedAction = identity; mode = nextAction; selectedTarget = '';
    if (nextAction === 'history') void openHistory();
    if (nextAction === 'annotate' || nextAction === 'annotatePrevious') void openAnnotation(nextPath, nextRevision);
  });
  onDestroy(() => { request++; openRequest++; if (mode === 'annotate') void invoke('annotate_cancel', { generation: annotationGeneration }).catch(() => {}); });
  async function openHistory() {
    const ticket = ++openRequest, generation = browser.generation;
    busy = true; error = '';
    try { const value = await invoke<Summary>('file_history_open', { generation, path, revision }); if (ticket === openRequest && generation === browser.generation) summary = value; }
    catch (cause) { if (ticket === openRequest) fail(cause); } finally { if (ticket === openRequest) busy = false; }
  }
  async function load(next = range, reset = false) {
    if (!summary) return;
    range = next; const ticket = ++request, generation = browser.generation;
    try {
      const page = await invoke<{ total: number; start: number; rows: FileRevision[] }>('file_history_page', { generation, id: summary.id, start: next.start, end: Math.min(next.end, next.start + 200) });
      if (ticket !== request || generation !== browser.generation) return;
      const values = new Map(reset ? [] : rows); page.rows.forEach((row, index) => values.set(page.start + index, row));
      rows = new Map([...values].filter(([index]) => index >= next.start - 200 && index < next.end + 200));
    } catch (cause) { fail(cause); }
  }
  async function openAnnotation(targetPath: RepoPath, targetRevision: string) {
    const ticket = ++openRequest, generation = browser.generation;
    annotationGeneration = generation;
    mode = 'annotate'; busy = true; error = ''; summary = null; preview = null; chosenPath = targetPath; selectedLine = null;
    try { const value = await invoke<Summary>('annotate_open', { generation, path: targetPath, revision: targetRevision }); if (ticket === openRequest && generation === browser.generation) summary = value; }
    catch (cause) { if (ticket === openRequest) fail(cause); } finally { if (ticket === openRequest) busy = false; }
  }
  async function compare() {
    if (!selectedTarget) return;
    busy = true; error = ''; preview = null;
    try { preview = await invoke<Preview>('compare_file_with_revision', { generation: browser.generation, path, revision: selectedTarget }); }
    catch (cause) { fail(cause); } finally { busy = false; }
  }
  function fail(cause: unknown) { error = String((cause as { detail?: string }).detail ?? cause); }
  function close() { onclose(); }
  function chooseHistory(index: number) {
    const row = rows.get(index); if (!row) return;
    close(); oncommit(row.hash);
  }
  function date(value: number) { return new Date(value * 1000).toLocaleString(); }
</script>
<dialog bind:this={element} aria-labelledby="current-file-title" oncancel={event => { event.preventDefault(); close(); }}>
  <header><h2 id="current-file-title">{t(mode === 'history' ? 'history' : mode === 'annotate' ? 'annotate' : mode === 'compareBranch' ? 'compareBranch' : 'compareRevision')} · {mode === 'annotate' ? chosenPath.display : path.display}</h2><button onclick={close}>{t('close')}</button></header>
  {#if error}<p role="alert">{error}</p>{/if}
  {#if mode === 'history'}
    {#if !summary}<p role="status">{busy ? t('loading') : t('noHistory')}</p>{:else}<List label={t('commits')} rowCount={summary.total} getRow={rowAt} onrangechange={next => { if (next.start !== range.start || next.end !== range.end) void load(next); }} onactivate={(_, index) => chooseHistory(index)}>
      {#snippet row(_, index)}{@const row = rows.get(index)}<span class="history-row"><span class="hash">{row?.hash.slice(0, 8) ?? ''}</span><span class="subject">{row?.subject ?? t('loading')}</span><span class="path" title={row?.path.display}>{row?.path.display ?? ''}</span><span class="author">{row?.author ?? ''}</span><span class="date">{row ? date(row.authorTime) : ''}</span></span>{/snippet}
    </List>{/if}
  {:else if mode === 'annotate'}
    {#if summary}<div class="annotation"><AnnotatedCode {browser} id={summary.id} total={summary.total} oldest={summary.oldest ?? 0} newest={summary.newest ?? 0} oncommit={hash => { oncommit(hash); }} onprevious={line => selectedLine = line} onerror={fail} /></div>
      {#if selectedLine?.previous && selectedLine.previousPath}<button onclick={() => onprevious(selectedLine!.previousPath!, selectedLine!.previous!)}>{t('previous')} · {selectedLine.previous.slice(0, 8)} ({selectedLine.previousPath.display})</button>{/if}
    {:else if busy}<p role="status">{t('loading')}</p>{/if}
  {:else}
    <label>{t('revision')}<select bind:value={selectedTarget}>{#each targets as target}<option value={target.id}>{target.label}</option>{/each}</select></label>
    <button disabled={!selectedTarget || busy} onclick={() => void compare()}>{t('choose')}</button>
    {#if preview}<p>{preview.leftLabel} ↔ {preview.rightLabel}</p><div class="diff"><DiffView fileKey={JSON.stringify(path.bytes)} left={preview.left} right={preview.right} result={preview.result ?? { blocks: [], alignment: [], folds: [], leftLines: 0, rightLines: 0 }} /></div>{:else if busy}<p role="status">{t('loading')}</p>{/if}
  {/if}
</dialog>
<style>
  dialog { width: min(1200px, 94vw); height: 80vh; color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  header { display:flex; align-items:center; gap:var(--space-inline); } h2 { flex:1; font-size:calc(var(--font-ui-size) * 1.1); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  header button, dialog > button, label select { font:inherit; padding:var(--space-panel-y) var(--space-inline); } dialog > :global(.list) { height:calc(100% - 70px); }
  .history-row { display:grid; grid-template-columns:80px minmax(100px,1fr) minmax(100px,1.2fr) 130px 170px; gap:var(--space-inline); width:100%; } .subject,.path,.author,.date { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; } .hash { color:var(--text-secondary); }
  .annotation,.diff { height:calc(100% - 90px); min-height:0; } label { display:flex; gap:var(--space-inline); align-items:center; margin-block:var(--space-panel-y); } label select { flex:1; min-width:0; }
  [role='alert'] { white-space:pre-wrap; color:var(--status-conflicted); }
</style>
