<script lang="ts">
  import type { Snippet } from 'svelte';

  let { title, children }: { title: string; children?: Snippet } = $props();
</script>

<section class="panel" aria-label={title}>
  <h2>{title}</h2>
  {#if children}
    <div class="content">{@render children()}</div>
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 0;
    min-height: 0;
    background: var(--background-primary);
  }
  h2 {
    flex: none;
    margin: 0;
    padding: var(--space-panel-y) var(--space-panel-x);
    border-bottom: var(--border-width) solid var(--separator);
    background: var(--background-secondary);
    color: var(--text-primary);
    font-size: var(--font-ui-size);
    font-weight: var(--font-weight-heading);
    line-height: var(--line-height-ui);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .content { flex: 1; min-width: 0; min-height: 0; }
</style>
