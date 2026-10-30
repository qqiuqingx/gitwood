<!-- New theme variables: --search-bg, --search-text, --search-border,
     --search-error-bg, --search-error-text, --search-width. -->
<script lang="ts">
  let { query, noMatch, label, noMatchLabel }: {
    query: string; noMatch: boolean; label: string; noMatchLabel: string;
  } = $props();
</script>
<div class="search" class:unmatched={noMatch}>
  <input aria-label={label} aria-invalid={noMatch} readonly tabindex="-1" onpointerdown={(event) => event.preventDefault()} value={query} />
  <span role="status">{noMatch ? noMatchLabel : ''}</span>
</div>
<style>
  .search { position: absolute; inset-block-start: 0; inset-inline-end: var(--space-inline); z-index: 1;
    background: var(--search-bg, var(--background-primary)); color: var(--search-text, var(--text-primary));
    border: var(--border-width) solid var(--search-border, var(--focus-border)); padding: var(--space-panel-y) var(--space-inline); }
  input { width: var(--search-width, calc(var(--font-ui-size) * 14)); font: inherit; color: inherit; background: inherit; border: 0; padding: 0; outline: none; }
  .unmatched { background: var(--search-error-bg, var(--background-header)); color: var(--search-error-text, var(--text-secondary)); }
  span { display: block; font-size: var(--font-small-size); }
</style>
