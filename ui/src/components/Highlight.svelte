<!-- New theme variables: --search-match-bg, --search-match-text. -->
<script lang="ts">
  import { highlightParts, matchSpeedSearch } from './speed-search';
  let { text, query = '' }: { text: string; query?: string } = $props();
  const parts = $derived(highlightParts(text, matchSpeedSearch(query, text) ?? []));
</script>
<span class="highlight">{#each parts as part}{#if part.matched}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}</span>
<style>
  .highlight { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: pre; }
  mark { color: var(--search-match-text, var(--text-primary)); background: var(--search-match-bg, var(--background-header)); }
</style>
