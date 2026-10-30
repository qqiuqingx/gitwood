<script lang="ts">
  import { tick } from 'svelte';
  import Popup from '../../components/Popup.svelte';
  import type { PopupItem } from '../../components/popup';
  import { emptyFilter, fittingFilters } from './model';
  import type { FilterInput, References } from './types';
  import { browseText as t, type BrowseMessage } from '../../i18n/browse';
  let { value, refs, apply, refresh }: { value: FilterInput; refs: References | null; apply: (value: FilterInput) => void; refresh: () => void } = $props();
  let text = $state('');
  let dialog: HTMLDialogElement;
  let field = $state<'author' | 'date' | 'path' | null>(null);
  let draft = $state(emptyFilter());
  let popup = $state<{ anchor: HTMLElement; kind: 'branch' | 'more' } | null>(null);
  let lastValue = '';
  // Layout measurements for IDEA's overflow: filters that do not fit move behind `>`.
  let rowWidth = $state(0), searchWidth = $state(0), actionsWidth = $state(0), overflowWidth = $state(0);
  let widths = $state([0, 0, 0, 0]);
  $effect(() => { if (value.text !== lastValue) { text = value.text; lastValue = value.text; } });
  const short = (ref: string) => ref.replace(/^refs\/(heads|remotes|tags)\//, '');
  const branches = $derived<PopupItem[]>([
    { id: 'all', label: t('all'), run: () => apply({ ...value, branch: '' }) },
    ...(refs ? [...refs.local, ...Object.values(refs.remotes).flat(), ...refs.tags].map(ref => ({ id: ref.name, label: short(ref.name), run: () => apply({ ...value, branch: ref.name }) })) : []),
  ]);
  interface FilterField { id: 'branch' | 'author' | 'date' | 'path'; label: string; active: boolean; open: (anchor: HTMLElement) => void }
  const fields = $derived<FilterField[]>([
    { id: 'branch', label: value.branch ? short(value.branch) : t('branch'), active: !!value.branch, open: (anchor: HTMLElement) => { popup = { anchor, kind: 'branch' }; } },
    { id: 'author', label: value.author ? `${t('user')}: ${value.author}` : t('user'), active: !!value.author, open: () => void edit('author') },
    { id: 'date', label: t('date'), active: !!value.since || !!value.until, open: () => void edit('date') },
    { id: 'path', label: value.path ? `${t('paths')}: ${value.path}` : t('paths'), active: !!value.path, open: () => void edit('path') },
  ]);
  const gap = 2;
  const shown = $derived(rowWidth ? fittingFilters(rowWidth - searchWidth - actionsWidth - gap * 3, widths, overflowWidth, gap) : fields.length);
  const more = $derived<PopupItem[]>(fields.slice(shown).map(item => item.id === 'branch'
    ? { id: item.id, label: item.label, checked: item.active, children: branches }
    : { id: item.id, label: item.label, checked: item.active, run: () => item.open(document.body) }));
  function submitText(next: Partial<FilterInput> = {}) { apply({ ...value, text, ...next }); }
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
{#snippet chevron()}<svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="m4 6 4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>{/snippet}
<div class="filters" bind:clientWidth={rowWidth}>
  <form class="search" bind:offsetWidth={searchWidth} onsubmit={event => { event.preventDefault(); submitText(); }}>
    <svg class="glass" viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" /><path d="m10.5 10.5 3.5 3.5" stroke="currentColor" /></svg>
    <input aria-label={t('text')} placeholder={t('text')} bind:value={text} onchange={() => submitText()} />
    <button type="button" class="toggle" title={t('regex')} aria-label={t('regex')} aria-pressed={value.regex} onclick={() => submitText({ regex: !value.regex })}>.*</button>
    <button type="button" class="toggle" title={t('matchCase')} aria-label={t('matchCase')} aria-pressed={value.matchCase} onclick={() => submitText({ matchCase: !value.matchCase })}>Cc</button>
  </form>
  {#each fields.slice(0, shown) as item (item.id)}
    <button class="filter" class:active={item.active} title={item.label} onclick={event => item.open(event.currentTarget)}><span>{item.label}</span>{@render chevron()}</button>
  {/each}
  {#if shown < fields.length}
    <button class="filter overflow" title={t('moreFilters')} aria-label={t('moreFilters')} onclick={event => { popup = { anchor: event.currentTarget, kind: 'more' }; }}>
      <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="m6 4 4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>
    </button>
  {/if}
  <span class="actions" bind:offsetWidth={actionsWidth}>
    <button class="icon" title={t('refresh')} aria-label={t('refresh')} onclick={refresh}>
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path d="M13 8a5 5 0 1 1-1.5-3.55M13 2.5v3h-3" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>
    </button>
  </span>
  <!-- Off-screen copies give stable widths for the overflow calculation. -->
  <span class="measure" aria-hidden="true">
    {#each fields as item, index (item.id)}<span class="filter" bind:offsetWidth={widths[index]}><span>{item.label}</span>{@render chevron()}</span>{/each}
    <span class="filter overflow" bind:offsetWidth={overflowWidth}><svg viewBox="0 0 16 16" width="12" height="12"></svg></span>
  </span>
</div>
{#if popup}<Popup label={popup.kind === 'branch' ? t('branch') : t('moreFilters')} anchor={popup.anchor} items={popup.kind === 'branch' ? branches : more} onclose={() => { popup = null; }} />{/if}
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
  .filters { position: relative; display: flex; align-items: center; flex-wrap: nowrap; gap: 2px; min-width: 0; overflow: hidden; padding: var(--space-panel-y) var(--space-inline); border-bottom: var(--border-width) solid var(--separator); }
  .search { box-sizing: border-box; flex: none; display: flex; align-items: center; gap: 2px; width: clamp(calc(var(--font-ui-size) * 9), 45%, calc(var(--font-ui-size) * 16));
    padding-inline: calc(var(--space-inline) / 2); background: var(--input-bg); border: var(--border-width) solid var(--input-border); border-radius: calc(var(--space-inline) / 2); }
  .search:focus-within { border-color: var(--focus-border); }
  .glass { flex: none; color: var(--text-secondary); }
  .search input { flex: 1; min-width: 0; font: inherit; color: var(--text-primary); background: none; border: 0; outline: 0; padding: calc(var(--space-panel-y) / 2) 0; }
  .toggle { flex: none; font: inherit; font-size: var(--font-small-size); color: var(--text-secondary); background: none; border: 0; border-radius: calc(var(--space-inline) / 3); padding: 0 calc(var(--space-inline) / 3); }
  .toggle:hover { background: var(--toolbar-hover-bg); }
  .toggle[aria-pressed='true'] { color: var(--text-primary); background: var(--toolbar-active-bg, var(--toolbar-hover-bg)); }
  .filter { flex: none; display: inline-flex; align-items: center; gap: 2px; max-width: calc(var(--font-ui-size) * 14); font: inherit; color: var(--text-primary); background: none; border: 0;
    border-radius: calc(var(--space-inline) / 2); padding: calc(var(--space-panel-y) / 2) calc(var(--space-inline) / 2); white-space: nowrap; }
  .filter span { overflow: hidden; text-overflow: ellipsis; }
  .filter svg { flex: none; color: var(--text-secondary); }
  .filter:hover, .icon:hover { background: var(--toolbar-hover-bg); }
  .filter.active { color: var(--accent); }
  .actions { flex: none; display: inline-flex; }
  .icon { display: inline-flex; color: var(--text-primary); background: none; border: 0; border-radius: calc(var(--space-inline) / 2); padding: calc(var(--space-panel-y) / 2); }
  .measure { position: absolute; visibility: hidden; pointer-events: none; display: flex; gap: 2px; top: 0; left: 0; }
  dialog { color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  dialog form { display: flex; flex-direction: column; gap: var(--space-inline); }
  dialog input, dialog button { font: inherit; color: var(--text-primary); background: var(--input-bg); border: var(--border-width) solid var(--input-border); padding: calc(var(--space-panel-y) / 2); }
  h3 { margin: 0; font: inherit; font-weight: var(--font-weight-heading); }
  label, footer { display: flex; gap: var(--space-inline); align-items: center; }
  footer { justify-content: end; }
</style>
