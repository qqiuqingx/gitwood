<!-- New theme variables: --tab-bg, --tab-active-bg, --tab-active-border,
     --tab-disabled-opacity, --tab-close-size. -->
<script lang="ts">
  import { tick, type Snippet } from 'svelte';
  import { createActionRegistry, isEditingTarget } from '../keymap/registry';
  import { keymapFor } from '../keymap/keymaps';
  import { detectPlatform } from '../keymap/shortcuts';
  export interface Tab { id: string; label: string; disabled?: boolean; closeLabel?: string }
  let { label, tabs, activeId, onchange, onclose, panel }: {
    label: string; tabs: readonly Tab[]; activeId: string; onchange: (id: string) => void;
    onclose?: (id: string) => void; panel?: Snippet<[string]>;
  } = $props();
  const id = $props.id();
  let host: HTMLDivElement;
  const actions = createActionRegistry();
  actions.register('NextTab', { run: () => move(1) });
  actions.register('PreviousTab', { run: () => move(-1) });
  const canClose = () => !!onclose && !!tabs.find((tab) => tab.id === activeId && tab.closeLabel && !tab.disabled);
  actions.register('CloseContent', { run: () => closeTab(activeId), enabled: canClose });
  actions.register('CloseActiveTab', { run: () => closeTab(activeId), enabled: canClose });
  async function closeTab(tabId: string) {
    onclose?.(tabId);
    await tick();
    const index = tabs.findIndex((tab) => tab.id === activeId && !tab.disabled);
    host.querySelector<HTMLButtonElement>(`[data-tab-index="${index}"]`)?.focus();
  }
  function select(tab: Tab) {
    if (tab.disabled) return;
    onchange(tab.id);
    const index = tabs.indexOf(tab);
    host.querySelector<HTMLButtonElement>(`[data-tab-index="${index}"]`)?.focus();
  }
  function move(direction: number) {
    const available = tabs.filter((tab) => !tab.disabled);
    if (!available.length) return;
    const index = available.findIndex((tab) => tab.id === activeId);
    select(available[(index + direction + available.length) % available.length]);
  }
  function keyboard(event: KeyboardEvent) {
    if (event.defaultPrevented) return;
    if (actions.dispatch(event, keymapFor(detectPlatform(navigator.platform)), isEditingTarget(event.target))) return;
    if (!(event.target instanceof HTMLElement) || !event.target.closest('[role="tablist"]')) return;
    if (event.metaKey || event.ctrlKey || event.altKey || !['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    if (event.key === 'Home' || event.key === 'End') {
      const available = tabs.filter((tab) => !tab.disabled);
      const tab = event.key === 'Home' ? available[0] : available.at(-1); if (tab) select(tab);
    } else move(event.key === 'ArrowLeft' ? -1 : 1);
  }
</script>
<!-- svelte-ignore a11y_no_static_element_interactions (Scoped keymap also handles shortcuts from the active panel.) -->
<div class="tabs" bind:this={host} onkeydown={keyboard}>
  <!-- svelte-ignore a11y_interactive_supports_focus (The active native tab button is the composite tab stop.) -->
  <div role="tablist" aria-label={label}>
    {#each tabs as tab, index (tab.id)}
      <span class="tab" class:active={activeId === tab.id}>
        <button type="button" role="tab" id={`${id}-tab-${index}`} data-tab-index={index}
          aria-selected={activeId === tab.id} aria-controls={`${id}-panel`} disabled={tab.disabled}
          tabindex={activeId === tab.id || (!tabs.some((entry) => entry.id === activeId && !entry.disabled) && index === tabs.findIndex((entry) => !entry.disabled)) ? 0 : -1} onclick={() => select(tab)}>{tab.label}</button>
        {#if tab.closeLabel && onclose}<button class="close" type="button" aria-label={tab.closeLabel} disabled={tab.disabled}
          onclick={() => closeTab(tab.id)}>×</button>{/if}
      </span>
    {/each}
  </div>
  <div class="panel" role="tabpanel" id={`${id}-panel`} aria-labelledby={tabs.some((tab) => tab.id === activeId) ? `${id}-tab-${tabs.findIndex((tab) => tab.id === activeId)}` : undefined} tabindex="0">
    {#if panel}{@render panel(activeId)}{/if}
  </div>
</div>
<style>
  .tabs { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  [role='tablist'] { display: flex; border-bottom: var(--border-width) solid var(--separator); }
  .tab { display: flex; align-items: center; background: var(--tab-bg, var(--background-secondary)); border-bottom: var(--focus-width) solid var(--separator); }
  .tab.active { background: var(--tab-active-bg, var(--background-primary)); border-color: var(--tab-active-border, var(--focus-border)); }
  button { font: inherit; color: var(--text-primary); background: none; border: 0; padding: var(--space-panel-y) var(--space-inline); }
  button:disabled { opacity: var(--tab-disabled-opacity, 0.5); }
  button:focus-visible, .panel:focus-visible { outline: var(--focus-width) solid var(--focus-border); outline-offset: calc(-1 * var(--focus-width)); }
  .close { width: var(--tab-close-size, calc(var(--space-panel-x) * 2)); padding-inline: 0; }
  .panel { flex: 1; min-height: 0; outline: none; }
</style>
