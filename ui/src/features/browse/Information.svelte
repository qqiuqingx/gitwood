<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import Splitter from '../../components/Splitter.svelte';
  import PagedFileTree from './PagedFileTree.svelte';
  import { formatDate } from './model';
  import type { RepositoryBrowser } from './browser.svelte';
  import { browseText as t, prettyDateWords } from '../../i18n/browse';
  let { browser, ratio = $bindable(0.6), label }: { browser: RepositoryBrowser; ratio?: number; label: string } = $props();
  let branches = $state<{ branches: string[]; total: number } | null>(null);
  let expanded = $state(false);
  $effect(() => {
    const hash = browser.selected, generation = browser.generation;
    branches = null; expanded = false;
    let alive = true;
    if (hash) void invoke<{ branches: string[]; total: number }>('containing_branches', { hash, generation }).then(value => { if (alive) branches = value; }).catch(error => { if (alive) browser.report(error); });
    return () => { alive = false; };
  });
</script>
<Splitter axis="y" {label} minFirst={90} minSecond={90} bind:ratio>
  {#snippet first()}
    <PagedFileTree onselect={row => { if (row.path) browser.previewFile = { path: row.path, staged: row.staged, unstaged: row.unstaged, hash: browser.selected }; }} hash={browser.selected} generation={browser.generation} onerror={error => browser.report(error)} />
  {/snippet}
  {#snippet second()}
    <div class="details">
      {#if browser.details}
        {#if branches}<p>{#each ['local', 'remote'] as group}<span>{t(group as 'local' | 'remote')}: </span>{#each (expanded ? branches.branches : branches.branches.slice(0, 3)).filter(branch => branch.startsWith(group === 'local' ? 'refs/heads/' : 'refs/remotes/')) as branch}<code>{branch.replace(/^refs\/(heads|remotes)\//, '')} </code>{/each}{/each}{#if branches.total > 3 && !expanded}<button onclick={() => { expanded = true; }}>{t('andOthers')} {branches.total - 3}</button>{/if}{#if expanded && branches.total > branches.branches.length}<span>+{branches.total - branches.branches.length}</span>{/if}</p>{/if}
        <p class="message">{browser.details.message}</p>
        <p>{browser.details.author} &lt;{browser.details.email}&gt;</p>
        <p>{formatDate(browser.details.author_time, navigator.language, new Date(), prettyDateWords())}</p>
        <p><span>{t('hash')}: </span><code>{browser.details.hash}</code></p>
        {#if browser.details.parents.length}<p><span>{t('parents')}: </span>{#each browser.details.parents as parent}<code>{parent} </code>{/each}</p>{/if}
      {:else}<p>{browser.selected ? t('loading') : t('selectCommit')}</p>{/if}
    </div>
  {/snippet}
</Splitter>
<style>
  .details { height: 100%; overflow: auto; padding: var(--space-panel-y) var(--space-panel-x); font-size: var(--font-small-size); }
  p { margin: 0 0 var(--space-inline); white-space: pre-wrap; overflow-wrap: anywhere; }
  .message { font-size: var(--font-ui-size); }
  code { font-family: var(--font-editor-family); color: var(--accent); }
  span { color: var(--text-secondary); }
</style>
