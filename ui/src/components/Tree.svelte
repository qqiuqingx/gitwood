<!-- New theme variables: --tree-indent, --tree-disclosure-size. List/Highlight own row/selection variables. -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import List from './List.svelte';
  import { controlText } from '../i18n/controls';
  import Highlight from './Highlight.svelte';
  import { flattenTree, type TreeSource, type TreeRow } from './tree';
  let { source, visibleRows, onrangechange, onviewportchange, scrollTop = $bindable(0), activeIndex = $bindable(-1), onquerychange, searchRows, search = true, label, expanded, onexpandedchange, selectedIds = $bindable(new Set<string>() as ReadonlySet<string>),
    content, onselectionchange, onactivate, onspace, searchLabel = controlText('search'), noMatchLabel = controlText('noMatch'),
  }: {
    scrollTop?: number;
    activeIndex?: number;
    onviewportchange?: (viewport: { start: number; end: number; rowHeight: number; scrollTop: number; scrollLeft: number; width: number; height: number }) => void;
    onquerychange?: (query: string) => void;
    search?: boolean;
    searchRows?: (query: string, active: number, direction: 1 | -1, includeCurrent: boolean) => Promise<number>;
    content?: Snippet<[TreeRow, string]>;
    source?: TreeSource;
    visibleRows?: { count: number; getRow: (index: number) => TreeRow };
    onrangechange?: (range: { start: number; end: number }) => void; label: string; expanded: ReadonlySet<string>;
    onexpandedchange: (ids: ReadonlySet<string>) => void;
    selectedIds?: ReadonlySet<string>;
    onselectionchange?: (ids: ReadonlySet<string>, row: TreeRow | undefined) => void;
    onspace?: (ids: readonly string[]) => void;
    onactivate?: (row: TreeRow) => void; searchLabel?: string; noMatchLabel?: string;
  } = $props();
  // Expansion belongs to the caller: async children and future refreshes share one source of truth.
  const rows = $derived(source ? flattenTree(source, expanded) : []);
  const rowCount = $derived(visibleRows?.count ?? rows.length);
  const rowAt = $derived(visibleRows?.getRow ?? ((index: number) => rows[index]));
  let activeId = $state<string | null>(null);
  let list: List;
  const currentActive = $derived(visibleRows ? activeIndex : rows.findIndex((row) => row.id === activeId));
  function expand(id: string, value: boolean) {
    const next = new Set(expanded);
    if (value) { next.add(id); next.delete(`!${id}`); }
    else { next.delete(id); if (next.has('*')) next.add(`!${id}`); }
    onexpandedchange(next);
  }
  function keyboard(event: KeyboardEvent, index: number): boolean {
    if (event.metaKey || event.ctrlKey || event.altKey || !['ArrowLeft', 'ArrowRight', ' '].includes(event.key)) return false;
    const row = rowAt(index);
    if (!event.metaKey && !event.ctrlKey && !event.altKey && event.key === ' ' && onspace) {
      onspace(selectedIds.size ? [...selectedIds] : row ? [row.id] : []); return true;
    }
    if (!row) return true;
    if (event.key === 'ArrowRight') {
      if (row.hasChildren && !row.expanded) expand(row.id, true);
      else if (row.expanded && index + 1 < rowCount && rowAt(index + 1).parentIndex === index) list.selectIndex(index + 1, event.shiftKey);
    } else {
      if (row.expanded) expand(row.id, false);
      else if (row.parentIndex >= 0) list.selectIndex(row.parentIndex, event.shiftKey);
    }
    return true;
  }
</script>
<List bind:this={list} role="tree" {rowCount} getRow={rowAt} bind:scrollTop {onrangechange} {onviewportchange} {onquerychange} {label}
  {search} {searchRows} multi bind:selectedIds activeIndex={currentActive} {searchLabel} {noMatchLabel} onnavigate={keyboard}
  onselectionchange={(ids, index) => { activeIndex = index; activeId = rowAt(index)?.id ?? null; onselectionchange?.(ids, rowAt(index)); }}
  onactivate={(_, index, reason) => { const row = rowAt(index); if (reason === 'doubleclick' && row.hasChildren) expand(row.id, !row.expanded); else onactivate?.(row); }}>
  {#snippet row(_, index, query)}
    {@const node = rowAt(index)}
    <span class="node" style:padding-inline-start={`calc(${(node.level ?? 1) - 1} * var(--tree-indent, var(--space-panel-x)))`}>
      {#if node.hasChildren}
        <button type="button" tabindex="-1" disabled={node.disabled} aria-label={node.label} aria-expanded={node.expanded}
          onclick={(event) => { event.stopPropagation(); list.focus(); list.selectIndex(index); expand(node.id, !node.expanded); }}
          ondblclick={(event) => event.stopPropagation()}>{node.expanded ? '▾' : '▸'}</button>
      {:else}<span class="spacer"></span>{/if}
      {#if content}{@render content(node, query)}{:else}<Highlight text={node.label} {query} />{/if}
    </span>
  {/snippet}
</List>
<style>
  .node { display: flex; align-items: center; gap: var(--space-inline); min-width: 0; }
  button, .spacer { flex: none; width: var(--tree-disclosure-size, var(--space-panel-x)); }
  button { border: 0; padding: 0; background: none; color: inherit; font: inherit; cursor: pointer; }
</style>
