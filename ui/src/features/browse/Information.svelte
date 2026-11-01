<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import Splitter from '../../components/Splitter.svelte';
  import PagedFileTree from './PagedFileTree.svelte';
  import { projectName } from '../projects/avatar';
  import { detailsDateTime, refLabelsByHash } from './model';
  import type { RepositoryBrowser } from './browser.svelte';
  import { browseText as t } from '../../i18n/browse';
  let { browser, ratio = $bindable(0.6), label }: { browser: RepositoryBrowser; ratio?: number; label: string } = $props();
  let branches = $state<{ branches: string[]; total: number } | null>(null);
  let expanded = $state(false);
  // IDEA expands every node of the commit changes tree for each newly selected commit.
  let files = $state<ReadonlySet<string>>(new Set(['*']));
  $effect.pre(() => { void browser.selected; files = new Set(['*']); });
  const labels = $derived(browser.details ? refLabelsByHash(browser.refs).get(browser.details.hash) ?? [] : []);
  const when = $derived(browser.details ? detailsDateTime(browser.details.author_time) : null);
  const subject = $derived(browser.details ? browser.details.message.split('\n', 1)[0] : '');
  const body = $derived(browser.details ? browser.details.message.slice(subject.length).replace(/\s+$/, '') : '');
  const shownBranches = $derived(branches ? (expanded ? branches.branches : branches.branches.slice(0, 3)).map(branch => branch.replace(/^refs\/(heads|remotes)\//, '')) : []);
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
    <PagedFileTree onselect={row => { if (row.path) browser.previewFile = { path: row.path, staged: row.staged, unstaged: row.unstaged, hash: browser.selected }; }} hash={browser.selected} root={browser.project ? projectName(browser.project.root) : ''} bind:expanded={files} generation={browser.generation} onerror={error => browser.report(error)} />
  {/snippet}
  {#snippet second()}
    <div class="details">
      {#if browser.details}
        <p class="message"><strong>{subject}</strong>{body}</p>
        <p class="author"><span class="hash">{browser.details.hash.slice(0, 8)}</span> {browser.details.author} <span class="email">&lt;{browser.details.email}&gt;</span> {when ? t('onDateAt').replace('{date}', when.date).replace('{time}', when.time) : ''}</p>
        {#if labels.length}<p class="refs">{#each labels as label}<span class="ref"><svg style:color={`var(--label-${label.kind})`} viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M2 3v4.6l6.4 6.4 5.6-5.6L7.6 2H3a1 1 0 0 0-1 1Z" fill="none" stroke="currentColor" /><circle cx="5" cy="5" r="1" fill="currentColor" /></svg>{label.name}</span>{/each}</p>{/if}
        {#if branches && branches.total}<p class="branches">{t(branches.total === 1 ? 'inOneBranch' : 'inBranches').replace('{count}', String(branches.total))} {shownBranches.join(', ')}{#if branches.total > shownBranches.length}{@const more = t('andOthers').replace('{count}', String(branches.total - shownBranches.length))} {#if expanded}{more}{:else}<button onclick={() => { expanded = true; }}>{more}</button>{/if}{/if}</p>{/if}
      {:else}<p>{browser.selected ? t('loading') : t('selectCommit')}</p>{/if}
    </div>
  {/snippet}
</Splitter>
<style>
  .details { height: 100%; overflow: auto; padding: var(--space-panel-y) var(--space-panel-x); font-size: var(--font-small-size); }
  p { margin: 0 0 var(--space-inline); white-space: pre-wrap; overflow-wrap: anywhere; }
  .message { font-family: var(--font-editor-family); font-size: var(--font-ui-size); margin-block-end: calc(var(--space-inline) * 2); }
  .email { color: var(--accent); }
  .refs { display: flex; flex-wrap: wrap; gap: var(--space-inline); margin-block-end: 0; }
  .ref { display: inline-flex; align-items: center; gap: calc(var(--space-inline) / 2); }
  .ref svg { flex: none; }
  button { font: inherit; color: var(--accent); background: none; border: 0; padding: 0; cursor: pointer; }
</style>
