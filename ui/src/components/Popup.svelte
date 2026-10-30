<!-- New theme variables: --popup-width, --popup-height, --popup-viewport-margin,
     --popup-bg, --popup-border, --popup-shadow, --popup-padding. -->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Popup from './Popup.svelte';
  import { controlText } from '../i18n/controls';
  import List from './List.svelte';
  import Highlight from './Highlight.svelte';
  import { filterPopupItems, positionPopup, type PopupItem, type Rect } from './popup';
  import { selectable } from './list';
  let { items, label, anchor, onclose, onexecute, submenu = false, defaultAction, filter = false, searchLabel = controlText('search'), noMatchLabel = controlText('noMatch') }: {
    items: readonly PopupItem[]; label: string; anchor: Rect | HTMLElement; onclose: () => void;
    onexecute?: () => void;
    submenu?: boolean; defaultAction?: { label: string; run: () => void; enabled?: boolean };
    searchLabel?: string; noMatchLabel?: string;
    filter?: boolean;
  } = $props();
  let host: HTMLDivElement;
  let list: List;
  let marginProbe: HTMLDivElement;
  let activeIndex = $state(-1);
  let filterQuery = $state('');
  let searchInput = $state<HTMLInputElement>();
  const visibleItems = $derived(filter ? filterPopupItems(items, filterQuery) : items);
  const rowScales = $derived(items.some(item => item.description || item.groupHeader)
    ? visibleItems.map(item => item.description ? 2 : item.groupHeader ? 1.5 : 1) : undefined);
  let child = $state<{ items: readonly PopupItem[]; label: string; anchor: HTMLElement } | null>(null);
  let closed = false;
  let reposition: (() => void) | undefined;
  function close() { if (closed) return; closed = true; onclose(); }
  function choose(index: number, onlySubmenu = false) {
    const item = visibleItems[index];
    if (!item || !selectable(item)) return;
    if (item.children?.length) {
      const selected = host.querySelector('[aria-selected="true"]');
      child = { items: item.children, label: item.label, anchor: selected instanceof HTMLElement ? selected : host };
    } else if (!onlySubmenu && item.run) { close(); onexecute?.(); item.run(); }
  }
  function runDefault() {
    if (!defaultAction || defaultAction.enabled === false) return false;
    close(); onexecute?.(); defaultAction.run(); return true;
  }
  onMount(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const position = () => {
      if (anchor instanceof HTMLElement && !anchor.isConnected) { close(); return; }
      const margin = marginProbe.getBoundingClientRect().width;
      host.style.maxWidth = `${Math.max(0, window.innerWidth - 2 * margin)}px`;
      host.style.maxHeight = `${Math.max(0, window.innerHeight - 2 * margin)}px`;
      const rect = anchor instanceof HTMLElement ? anchor.getBoundingClientRect() : anchor;
      const result = positionPopup(rect, { width: host.offsetWidth, height: host.offsetHeight },
        { width: window.innerWidth, height: window.innerHeight }, margin, submenu);
      host.style.left = `${result.x}px`; host.style.top = `${result.y}px`;
    };
    reposition = position;
    const outside = (event: PointerEvent) => { if (!host.contains(event.target as Node)) close(); };
    document.addEventListener('pointerdown', outside, true);
    window.addEventListener('resize', position);
    // Anchor movement through scrolling must reposition the layer; no idle polling.
    window.addEventListener('scroll', position, true);
    const observer = new ResizeObserver(position); observer.observe(host); observer.observe(marginProbe);
    position();
    void tick().then(() => { if (!closed) { if (filter) searchInput?.focus(); else list.focus(); const first = visibleItems.findIndex(selectable); if (first >= 0) list.selectIndex(first); } });
    return () => {
      observer.disconnect(); document.removeEventListener('pointerdown', outside, true);
      window.removeEventListener('resize', position); window.removeEventListener('scroll', position, true);
      reposition = undefined; closed = true;
      // Wait until all submenu DOM is detached so its cleanup cannot steal focus back.
      void tick().then(() => { if (previousFocus?.isConnected) previousFocus.focus({ preventScroll: true }); });
    };
  });
  $effect(() => { anchor; reposition?.(); });
  $effect(() => {
    const current = visibleItems;
    if (filter) { child = null; void tick().then(() => { if (!closed) list?.selectIndex(current.findIndex(selectable)); }); }
  });
  function keyboard(event: KeyboardEvent) {
    if (event.defaultPrevented || child) return;
    if (event.key === 'Escape' || (submenu && event.key === 'ArrowLeft')) {
      event.preventDefault(); event.stopPropagation(); close();
    } else if (event.key === 'Enter' && runDefault()) { event.preventDefault(); event.stopPropagation(); }
    else if (filter && event.target === searchInput && ['ArrowDown', 'ArrowUp', 'Enter'].includes(event.key)) {
      event.preventDefault(); event.stopPropagation();
      if (event.key === 'Enter') choose(activeIndex); else list.focus();
    } else if (filter && event.target !== searchInput && !event.ctrlKey && !event.metaKey && !event.altKey && event.key.length === 1) {
      event.preventDefault(); filterQuery += event.key; searchInput?.focus();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions (The dialog delegates navigation to its list and handles dismiss/default actions.) -->
<div class="popup" role="dialog" aria-label={label} tabindex="-1" bind:this={host} onkeydown={keyboard}>
  <div class="margin-probe" bind:this={marginProbe} aria-hidden="true"></div>
  {#if filter}<input class="search" bind:this={searchInput} bind:value={filterQuery} aria-label={searchLabel} placeholder={searchLabel} />{/if}
  <div class="content">
    <List bind:this={list} rowCount={visibleItems.length} {rowScales} getRow={(index) => visibleItems[index]} {label} bind:activeIndex search={!filter}
      {searchLabel} {noMatchLabel} activateOnClick onactivate={(_, index) => choose(index)}
      onselectionchange={() => { child = null; }}
      onnavigate={(event, index) => {
        if (event.metaKey || event.ctrlKey || event.altKey) return false;
        if (event.key === 'ArrowRight') { choose(index, true); return true; }
        if (event.key === 'ArrowLeft' && submenu) { close(); return true; }
        if (event.key === 'Enter' && defaultAction) { runDefault(); return true; }
        return false;
      }}>
      {#snippet row(item, index, query)}
        {@const entry = visibleItems[index]}
        {#if item.kind !== 'separator'}
          {#if entry.checked !== undefined}<span aria-hidden="true">{entry.checked ? '✓' : '　'}</span><span class="check-state" role="checkbox" aria-label={item.label} aria-checked={entry.checked}></span>{/if}
          {#if entry.icon}{@render entry.icon(entry.id)}{/if}
          <span class="label" class:group-header={entry.groupHeader} class:first-group={entry.firstGroup} title={entry.hint}>
            <Highlight text={item.label} query={filter ? filterQuery : query} />
            {#if entry.description}<span class="description">{entry.description}</span>{/if}
          </span>
          {#if entry.shortcut}<span class="shortcut">{entry.shortcut}</span>{/if}
          {#if entry.children?.length}<span aria-hidden="true">▸</span>{/if}
        {/if}
      {/snippet}
    </List>
  </div>
  {#if filter && !visibleItems.length}<span class="no-match">{noMatchLabel}</span>{/if}
  {#if defaultAction}<button type="button" disabled={defaultAction.enabled === false} onclick={runDefault}>{defaultAction.label}</button>{/if}
  {#if child}<Popup items={child.items} label={child.label} anchor={child.anchor} submenu
    {searchLabel} {noMatchLabel} onclose={() => { child = null; }} onexecute={() => { close(); onexecute?.(); }} />{/if}
</div>
<style>
  .popup { position: fixed; z-index: 10; display: flex; flex-direction: column; width: var(--popup-width, calc(var(--font-ui-size) * 24));
    height: var(--popup-height, calc(var(--font-ui-size) * 24)); padding: var(--popup-padding, var(--space-panel-y));
    background: var(--popup-bg, var(--background-primary)); color: var(--text-primary);
    border: var(--border-width) solid var(--popup-border, var(--separator)); box-shadow: 0 4px 16px var(--popup-shadow); }
  .margin-probe { position: absolute; visibility: hidden; pointer-events: none; width: var(--popup-viewport-margin, var(--space-inline)); height: 0; }
  .content { flex: 1; min-height: 0; }
  .label { flex: 1; overflow: hidden; text-overflow: ellipsis; }
  .description { display: block; color: var(--text-secondary); font-size: var(--font-small-size); overflow: hidden; text-overflow: ellipsis; }
  .shortcut { flex: none; color: var(--text-secondary); font-size: var(--font-small-size); }
  .group-header { align-self: stretch; display: flex; align-items: center; font-size: var(--font-small-size); font-weight: var(--font-weight-heading); border-top: var(--border-width) solid var(--separator); }
  .group-header.first-group { border-top: 0; }
  .search { flex: none; font: inherit; padding: var(--space-panel-y); background: var(--search-bg); color: var(--search-text); border: var(--border-width) solid var(--search-border); }
  .no-match { padding: var(--space-panel-y); color: var(--text-secondary); }
  .check-state { position: absolute; width: 0; height: 0; overflow: hidden; }
  button { font: inherit; padding: var(--space-panel-y) var(--space-inline); color: var(--text-primary); background: var(--background-secondary); border: var(--border-width) solid var(--separator); }
  button:focus-visible { outline: var(--focus-width) solid var(--focus-border); }
</style>
