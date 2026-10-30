<!-- New theme variables: --list-row-height, --list-selection-bg, --list-selection-text,
     --list-selection-inactive-bg, --list-selection-inactive-text, --list-disabled-opacity.
     Row height is measured from CSS; theme changes retain selection and scroll. -->
<script lang="ts">
  import { onMount, untrack, type Snippet } from 'svelte';
  import { controlText } from '../i18n/controls';
  import { navigationIndex, selectRow, selectable, visibleRange, type ListRow } from './list';
  import { findSearchMatch } from './speed-search';
  import SpeedSearch from './SpeedSearch.svelte';
  import Highlight from './Highlight.svelte';
  import { createActionRegistry } from '../keymap/registry';
  import { detectPlatform } from '../keymap/shortcuts';
  import { keymapFor } from '../keymap/keymaps';

  interface Props {
    scrollTop?: number;
    minRowWidth?: number;
    overlay?: Snippet<[{ start: number; end: number; scrollTop: number; scrollLeft: number; width: number; rowHeight: number; height: number }]>;
    onviewportchange?: (viewport: { start: number; end: number; scrollTop: number; scrollLeft: number; width: number; rowHeight: number; height: number }) => void;
    rowCount: number;
    rowScales?: readonly number[];
    getRow: (index: number) => ListRow;
    label: string;
    multi?: boolean;
    selectedIds?: ReadonlySet<string>;
    activeIndex?: number;
    role?: 'listbox' | 'tree';
    search?: boolean;
    searchRows?: (query: string, active: number, direction: 1 | -1, includeCurrent: boolean) => Promise<number>;
    searchLabel?: string;
    noMatchLabel?: string;
    activateOnClick?: boolean;
    row?: Snippet<[ListRow, number, string]>;
    onselectionchange?: (ids: ReadonlySet<string>, index: number) => void;
    onactivate?: (row: ListRow, index: number, reason: 'enter' | 'doubleclick' | 'click') => void;
    onrangechange?: (range: { start: number; end: number }) => void;
    onquerychange?: (query: string) => void;
    onnavigate?: (event: KeyboardEvent, index: number) => boolean;
    oncontextmenu?: (event: MouseEvent, row: ListRow, index: number) => void;
  }
  let {
    rowCount, rowScales, getRow, label, multi = false, selectedIds = $bindable(new Set<string>() as ReadonlySet<string>),
    activeIndex = $bindable(-1), role = 'listbox', search = true, searchLabel = controlText('search'), noMatchLabel = controlText('noMatch'),
    scrollTop = $bindable(0), minRowWidth = 0, overlay, onviewportchange,
    searchRows, activateOnClick = false, row, onselectionchange, onactivate, onrangechange, onquerychange, onnavigate, oncontextmenu,
  }: Props = $props();
  const id = $props.id();
  let host: HTMLDivElement;
  let viewport: HTMLDivElement;
  let probe: HTMLDivElement;
  let rowHeight = $state(0);
  let height = $state(0);
  let width = $state(0);
  let scrollLeft = $state(0);

  let focused = $state(false);
  let anchor = $state(-1);
  let query = $state('');
  let noMatch = $state(false);
  let searchRequest = 0;
  const active = $derived(activeIndex >= 0 && activeIndex < rowCount ? activeIndex : -1);
  // Popup descriptions use two-line rows; ordinary long lists keep constant-time geometry.
  const offsets = $derived.by(() => {
    if (!rowScales) return undefined;
    const result = [0];
    for (let index = 0; index < rowCount; index++) result.push(result[index] + rowHeight * (rowScales[index] ?? 1));
    return result;
  });
  const range = $derived(visibleRange(rowCount, rowHeight, scrollTop, height, 3, offsets));
  const rows = $derived.by(() => {
    const result: { index: number; item: ListRow }[] = [];
    for (let index = range.start; index < range.end; index++) result.push({ index, item: getRow(index) });
    return result;
  });
  let bindings = keymapFor('linux');
  const actions = createActionRegistry();
  actions.register('List-selectFirstRow', { run: () => navigate('Home') });
  actions.register('List-selectLastRow', { run: () => navigate('End') });
  actions.register('List-selectFirstRowExtendSelection', { run: () => navigate('Home', true) });
  actions.register('List-selectLastRowExtendSelection', { run: () => navigate('End', true) });

  onMount(() => {
    bindings = keymapFor(detectPlatform(navigator.platform));
    const measure = () => { rowHeight = probe.getBoundingClientRect().height; height = viewport.clientHeight; width = viewport.clientWidth; };
    const observer = new ResizeObserver(measure);
    observer.observe(viewport); observer.observe(probe); measure();
    return () => observer.disconnect();
  });
  $effect(() => { const current = range; untrack(() => onrangechange?.(current)); });
  $effect(() => { onquerychange?.(query); });
  $effect(() => { const current = { ...range, scrollTop, scrollLeft, width, rowHeight, height }; untrack(() => onviewportchange?.(current)); });
  $effect(() => {
    if (viewport && Math.abs(viewport.scrollTop - scrollTop) > 0.5) viewport.scrollTop = scrollTop;
  });
  function reveal(index: number) {
    if (!viewport || !rowHeight || index < 0) return;
    const top = offsets?.[index] ?? index * rowHeight;
    const bottom = offsets?.[index + 1] ?? top + rowHeight;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (bottom > viewport.scrollTop + height) viewport.scrollTop = bottom - height;
    scrollTop = viewport.scrollTop;
  }

  export function focus() { viewport?.focus(); }
  export function selectIndex(index: number, extend = false, toggle = false) {
    const state = selectRow({ ids: selectedIds, anchor, active }, index, rowCount, getRow,
      { multi, range: extend, toggle });
    selectedIds = state.ids; anchor = state.anchor; activeIndex = state.active;
    reveal(activeIndex);
    onselectionchange?.(selectedIds, activeIndex);
  }
  function navigate(key: string, extend = false) {
    selectIndex(navigationIndex(key, active, rowCount, Math.max(1, Math.floor(height / rowHeight) - 1), getRow), extend);
  }
  async function updateSearch(value: string, direction: 1 | -1 = 1, includeCurrent = true, origin = active) {
    query = value;
    const request = ++searchRequest;
    const next = searchRows ? await searchRows(value, origin, direction, includeCurrent)
      : findSearchMatch(rowCount, (i) => selectable(getRow(i)) ? getRow(i).label : null, value, origin, direction, includeCurrent);
    if (request !== searchRequest) return;
    noMatch = next < 0;
    if (next >= 0) selectIndex(next);
  }
  function closeSearch() { searchRequest++; query = ''; noMatch = false; }
  function activate(reason: 'enter' | 'doubleclick' | 'click', index = active) {
    if (index >= 0 && selectable(getRow(index))) onactivate?.(getRow(index), index, reason);
  }
  function keyboard(event: KeyboardEvent) {
    if (event.defaultPrevented || event.isComposing || event.target !== viewport) return;
    if (oncontextmenu && (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) && active >= 0 && selectable(getRow(active))) {
      event.preventDefault(); event.stopPropagation();
      const bounds = document.getElementById(`${id}-${active}`)?.getBoundingClientRect() ?? viewport.getBoundingClientRect();
      oncontextmenu(new MouseEvent('contextmenu', { clientX: bounds.x, clientY: bounds.y + bounds.height, bubbles: true, cancelable: true }), getRow(active), active);
      return;
    }
    if (query) {
      if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); closeSearch(); return; }
      if (event.key === 'Backspace' && !event.metaKey && !event.ctrlKey && !event.altKey) {
        event.preventDefault(); updateSearch(Array.from(query).slice(0, -1).join('')); return;
      }
      if (['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key) && !event.metaKey && !event.ctrlKey && !event.altKey) {
        event.preventDefault();
        const start = event.key === 'Home' || event.key === 'End' ? -1 : active;
        const direction = event.key === 'ArrowUp' || event.key === 'End' ? -1 : 1;
        void updateSearch(query, direction, false, start); return;
      }
      if (['Enter', 'PageUp', 'PageDown', 'ArrowLeft', 'ArrowRight', 'Tab'].includes(event.key)) closeSearch();
    }
    if (actions.dispatch(event, bindings)) return;
    if (onnavigate?.(event, active)) { event.preventDefault(); event.stopPropagation(); return; }
    if (!event.metaKey && !event.ctrlKey && !event.altKey) {
      if (['ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown'].includes(event.key)) {
        event.preventDefault(); event.stopPropagation(); navigate(event.key, event.shiftKey); return;
      }
      if (event.key === 'Enter') { event.preventDefault(); event.stopPropagation(); activate('enter'); return; }
      if (event.key === 'Backspace') { event.preventDefault(); return; }
      if (search && Array.from(event.key).length === 1) { event.preventDefault(); updateSearch(query + event.key); }
    }
  }
  function pointer(event: MouseEvent, index: number) {
    if (event.button !== 0 || !selectable(getRow(index))) return;
    focus(); selectIndex(index, event.shiftKey, event.metaKey || event.ctrlKey);
    if (activateOnClick) activate('click', index);
  }
</script>

<div class="host" bind:this={host} class:focused>
  <div class="probe" aria-hidden="true" bind:this={probe}></div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (The dynamic role is always the composite widget listbox or tree.) -->
  <div class="viewport" bind:this={viewport} {role} tabindex="0" aria-label={label}
    aria-multiselectable={multi} aria-activedescendant={active >= range.start && active < range.end ? `${id}-${active}` : undefined}
    onkeydown={keyboard} onscroll={() => { scrollTop = viewport.scrollTop; scrollLeft = viewport.scrollLeft; }}
    onfocus={() => { focused = true; }} onblur={(event) => { if (!host.contains(event.relatedTarget as Node | null)) { focused = false; closeSearch(); } }}
    oncompositionend={(event) => { if (search && event.data) updateSearch(query + event.data); }}>
    <div class="canvas" role="presentation" style:height={`${offsets?.[rowCount] ?? rowCount * rowHeight}px`} style:min-width={`${minRowWidth}px`}>
      {#each rows as { index, item } (item.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions (Keyboard navigation is owned by the active-descendant list/tree.) -->
        <div id={`${id}-${index}`} class="row" class:selected={selectedIds.has(item.id)} class:disabled={item.disabled}
          class:heading={item.kind === 'heading'} class:separator={item.kind === 'separator'}
          role={item.kind === 'heading' || item.kind === 'separator' ? 'presentation' : role === 'tree' ? 'treeitem' : 'option'}
          aria-selected={selectable(item) ? selectedIds.has(item.id) : undefined} aria-disabled={item.disabled || undefined}
          aria-expanded={role === 'tree' ? item.expanded : undefined} aria-level={role === 'tree' ? item.level : undefined}
          aria-posinset={item.posInSet ?? index + 1} aria-setsize={item.setSize ?? rowCount}
          style:top={`${offsets?.[index] ?? index * rowHeight}px`} style:height={`${offsets ? offsets[index + 1] - offsets[index] : rowHeight}px`}
          onclick={(event) => pointer(event, index)} ondblclick={() => activate('doubleclick', index)}
          oncontextmenu={(event) => { if (selectable(item)) { focus(); if (!selectedIds.has(item.id)) selectIndex(index); oncontextmenu?.(event, item, index); } }}>
          {#if row}{@render row(item, index, query)}{:else if item.kind !== 'separator'}<Highlight text={item.label} {query} />{/if}
        </div>
      {/each}
    </div>
  </div>
  {#if overlay}{@render overlay({ ...range, scrollTop, scrollLeft, width, rowHeight, height })}{/if}
  {#if query}<SpeedSearch {query} {noMatch} label={searchLabel} {noMatchLabel} />{/if}
</div>
<style>
  .host { position: relative; height: 100%; overflow: hidden; min-height: 0; min-width: 0; color: var(--text-primary); background: var(--background-primary); }
  .viewport { height: 100%; overflow: auto; outline: none; }
  .viewport:focus-visible { outline: var(--focus-width) solid var(--focus-border); outline-offset: calc(-1 * var(--focus-width)); }
  .probe { position: absolute; visibility: hidden; height: var(--list-row-height, calc(var(--font-ui-size) * var(--line-height-ui) + var(--space-panel-y))); }
  .canvas { position: relative; }
  .row { position: absolute; width: 100%; display: flex; align-items: center; gap: var(--space-inline); padding-inline: var(--space-inline); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; user-select: none; }
  .selected { background: var(--list-selection-inactive-bg, var(--background-header)); color: var(--list-selection-inactive-text, var(--text-primary)); }
  .focused .selected { background: var(--list-selection-bg, var(--focus-border)); color: var(--list-selection-text, var(--background-primary)); }
  .disabled { opacity: var(--list-disabled-opacity, 0.5); }
  .heading { font-weight: var(--font-weight-heading); color: var(--text-secondary); }
  .separator::after { content: ''; width: 100%; border-top: var(--border-width) solid var(--separator); }
</style>
