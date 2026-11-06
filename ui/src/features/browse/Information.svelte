<script lang="ts">
  import { tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Splitter from '../../components/Splitter.svelte';
  import PagedFileTree from './PagedFileTree.svelte';
  import { projectName } from '../projects/avatar';
  import { detailsDateTime, refLabelsByHash } from './model';
  import type { RepositoryBrowser } from './browser.svelte';
  import { browseText as t } from '../../i18n/browse';
  import Popup from '../../components/Popup.svelte';
  import type { Rect } from '../../components/popup';
  import { currentFileItems, type CurrentFileAction } from '../git/current-file-menu';
  import { currentFileText } from '../../i18n/current-file';
  import { hostingMenuItems } from '../git/hosting-menu';
  import { hostingHandlers, publishedHostingRemotes } from '../git/hosting-actions';
  import type { DiffTabInput } from '../diff/tabs.svelte';
  import { createActionRegistry } from '../../keymap/registry';
  import { keymapFor } from '../../keymap/keymaps';
  import { detectPlatform } from '../../keymap/shortcuts';
  let { browser, ratio = $bindable(0.6), label, oncurrentfile = () => {} }: { browser: RepositoryBrowser; ratio?: number; label: string; oncurrentfile?: (action: CurrentFileAction, path: import('./types').RepoPath, revision?: string) => void } = $props();
  let branches = $state<{ branches: string[]; total: number } | null>(null);
  let expanded = $state(false);
  // IDEA expands every node of the commit changes tree for each newly selected commit.
  let files = $state<ReadonlySet<string>>(new Set(['*']));
  let context = $state<{ anchor: Rect; path: import('./types').RepoPath; revision: string } | null>(null);
  let selectedDiff = $state<DiffTabInput | null>(null);
  function diffInput(path: import('./types').RepoPath, hash: string): DiffTabInput {
    const commit = browser.rows.size ? [...browser.rows.values()].find(row => row.commit.hash === hash)?.commit : undefined;
    const details = browser.details?.hash === hash ? browser.details : undefined;
    const parent = (commit ?? details)?.parents[0] ?? 'Empty';
    return { source: 'commit', path, hash, revision: hash, leftRevision: parent, rightRevision: hash,
      staged: false, unstaged: false, area: 'commit' };
  }
  function selectFile(row: import('./PagedFileTree.svelte').FileRow) {
    if (!row.path || !browser.selected) return;
    const hash = browser.selected;
    browser.currentFile = { path: row.path, revision: hash, source: 'commit', versioned: true, untracked: false, conflicted: row.conflicted, regularFile: !row.hasChildren };
    selectedDiff = diffInput(row.path, hash);
    browser.previewFile = { path: row.path, staged: row.staged, unstaged: row.unstaged, hash };
    if (!row.conflicted) browser.diffTabs.preview(selectedDiff);
  }
  async function openFile(input: DiffTabInput) {
    browser.previewFile = { path: input.path, staged: false, unstaged: false, hash: input.hash };
    browser.diffTabs.open(input);
    await tick(); document.querySelector<HTMLElement>('.diff-view .cm-content')?.focus();
  }
  function openSelectedFile() {
    if (selectedDiff) void openFile(selectedDiff);
  }
  const actions = createActionRegistry();
  actions.register('Diff.ShowDiff', { run: openSelectedFile, enabled: () => !!selectedDiff });
  function keyboard(event: KeyboardEvent) {
    if (event.defaultPrevented || !(event.target instanceof HTMLElement) || !event.target.closest('.changed-files')) return;
    actions.dispatch(event, keymapFor(detectPlatform(navigator.platform)), event.target instanceof HTMLElement && !!event.target.closest('input,textarea,select'));
  }
  const hosted = hostingHandlers(() => browser);
  const contextItems = $derived.by(() => {
    const target = context;
    const enabled = {
      history: browser.nativeMenuActions.actions['Git.CurrentFile.ShowHistory'] === true,
      annotate: browser.nativeMenuActions.actions['Git.CurrentFile.Annotate'] === true,
      compareRevision: browser.nativeMenuActions.actions['Git.CurrentFile.CompareRevision'] === true,
      compareBranch: browser.nativeMenuActions.actions['Git.CurrentFile.CompareBranch'] === true,
    };
    return target ? [
      { id: 'show-diff', label: t('showDiffAction'), run: () => { void openFile(diffInput(target.path, target.revision)); context = null; } },
      { id: 'current-file', label: currentFileText('menu'), children: currentFileItems(action => oncurrentfile(action, target.path, target.revision), undefined, enabled, hostingMenuItems(publishedHostingRemotes(browser), { kind: 'file', revision: target.revision, path: target.path }, hosted.open, hosted.copy)) },
    ] : [];
  });
  $effect.pre(() => { void browser.selected; files = new Set(['*']); selectedDiff = null; if (browser.currentFile?.source === 'commit') browser.currentFile = null; });
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
<svelte:window onkeydown={keyboard} />
<Splitter axis="y" {label} minFirst={90} minSecond={90} bind:ratio>
  {#snippet first()}
    <div class="changed-files" role="group" aria-label={t('files')}><PagedFileTree onselect={selectFile} onopen={openSelectedFile} oncontextmenu={(event,row) => { if (!row.path || !browser.selected) return; event.preventDefault(); context = { anchor: { x: event.clientX, y: event.clientY, width: 0, height: 0 }, path: row.path, revision: browser.selected }; }} hash={browser.selected} root={browser.project ? projectName(browser.project.root) : ''} bind:expanded={files} generation={browser.generation} onerror={error => browser.report(error)} /></div>
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
{#if context}<Popup items={contextItems} label={currentFileText('menu')} anchor={context.anchor} onclose={() => { context = null; }} />{/if}
<style>
  .details { height: 100%; overflow: auto; padding: var(--space-panel-y) var(--space-panel-x); font-size: var(--font-small-size); }
  .changed-files { height: 100%; min-height: 0; }
  p { margin: 0 0 var(--space-inline); white-space: pre-wrap; overflow-wrap: anywhere; }
  .message { font-family: var(--font-editor-family); font-size: var(--font-ui-size); margin-block-end: calc(var(--space-inline) * 2); }
  .email { color: var(--accent); }
  .refs { display: flex; flex-wrap: wrap; gap: var(--space-inline); margin-block-end: 0; }
  .ref { display: inline-flex; align-items: center; gap: calc(var(--space-inline) / 2); }
  .ref svg { flex: none; }
  button { font: inherit; color: var(--accent); background: none; border: 0; padding: 0; cursor: pointer; }
</style>
