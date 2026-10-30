<script lang="ts">
  import { tick } from 'svelte';
  import Popup from '../../components/Popup.svelte';
  import type { PopupItem } from '../../components/popup';
  import { emptyFilter } from './model';
  import type { FilterInput, References } from './types';
  import { browseText as t, type BrowseMessage } from '../../i18n/browse';
  let { value, refs, apply, refresh }: { value: FilterInput; refs: References | null; apply: (value: FilterInput) => void; refresh: () => void } = $props();
  let text = $state('');
  let dialog: HTMLDialogElement;
  let field = $state<'author' | 'date' | 'path' | null>(null);
  let draft = $state(emptyFilter());
  let branchAnchor = $state<HTMLElement | null>(null);
  let lastValue = '';
  $effect(() => { if (value.text !== lastValue) { text = value.text; lastValue = value.text; } });
  const branches = $derived<PopupItem[]>([
    { id: 'all', label: t('all'), run: () => apply({ ...value, branch: '' }) },
    ...(refs ? [...refs.local, ...Object.values(refs.remotes).flat(), ...refs.tags].map(ref => ({ id: ref.name, label: ref.name.replace(/^refs\/(heads|remotes|tags)\//, ''), run: () => apply({ ...value, branch: ref.name }) })) : []),
  ]);
  async function edit(next: 'author' | 'date' | 'path') {
    draft = { ...value }; field = next; dialog.showModal(); await tick(); dialog.querySelector('input')?.focus();
  }
  function save() { apply(draft); dialog.close(); field = null; }
  function clear() {
    const next = { ...value };
    if (field === 'date') { next.since = ''; next.until = ''; }
    else if (field) next[field] = '';
    apply(next); dialog.close(); field = null;
  }
</script>
<div class="filters">
  <form onsubmit={event => { event.preventDefault(); apply({ ...value, text }); }}><input aria-label={t('text')} placeholder={t('text')} bind:value={text}
    onchange={() => apply({ ...value, text })} /></form>
  <button class:active={!!value.branch} onclick={event => { branchAnchor = event.currentTarget; }}>{value.branch ? value.branch.replace(/^refs\/(heads|remotes|tags)\//, '') : t('branch')} ⌄</button>
  <button class:active={!!value.author} onclick={() => edit('author')}>{t('user')}{value.author ? `: ${value.author}` : ''} ⌄</button>
  <button class:active={!!value.since || !!value.until} onclick={() => edit('date')}>{t('date')} ⌄</button>
  <button class:active={!!value.path} onclick={() => edit('path')}>{t('paths')}{value.path ? `: ${value.path}` : ''} ⌄</button>
  <button class="refresh" title={t('refresh')} aria-label={t('refresh')} onclick={refresh}>⟳</button>
</div>
{#if branchAnchor}<Popup label={t('branch')} anchor={branchAnchor} items={branches} onclose={() => { branchAnchor = null; }} />{/if}
<dialog bind:this={dialog} onclose={() => { field = null; }}>
  <form onsubmit={event => { event.preventDefault(); save(); }}>
    <h3>{t((field === 'author' ? 'user' : field === 'path' ? 'paths' : 'date') as BrowseMessage)}</h3>
    {#if field === 'date'}
      <label>{t('since')}<input type="date" bind:value={draft.since} /></label>
      <label>{t('until')}<input type="date" bind:value={draft.until} /></label>
    {:else if field}<input aria-label={t(field === 'author' ? 'user' : 'paths')} bind:value={draft[field]} />{/if}
    <footer><button type="button" onclick={clear}>{t('clear')}</button><button type="button" onclick={() => dialog.close()}>{t('close')}</button><button type="submit">{t('apply')}</button></footer>
  </form>
</dialog>
<style>
  .filters { display: flex; align-items: center; flex-wrap: wrap; gap: var(--space-inline); padding: var(--space-panel-y) var(--space-inline); border-bottom: var(--border-width) solid var(--separator); }
  .filters form { min-width: 0; flex: 1; }
  .filters input { width: 100%; min-width: calc(var(--font-ui-size) * 8); }
  button, input { font: inherit; color: var(--text-primary); background: var(--input-bg); border: var(--border-width) solid var(--input-border); padding: calc(var(--space-panel-y) / 2); }
  .filters button { border-color: var(--separator); background: none; white-space: nowrap; max-width: calc(var(--font-ui-size) * 16); overflow: hidden; text-overflow: ellipsis; }
  button.active { color: var(--accent); }
  .refresh { margin-inline-start: auto; }
  dialog { color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  dialog form { display: flex; flex-direction: column; gap: var(--space-inline); }
  h3 { margin: 0; font: inherit; font-weight: var(--font-weight-heading); }
  label, footer { display: flex; gap: var(--space-inline); align-items: center; }
  footer { justify-content: end; }
</style>
