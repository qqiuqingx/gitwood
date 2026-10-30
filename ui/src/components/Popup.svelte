<!-- New theme variables: --popup-width, --popup-height, --popup-viewport-margin,
     --popup-bg, --popup-border, --popup-shadow, --popup-padding. -->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Popup from './Popup.svelte';
  import { controlText } from '../i18n/controls';
  import List from './List.svelte';
  import Highlight from './Highlight.svelte';
  import { positionPopup, type PopupItem, type Rect } from './popup';
  import { selectable } from './list';
  let { items, label, anchor, onclose, onexecute, submenu = false, defaultAction, searchLabel = controlText('search'), noMatchLabel = controlText('noMatch') }: {
    items: readonly PopupItem[]; label: string; anchor: Rect | HTMLElement; onclose: () => void;
    onexecute?: () => void;
    submenu?: boolean; defaultAction?: { label: string; run: () => void; enabled?: boolean };
    searchLabel?: string; noMatchLabel?: string;
  } = $props();
  let host: HTMLDivElement;
  let list: List;
  let marginProbe: HTMLDivElement;
  let activeIndex = $state(-1);
  let child = $state<{ items: readonly PopupItem[]; label: string; anchor: HTMLElement } | null>(null);
  let closed = false;
  let reposition: (() => void) | undefined;
  function close() { if (closed) return; closed = true; onclose(); }
  function choose(index: number, onlySubmenu = false) {
    const item = items[index];
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
    void tick().then(() => { if (!closed) { list.focus(); const first = items.findIndex(selectable); if (first >= 0) list.selectIndex(first); } });
    return () => {
      observer.disconnect(); document.removeEventListener('pointerdown', outside, true);
      window.removeEventListener('resize', position); window.removeEventListener('scroll', position, true);
      reposition = undefined; closed = true;
      // Wait until all submenu DOM is detached so its cleanup cannot steal focus back.
      void tick().then(() => { if (previousFocus?.isConnected) previousFocus.focus({ preventScroll: true }); });
    };
  });
  $effect(() => { anchor; reposition?.(); });
  function keyboard(event: KeyboardEvent) {
    if (event.defaultPrevented || child) return;
    if (event.key === 'Escape' || (submenu && event.key === 'ArrowLeft')) {
      event.preventDefault(); event.stopPropagation(); close();
    } else if (event.key === 'Enter' && runDefault()) { event.preventDefault(); event.stopPropagation(); }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions (The dialog delegates navigation to its list and handles dismiss/default actions.) -->
<div class="popup" role="dialog" aria-label={label} tabindex="-1" bind:this={host} onkeydown={keyboard}>
  <div class="margin-probe" bind:this={marginProbe} aria-hidden="true"></div>
  <div class="content">
    <List bind:this={list} rowCount={items.length} getRow={(index) => items[index]} {label} bind:activeIndex
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
        {#if item.kind !== 'separator'}<span class="label"><Highlight text={item.label} {query} /></span>
          {#if items[index].children?.length}<span aria-hidden="true">▸</span>{/if}
        {/if}
      {/snippet}
    </List>
  </div>
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
  button { font: inherit; padding: var(--space-panel-y) var(--space-inline); color: var(--text-primary); background: var(--background-secondary); border: var(--border-width) solid var(--separator); }
  button:focus-visible { outline: var(--focus-width) solid var(--focus-border); }
</style>
