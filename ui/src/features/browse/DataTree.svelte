<script lang="ts">
  import Tree from '../../components/Tree.svelte';
  import Highlight from '../../components/Highlight.svelte';
  import type { BrowseTree, BrowseNode } from './model';
  let { tree, label, expanded = $bindable(new Set<string>() as ReadonlySet<string>), selected = $bindable(new Set<string>() as ReadonlySet<string>), onselect }: {
    tree: BrowseTree; label: string; expanded?: ReadonlySet<string>; selected?: ReadonlySet<string>; onselect?: (node: BrowseNode) => void;
  } = $props();
</script>
<Tree source={tree.source} {label} {expanded} onexpandedchange={ids => { expanded = ids; }} bind:selectedIds={selected}
  onselectionchange={(_, row) => { const node = row && tree.nodes.get(row.id); if (node) onselect?.(node); }}>
  {#snippet content(row, query)}
    {@const node = tree.nodes.get(row.id)}
    <span class="name" style:color={node?.status ? `var(--status-${node.status})` : undefined} title={node?.path?.display ?? row.label}>
      <Highlight text={row.label} {query} />
    </span>
    {#if node?.note}<span class="note" title={node.note}>{node.note}</span>{/if}
  {/snippet}
</Tree>
<style>
  .name { min-width: 0; display: flex; }
  .note { min-width: 0; overflow: hidden; text-overflow: ellipsis; color: var(--text-secondary); font-size: var(--font-small-size); }
</style>
