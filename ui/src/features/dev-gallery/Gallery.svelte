<!-- New theme variables: --gallery-height, --gallery-gap. Loaded only from the DEV hash entry in main.ts. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import DiffView from '../diff/DiffView.svelte';
  import { diffText } from '../../i18n/diff';
  import { diffLeft, diffRight, diffExample } from './diff-example';
  import List from '../../components/List.svelte';
  import Tree from '../../components/Tree.svelte';
  import Popup from '../../components/Popup.svelte';
  import Toolbar from '../../components/Toolbar.svelte';
  import Tabs from '../../components/Tabs.svelte';
  import { controlText as t } from '../../i18n/controls';
  import type { TreeNode, TreeSource } from '../../components/tree';
  import type { Rect } from '../../components/popup';
  import { createActionRegistry, isEditingTarget } from '../../keymap/registry';
  import { detectPlatform } from '../../keymap/shortcuts';
  import { keymapFor } from '../../keymap/keymaps';
  let tab = $state('list');
  let showTreeTab = $state(true);
  let expanded = $state<ReadonlySet<string>>(new Set(['src']));
  let selected = $state<ReadonlySet<string>>(new Set());
  let preview = $state(false);
  let popup = $state<Rect | null>(null);
  let result = $state('');
  let popupButton: HTMLButtonElement;
  const nodes: Record<string, TreeNode[]> = {
    root: [{ id: 'src', label: 'src', hasChildren: true }, { id: 'docs', label: 'docs', hasChildren: true }],
    src: [{ id: 'GitBranchPanel', label: 'GitBranchPanel.svelte', hasChildren: false }, { id: 'GitLogView', label: 'GitLogView.svelte', hasChildren: false }],
    docs: [{ id: 'plan', label: 'implementation-plan.md', hasChildren: false }],
  };
  const source: TreeSource = { rootCount: nodes.root.length, getRoot: (i) => nodes.root[i],
    childCount: (id) => nodes[id]?.length ?? 0, getChild: (id, i) => nodes[id][i] };
  function openPopup() {
    const rect = popupButton.getBoundingClientRect();
    popup = { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
  }
  onMount(() => {
    // Concrete integration of window-scope actions; App decides the eventual shell targets.
    const actions = createActionRegistry();
    actions.register('HideActiveWindow', { run: () => { popup = null; }, enabled: () => !!popup });
    const bindings = keymapFor(detectPlatform(navigator.platform));
    const handler = (event: KeyboardEvent) => actions.dispatch(event, bindings, isEditingTarget(event.target));
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  });
</script>
<section aria-label={t('gallery')}>
  {#snippet openIcon()}⋯{/snippet}{#snippet previewIcon()}◫{/snippet}
  <Toolbar label={t('actions')} items={[
    { id: 'popup', label: t('openPopup'), icon: openIcon, run: openPopup },
    { id: 'divider', separator: true },
    { id: 'preview', label: t('preview'), icon: previewIcon, pressed: preview, run: () => { preview = !preview; } },
    { id: 'disabled', label: t('unavailable'), icon: previewIcon, disabled: true, run: () => {} },
  ]} />
  <button type="button" bind:this={popupButton} onclick={openPopup}>{t('openPopup')}</button>
  <div class="examples">
    <Tabs label={t('gallery')} activeId={tab} onchange={(id) => { tab = id; }}
      tabs={[{ id: 'list', label: t('first') }, { id: 'diff', label: diffText('example') }, ...(showTreeTab ? [{ id: 'tree', label: t('second'), closeLabel: t('closeTree') }] : [])]}
      onclose={() => { showTreeTab = false; tab = 'list'; }}>
      {#snippet panel(active)}
        {#if active === 'list'}
          <List label={t('list')} rowCount={100000} getRow={(index) => ({ id: String(index), label: `GitBranch${index}.txt` })}
            multi bind:selectedIds={selected} onactivate={(row) => { result = row.label; }} />
        {:else if active === 'diff'}
          <DiffView left={diffLeft} right={diffRight} result={diffExample} />
        {:else}
          <Tree label={t('tree')} {source} {expanded} onexpandedchange={(ids) => { expanded = ids; }} onactivate={(row) => { result = row.label; }} />
        {/if}
      {/snippet}
    </Tabs>
  </div>
  <output>{t('selected')}: {selected.size} / {result}</output>
  {#if popup}<Popup label={t('actions')} anchor={popup} onclose={() => { popup = null; }} items={[
    { id: 'group', label: t('group'), kind: 'heading' },
    { id: 'action', label: t('action'), run: () => { result = t('action'); } },
    { id: 'disabled', label: t('disabled'), disabled: true },
    { id: 'separator', label: '', kind: 'separator' },
    { id: 'more', label: t('submenu'), children: [{ id: 'preview', label: t('preview'), run: () => { preview = !preview; } }] },
  ]} />{/if}
</section>
<style>
  section { display: flex; flex-direction: column; gap: var(--gallery-gap, var(--space-inline)); padding: var(--space-panel-x); height: 100%; }
  .examples { height: var(--gallery-height, calc(var(--font-ui-size) * 30)); min-height: 0; }
  button { align-self: start; font: inherit; color: var(--text-primary); background: var(--background-secondary); border: var(--border-width) solid var(--separator); padding: var(--space-panel-y) var(--space-inline); }
</style>
