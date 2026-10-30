<script lang="ts">
  import DataTree from './DataTree.svelte';
  import { branchTree } from './model';
  import type { References } from './types';
  import { browseText as t } from '../../i18n/browse';
  let { refs, branch, select }: { refs: References | null; branch: string; select: (ref: string) => void } = $props();
  const tree = $derived(refs ? branchTree(refs, { local: t('local'), remote: t('remote'), tags: t('tags'), head: t('head'), gone: t('failed') }) : null);
  let expanded = $state<ReadonlySet<string>>(new Set(['local', 'remote', 'tags']));
  const selected = $derived(new Set(branch ? [branch] : ['HEAD']));
  let knownDirectories = new Set<string>();
  $effect(() => {
    if (!tree) return;
    const next = new Set(expanded); let changed = false;
    for (const id of tree.directories) if (!knownDirectories.has(id)) { next.add(id); changed = true; }
    knownDirectories = new Set(tree.directories);
    if (changed) expanded = next;
  });
</script>
{#if tree}<DataTree {tree} label={t('branch')} bind:expanded {selected} onselect={node => { if (node.ref !== undefined) select(node.ref); }} />{/if}
