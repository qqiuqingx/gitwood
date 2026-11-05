<script lang="ts">
  import List from '../../components/List.svelte';
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import { conflictText as t, type ConflictMessage } from '../../i18n/conflict';
  import { notifications } from '../notifications/center.svelte';
  import type { ConflictFile, MergeContent } from './model';
  import MergeEditor from './MergeEditor.svelte';
  let { browser, oncontinue }: { browser: RepositoryBrowser; oncontinue: () => void } = $props();
  let opened = $state(false), busy = $state(false), error = $state('');
  let selected = $state<ConflictFile | null>(null), content = $state<MergeContent | null>(null);
  let dialog = $state<HTMLDialogElement>();
  let editor = $state<MergeEditor>();
  const files = $derived(browser.status?.conflict_files ?? []);
  const activeFile = $derived(files.find(f => f.path.bytes.join(',') === selected?.path.bytes.join(',')) ?? files[0] ?? null);
  const rebase = $derived(browser.status?.operation?.kind === 'rebase');
  export function open() { opened = true; }
  let previous = '';
  $effect(() => {
    const status = browser.status;
    const signature = status?.conflicts ? `${status.branch.oid}:${status.operation?.step}:${status.conflict_files?.map(f => f.path.bytes.join(',')).join(';')}` : '';
    if (signature && signature !== previous) untrack(() => {
      opened = true;
      notifications.notify({ kind: 'warning', title: t('title'), group: 'conflicts', actions: [{ label: t('resolve'), run: open }] });
    });
    previous = signature;
  });
  $effect(() => { if (opened && dialog && !dialog.open) dialog.showModal(); });
  onMount(() => { window.addEventListener('resolve-conflicts', open); return () => window.removeEventListener('resolve-conflicts', open); });
  function close() { if (busy) return; if (content && !content.readOnly) { editor?.requestClose(); return; } content = null; opened = false; }
  async function load(file: ConflictFile) {
    if (busy || content) return;
    busy = true; error = ''; selected = file;
    try { content = await invoke<MergeContent>('conflict_content', { generation: browser.generation, path: file.path }); }
    catch (cause) { error = (cause as { detail?: string }).detail ?? String(cause); }
    finally { busy = false; }
  }
  async function resolve(file: ConflictFile, side: 'yours' | 'theirs' | null, text: string | null = null) {
    if (busy) return;
    busy = true; error = '';
    try {
      await invoke('resolve_conflict', { generation: browser.generation, path: file.path, expected: side ? browser.status?.fingerprint : content?.fingerprint, side, text });
      content = null; selected = null;
      await browser.refresh({ status: true, refs: true, history: false });
    } catch (cause) { error = (cause as { detail?: string }).detail ?? String(cause); }
    finally { busy = false; }
  }
</script>
{#if opened}
  <dialog bind:this={dialog} class:wide={!!content} aria-label={t('title')} oncancel={event => { event.preventDefault(); close(); }}>
    <h2>{t('title')}{selected && content ? ` — ${selected.path.display}` : ''}</h2>
    {#if content && selected}
      {#if content.readOnly}<p>{t(content.readOnly)}</p><button onclick={() => { content = null; selected = null; }}>{t('close')}</button>
      {:else}<MergeEditor bind:this={editor} {content} {rebase} {busy} onsave={text => void resolve(selected!, null, text)} onclose={() => { content = null; selected = null; }} />{/if}
    {:else}
      {#if rebase}<p>{t('rebaseHint')}</p>{/if}
      <div class="files"><List rowCount={files.length} label={t('title')}
        getRow={index => ({ id: files[index].path.bytes.join(','), label: files[index].path.display })}
        onselectionchange={(_, index) => { selected = files[index] ?? null; }} onactivate={(_, index) => void load(files[index])}>
        {#snippet row(_, index)}{@const file = files[index]}<span class="path">{file.path.display}</span>
          <span>{t(file.conflictType as ConflictMessage)}{(file.binary || file.large) ? ` · ${t('binary')}` : ''}{file.submodule ? ` · ${t('submodule')}` : ''}</span>
        {/snippet}
      </List></div>
      {#if activeFile}<div class="actions">
        <button disabled={busy} onclick={() => void resolve(activeFile!, 'yours')}>{t('yours')}</button>
        <button disabled={busy} onclick={() => void resolve(activeFile!, 'theirs')}>{t('theirs')}</button>
        <button disabled={busy} onclick={() => void load(activeFile!)}>{t('merge')}</button>
      </div>{/if}
      {#if !files.length}<p role="status">{t('allResolved')}</p>{/if}
      <footer><button disabled={busy} onclick={close}>{t('close')}</button>{#if !files.length && (browser.status?.operation || browser.status?.update_stash)}<button disabled={busy} onclick={() => { opened = false; oncontinue(); }}>{t('continue')}</button>{/if}</footer>
    {/if}
    {#if busy}<p role="status">{t('loading')}</p>{/if}
    {#if error}<div role="alert"><strong>{t('error')}</strong><pre>{error}</pre></div>{/if}
  </dialog>
{/if}
<style>
  dialog { width: min(1000px, 92vw); max-height: 92vh; overflow: auto; background: var(--popup-bg); color: var(--text-primary); border: 1px solid var(--popup-border); }
  dialog.wide { width: 94vw; max-width: 1600px; } dialog::backdrop { background: #0008; }
  h2 { font-size: calc(var(--font-ui-size) * 1.15); } button { font: inherit; } .files { height: min(400px, 50vh); min-height: 100px; }
  .actions { display: flex; align-items: center; gap: var(--space-inline); padding-block: var(--space-inline); border-bottom: 1px solid var(--separator); }
  .path { flex: 1; text-align: start; overflow: hidden; text-overflow: ellipsis; background: none; color: var(--text-primary); border: 0; }
  .files span { color: var(--text-secondary); } footer { display: flex; justify-content: end; gap: var(--space-inline); margin-top: var(--space-inline); }
  pre { white-space: pre-wrap; overflow-wrap: anywhere; color: var(--status-conflicted); }
</style>
