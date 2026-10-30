<!-- New theme variables: --toolbar-button-size, --toolbar-hover-bg, --toolbar-active-bg,
     --toolbar-disabled-opacity, --tooltip-bg, --tooltip-text, --tooltip-offset. -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  export interface ToolbarAction {
    id: string; label: string; shortcut?: string; disabled?: boolean;
    pressed?: boolean; icon: Snippet; run: () => void;
  }
  let { label, items }: { label: string; items: readonly (ToolbarAction | { id: string; separator: true })[] } = $props();
  let focused = $state<string | null>(null);
  let hovered = $state<string | null>(null);
  const id = $props.id();
  function keyboard(event: KeyboardEvent) {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    const host = event.currentTarget as HTMLElement;
    const buttons = Array.from(host.querySelectorAll<HTMLButtonElement>('button:not(:disabled)'));
    if (!buttons.length) return;
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1
      : (index + (event.key === 'ArrowLeft' ? -1 : 1) + buttons.length) % buttons.length;
    event.preventDefault(); buttons[next].focus();
  }
</script>
<!-- svelte-ignore a11y_interactive_supports_focus (Native buttons are the toolbar tab stops.) -->
<div class="toolbar" role="toolbar" aria-label={label} onkeydown={keyboard}>
  {#each items as item (item.id)}
    {#if 'separator' in item}<span class="separator" role="separator" aria-orientation="vertical"></span>
    {:else}
      <span class="item" role="presentation" onpointerenter={() => { hovered = item.id; }} onpointerleave={() => { hovered = null; }}>
        <button type="button" aria-label={item.label} aria-pressed={item.pressed} disabled={item.disabled}
          aria-describedby={`${id}-${item.id}`} class:pressed={item.pressed}
          onclick={item.run} onfocus={() => { focused = item.id; }} onblur={() => { focused = null; }}>
          <span aria-hidden="true">{@render item.icon()}</span>
        </button>
        <span id={`${id}-${item.id}`} role="tooltip" class="tooltip" class:visible={focused === item.id || hovered === item.id}>
          {item.label}{#if item.shortcut}<span class="shortcut">{item.shortcut}</span>{/if}
        </span>
      </span>
    {/if}
  {/each}
</div>
<style>
  .toolbar { display: flex; align-items: center; gap: var(--space-inline); padding: var(--space-panel-y) var(--space-inline); }
  .item { position: relative; display: inline-flex; }
  button { display: inline-flex; align-items: center; justify-content: center; width: var(--toolbar-button-size, calc(var(--space-panel-x) * 2));
    height: var(--toolbar-button-size, calc(var(--space-panel-x) * 2)); border: 0; padding: 0; font: inherit; color: var(--text-primary); background: none; }
  button:hover { background: var(--toolbar-hover-bg, var(--background-secondary)); }
  button.pressed { background: var(--toolbar-active-bg, var(--background-header)); }
  button:disabled { opacity: var(--toolbar-disabled-opacity, 0.5); }
  button:focus-visible { outline: var(--focus-width) solid var(--focus-border); }
  .separator { height: var(--toolbar-button-size, calc(var(--space-panel-x) * 2)); border-inline-start: var(--border-width) solid var(--separator); }
  .tooltip { display: none; position: absolute; inset-block-start: calc(100% + var(--tooltip-offset, var(--space-inline))); inset-inline-start: 0;
    z-index: 20; width: max-content; padding: var(--space-panel-y) var(--space-inline); background: var(--tooltip-bg, var(--background-secondary));
    color: var(--tooltip-text, var(--text-primary)); border: var(--border-width) solid var(--separator); pointer-events: none; }
  .tooltip.visible { display: flex; gap: var(--space-inline); }
  .shortcut { color: var(--text-secondary); }
</style>
