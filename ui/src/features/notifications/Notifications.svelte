<script lang="ts">
  import type { NotificationCenter } from './center.svelte';
  import { controlText } from '../../i18n/controls';
  let { center }: { center: NotificationCenter } = $props();
</script>

{#if center.items.length}
  <div class="notifications">
    {#each center.items as notice (notice.id)}
      <section class="notice {notice.kind}" role={notice.sticky ? 'alert' : 'status'} aria-label={notice.title}>
        <header><strong>{notice.title}</strong><button class="close" aria-label={controlText('close')} title={controlText('close')} onclick={() => center.dismiss(notice.id)}>×</button></header>
        {#if notice.detail}<p class="detail">{notice.detail}</p>{/if}
        {#if notice.files?.length}<ul>{#each notice.files.slice(0, 5) as file}<li>{file}</li>{/each}{#if notice.files.length > 5}<li>…</li>{/if}</ul>{/if}
        {#if notice.actions?.length}<footer>{#each notice.actions as action}<button class="link" onclick={() => { center.dismiss(notice.id); action.run(); }}>{action.label}</button>{/each}</footer>{/if}
      </section>
    {/each}
  </div>
{/if}

<style>
  .notifications { position: fixed; inset-inline-end: var(--space-panel-x); inset-block-end: calc(var(--space-panel-x) * 2); z-index: 20; display: flex; flex-direction: column; gap: var(--space-inline); width: min(calc(var(--font-ui-size) * 28), 80vw); }
  .notice { padding: var(--space-panel-y) var(--space-panel-x); color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); border-inline-start: calc(var(--border-width) * 3) solid var(--accent); box-shadow: 0 2px 8px #0003; }
  .notice.success { border-inline-start-color: var(--status-added); }
  .notice.warning { border-inline-start-color: var(--status-modified); }
  .notice.error { border-inline-start-color: var(--status-conflicted); }
  header { display: flex; align-items: start; gap: var(--space-inline); } header strong { flex: 1; font-weight: var(--font-weight-heading); }
  .close { border: 0; background: none; color: var(--text-secondary); font: inherit; padding: 0 2px; cursor: pointer; }
  .detail { margin: calc(var(--space-panel-y) / 2) 0 0; white-space: pre-wrap; overflow-wrap: anywhere; max-height: calc(var(--font-ui-size) * 8); overflow: auto; color: var(--text-secondary); font-size: var(--font-small-size); }
  ul { margin: calc(var(--space-panel-y) / 2) 0 0; padding-inline-start: calc(var(--space-inline) * 2); font-size: var(--font-small-size); }
  footer { display: flex; flex-wrap: wrap; gap: var(--space-panel-x); margin-top: var(--space-panel-y); }
  .link { font: inherit; color: var(--accent); background: none; border: 0; padding: 0; cursor: pointer; }
</style>
