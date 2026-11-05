<script lang="ts">
  import { onMount } from 'svelte';
  import { historyMenuText as t } from '../../i18n/history-menu';
  let { children, onselect, onclose }: { children: string[]; onselect: (hash: string) => void; onclose: () => void } = $props();
  let element: HTMLDialogElement;
  onMount(() => { element.showModal(); });
</script>
<dialog bind:this={element} aria-labelledby="children-title" oncancel={event => { event.preventDefault(); onclose(); }}>
  <header><h2 id="children-title">{t('chooseChild')}</h2><button onclick={onclose}>{t('cancel')}</button></header>
  <ol>{#each children as hash, index}<li><button onclick={() => onselect(hash)}><span>{index + 1}.</span><code>{hash.slice(0, 12)}</code></button></li>{/each}</ol>
</dialog>
<style>
  dialog { width: min(520px, 84vw); max-height: 75vh; overflow: auto; color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  header { display: flex; align-items: center; gap: var(--space-inline); } h2 { flex: 1; } button { font: inherit; }
  ol { list-style: none; padding: 0; } li button { width: 100%; display: flex; gap: var(--space-inline); text-align: start; padding: var(--space-panel-y) var(--space-inline); border: 0; color: inherit; background: transparent; cursor: pointer; } li button:hover { background: var(--selection-bg); }
  code { font-family: var(--font-editor-family); }
</style>
