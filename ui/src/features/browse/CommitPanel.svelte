<script lang="ts">
  import type { RemoteOutcome } from '../git/model';
  import { conflictText } from '../../i18n/conflict';
  import { notifications } from '../notifications/center.svelte';
  import { requestResolution } from '../conflicts/model';
  import { createActionRegistry } from '../../keymap/registry';
  import { keymapFor } from '../../keymap/keymaps';
  import { detectPlatform } from '../../keymap/shortcuts';
  import { invoke } from '@tauri-apps/api/core';
  import { tick } from 'svelte';
  import { canCommit, amendMessage, toggleRule, includeCommitFile } from '../commit/model';
  import RollbackDialog from '../commit/RollbackDialog.svelte';
  import DeleteChangesDialog from '../commit/DeleteChangesDialog.svelte';
  import { commitText as c } from '../../i18n/commit';
  import Popup from '../../components/Popup.svelte';
  import type { FileRow } from './PagedFileTree.svelte';
  import type { PopupItem, Rect } from '../../components/popup';
  import Toolbar from '../../components/Toolbar.svelte';
  import Tabs from '../../components/Tabs.svelte';
  import PagedFileTree from './PagedFileTree.svelte';
  import type { RepositoryBrowser } from './browser.svelte';
  import { currentFileItems, type CurrentFileAction } from '../git/current-file-menu';
  import { currentFileText } from '../../i18n/current-file';
  import { browseText as t } from '../../i18n/browse';
  let { browser, refresh, commitPush = () => {}, onfocuscommit = () => {}, oncurrentfile = () => {}, flat = $bindable(true), preview = $bindable(true) }: { browser: RepositoryBrowser; refresh: () => void; commitPush?: () => void; onfocuscommit?: () => void | Promise<void>; oncurrentfile?: (action: CurrentFileAction, path: import('./types').RepoPath, revision?: string) => void; flat?: boolean; preview?: boolean } = $props();
  let selectedFile = $state<FileRow>();
  let optionsAnchor = $state<HTMLButtonElement | null>(null);
  function previewFile(row: FileRow) {
    if (row.conflicted) return;
    if (row.path) browser.previewFile = { path: row.path, staged: row.staged, unstaged: row.unstaged, hash: '' };
  }
  function selectFile(row: FileRow) {
    selectedFile = row;
    contextRow = row;
    browser.currentFile = row.path ? {
      path: row.path,
      revision: 'HEAD',
      source: 'changes',
      versioned: row.status !== '?',
      untracked: row.status === '?',
      conflicted: row.conflicted,
    } : null;
    if (preview) previewFile(row);
  }
  function togglePreview() {
    preview = !preview;
    if (preview && selectedFile) previewFile(selectedFile);
  }
  let ids = $state<string[]>([]), rollback = $state(false), history = $state(false);
  let contextAnchor = $state<Rect | null>(null), contextRow = $state<FileRow | null>(null), deleteRow = $state<FileRow | null>(null);
  let messageBox: HTMLTextAreaElement; let historySelect = $state<HTMLSelectElement>(null!);
  const session = $derived(browser.commit);
  async function amend() {
    const enabled=!session.amend; session.busy=true;
    try {
      const last=enabled ? await invoke<string>('last_commit_message',{generation:browser.generation}) : '';
      const next=amendMessage(session.message,session.draft,enabled,last);
      session.message=next.message; session.draft=next.draft; session.amend=enabled;
    } catch(error) {browser.report(error);} finally {session.busy=false;}
  }
  async function commit() {
    if (!ready) return;
    try {
      if (browser.status?.update_stash && browser.status.operation) {
        session.busy = true;
        const value = await invoke<RemoteOutcome>('continue_update', { generation: browser.generation, expected: browser.status.fingerprint, stash: browser.status.update_stash, action: 'continue', message: session.message });
        if (value.kind === 'completed' || value.kind === 'stashRecovery') session.message = '';
        else requestResolution();
        const detail = value.kind === 'stashRecovery' ? `${value.stash}\n${value.detail ?? ''}` : value.kind === 'interrupted' ? value.error.detail : '';
        notifications.notify({ kind: value.kind === 'completed' ? 'success' : 'warning', title: value.kind === 'completed' ? conflictText('updateCompleted') : conflictText('title'), detail, group: 'git' });
      } else await session.submit(browser.generation,browser.status?.fingerprint ?? '');
    } catch(error) {browser.report(error);}
    finally {session.busy = false; await browser.refresh({status:true,refs:true,history:true}); messageBox?.focus();}
  }
  async function showHistory() {
    if(session.busy)return;
    if(history){history=false;return;}
    try {const messages=await invoke<string[]>('commit_message_history',{generation:browser.generation});session.history=[...new Set([...session.history,...messages])].slice(0,25);history=true;await tick();historySelect?.focus();}
    catch(error){browser.report(error);}
  }
  const actions=createActionRegistry();
  const menuEnabled = (id: string) => browser.nativeMenuActions.actions[id] === true;
  export function runAction(id: string) { return actions.run(id); }
  actions.register('CheckinProject', { run: () => { void Promise.resolve(onfocuscommit()).then(() => tick()).then(() => messageBox?.focus()).catch(error => browser.report(error)); }, enabled: () => browser.nativeMenuActions.actions.CheckinProject === true });
  actions.register('Vcs.ShowMessageHistory',{run:()=>void showHistory(),enabled:()=>!session.busy,allowInInput:true});
  actions.register('Vcs.ToggleAmendCommitMode',{run:()=>void amend(),enabled:()=>!session.busy && !browser.status?.operation && browser.status?.branch.oid !== '(initial)',allowInInput:true});
  actions.register('ChangesView.Revert',{run:()=>{rollback=true;},enabled:()=>menuEnabled('ChangesView.Revert')});
  actions.register('Diff.ShowDiff', { run: () => { if (contextRow) { selectFile(contextRow); void openDiff(); } }, enabled: () => !!contextRow?.path && !session.busy });
  actions.register('ChangesView.CommitFile', { run: () => { if (!contextRow?.path) return; session.selection = includeCommitFile(session.selection, contextRow.id, contextRow.path); void tick().then(() => messageBox?.focus()); }, enabled: () => menuEnabled('ChangesView.CommitFile') });
  actions.register('CheckinFiles', { run: () => { actions.run('ChangesView.CommitFile'); }, enabled: () => menuEnabled('ChangesView.CommitFile') });
  actions.register('Git.Add', { run: () => { actions.run('ChangesView.AddUnversioned'); }, enabled: () => menuEnabled('ChangesView.AddUnversioned') });
  actions.register('ChangesView.AddUnversioned', { run: () => { if (contextRow?.path && browser.status?.fingerprint) void invoke('changes_add', { generation: browser.generation, paths: [contextRow.path], expected: browser.status.fingerprint }).then(() => browser.refresh({ status:true, refs:false, history:false })).catch(error => browser.report(error)); }, enabled: () => menuEnabled('ChangesView.AddUnversioned') });
  actions.register('ChangesView.Delete', { run: () => { if (contextRow?.path) deleteRow = contextRow; }, enabled: () => !!contextRow?.path && !session.busy });
  actions.register('ChangesView.Refresh', { run: () => refresh(), enabled: () => !session.busy });
  function keyboard(event: KeyboardEvent) {
    if(event.key==='Escape' && history){event.preventDefault();history=false;messageBox.focus();return;}
    actions.dispatch(event,keymapFor(detectPlatform(navigator.platform)),event.target instanceof HTMLElement && !!event.target.closest('input,textarea,select'));
  }
  async function openDiff() {if(selectedFile?.conflicted) { requestResolution(); return; } if(selectedFile)previewFile(selectedFile);await tick(); document.querySelector<HTMLElement>('.diff-view .cm-content')?.focus();}
  function contextMenu(event: MouseEvent, row: FileRow) {
    if (!row.path) return;
    event.preventDefault(); selectFile(row);
    contextAnchor = { x: event.clientX, y: event.clientY, width: 0, height: 0 };
  }
  const contextActions = $derived.by((): PopupItem[] => {
    const row = contextRow;
    const canFile = !!row?.path && !row.conflicted;
    const canCommitFile = menuEnabled('ChangesView.CommitFile');
    const canRollback = menuEnabled('ChangesView.Revert');
    const canAdd = menuEnabled('ChangesView.AddUnversioned');
    const currentFileAvailability = {
      history: menuEnabled('Git.CurrentFile.ShowHistory'),
      annotate: menuEnabled('Git.CurrentFile.Annotate'),
      compareRevision: menuEnabled('Git.CurrentFile.CompareRevision'),
      compareBranch: menuEnabled('Git.CurrentFile.CompareBranch'),
    };
    const shortcut = detectPlatform(navigator.platform) === 'mac' ? { rollback: '⌥⌘Z', diff: '⌘D', add: '⌥⌘A' } : { rollback: 'Ctrl+Alt+Z', diff: 'Ctrl+D', add: 'Ctrl+Alt+A' };
    const commit: PopupItem = { id:'context-commit', label:t('commitFile'), disabled:!canCommitFile || !canFile, run:()=>actions.run('ChangesView.CommitFile') };
    const revert: PopupItem = { id:'context-rollback', label:c('rollback'), shortcut:shortcut.rollback, disabled:!canRollback || !canFile, run:()=>{ ids=[row!.id]; actions.run('ChangesView.Revert'); } };
    const add: PopupItem = { id:'context-add', label:t('addToVcs'), shortcut:shortcut.add, disabled:!canAdd, run:()=>actions.run('ChangesView.AddUnversioned') };
    return [commit, revert, { id:'context-diff', label:t('showDiffAction'), shortcut:shortcut.diff, disabled:!canFile || session.busy, run:()=>actions.run('Diff.ShowDiff') },
      { id:'context-delete', label:t('delete'), disabled:!canFile || session.busy, run:()=>actions.run('ChangesView.Delete') }, add,
      { id:'context-refresh', label:t('refresh'), disabled:session.busy, run:()=>actions.run('ChangesView.Refresh') },
      { id:'context-separator', label:'', kind:'separator' },
      { id:'context-git', label:t('gitSubmenu'), children:[commit, revert, add, { id:'context-current-file', label:currentFileText('menu'), children:currentFileItems(action => { if (row?.path) oncurrentfile(action, row.path); }, undefined, currentFileAvailability) }] }];
  });

  const ready = $derived(canCommit(session.message,session.count + (browser.status?.operation?.kind === 'merge' ? 1 : 0),session.amend,session.busy || !!browser.status?.conflicts || ['rebase','applyMailbox'].includes(browser.status?.operation?.kind ?? '')));
  let messageOperation = '';
  $effect(() => {
    const status = browser.status;
    const key = status?.operation ? `${status.operation.kind}:${status.branch.oid}` : '';
    if (key && key !== messageOperation && status?.merge_message && !session.message) {
      session.message = status.merge_message;
      session.selection = { rules: [{ id: 'changes', included: true }], hunks: [] };
    }
    messageOperation = key;
  });
  let expanded = $state<ReadonlySet<string>>(new Set(['conflicts', 'changes', 'unversioned']));
</script>
<Tabs label={t('commit')} activeId="commit" tabs={[{ id: 'commit', label: t('commit') }]} onchange={() => {}}>
  {#snippet panel()}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions (Commit scope shortcuts are delegated from its tree and message input.) -->
    <section class="commit-panel" onkeydown={keyboard} aria-label={t('commit')}>
      {#snippet refreshIcon()}⟳{/snippet}{#snippet optionsIcon()}⚙{/snippet}{#snippet rollbackIcon()}↶{/snippet}
      {#snippet expandIcon()}<svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" aria-hidden="true"><path d="m5 5 3-3 3 3M5 11l3 3 3-3" /></svg>{/snippet}
      {#snippet collapseIcon()}<svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" aria-hidden="true"><path d="m5 2 3 3 3-3M5 14l3-3 3 3" /></svg>{/snippet}
      {#snippet previewIcon()}<svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" aria-hidden="true"><path d="M1 8s2.5-5 7-5 7 5 7 5-2.5 5-7 5-7-5-7-5Z" /><circle cx="8" cy="8" r="2" /></svg>{/snippet}
      <Toolbar label={t('changes')} items={[
        { id: 'refresh', label: t('refresh'), icon: refreshIcon, run: refresh },
        { id: 'rollback', label: c('rollback'), icon: rollbackIcon, disabled: !menuEnabled('ChangesView.Revert'), run: () => { rollback=true; } },
        { id: 'preview', label: t('showPreview'), icon: previewIcon, pressed: preview, run: togglePreview },
        { id: 'expand', label: t('expandAll'), icon: expandIcon, run: () => { expanded = new Set(['*']); } },
        { id: 'collapse', label: t('collapseAll'), icon: collapseIcon, run: () => { expanded = new Set(); } },
        { id: 'options', label: t('viewOptions'), icon: optionsIcon, popup: true, expanded: !!optionsAnchor, run: anchor => { optionsAnchor = optionsAnchor ? null : anchor; } },
      ]} />
      {#if optionsAnchor}<div class="view-options"><Popup label={t('viewOptions')} anchor={optionsAnchor} onclose={() => { optionsAnchor = null; }} items={[
        { id: 'group', label: t('groupDirectories'), checked: !flat, run: () => { flat = !flat; } },
      ]} /></div>{/if}
      <div class="files"><PagedFileTree selection={session.selection} oncount={count=>{session.count=count;}} onselectedids={selected=>{ids=selected;browser.selectedChangeCount=selected.length;if(!selected.length && browser.currentFile?.source==='changes')browser.currentFile=null;}} onopen={openDiff} ontoggle={(id,state)=>{if(!session.busy)session.selection=toggleRule(session.selection,id,state);}} onselect={selectFile} oncontextmenu={contextMenu} changes generation={browser.statusGeneration} revision={browser.statusRevision} {flat} bind:expanded onerror={error => browser.report(error)} /></div>
      <div class="form">
        <label><input type="checkbox" checked={session.amend} disabled={session.busy || (!!browser.status?.operation || browser.status?.branch.oid === '(initial)')} onchange={event=>{const box=event.currentTarget;void amend().finally(()=>{box.checked=session.amend;});}} /> {t('amend')}</label>
        <button title="Ctrl+M" disabled={session.busy} onclick={()=>{void showHistory();}}>{c('history')}</button>
        {#if history}<select bind:this={historySelect} aria-label={c('history')} size={Math.max(2,Math.min(6,session.history.length))} onchange={event=>{session.message=event.currentTarget.value;history=false;messageBox.focus();}}><option disabled selected>{c('history')}</option>{#each session.history as message}<option value={message}>{message}</option>{/each}</select>{/if}
        <textarea bind:this={messageBox} aria-label={t('commitMessage')} placeholder={t('commitMessage')} bind:value={session.message} disabled={session.busy}></textarea>
        <div><button disabled={!ready} onclick={commit}>{t('commit')}</button><button disabled={!ready} onclick={commitPush} title={c('commitPush')}>{t('commitPush')}</button></div>
        {#if session.busy}<div role="status">{session.task}<progress aria-label={c('running')}></progress><button onclick={()=>void invoke('cancel_write',{generation:browser.generation}).catch(error=>browser.report(error))}>{c('cancel')}</button></div>{/if}
        {#if session.result?.kind === 'committed'}<p role="status">{c('committed')} {session.result.hash}</p>{/if}
        {#if session.result?.kind === 'recovery'}<div role="alert"><strong>{c('recovery')}</strong><p>{session.result.directory}</p><pre>{session.result.instructions}</pre></div>{/if}
      {#if contextAnchor}<div class="context-popup"><Popup label={t('changes')} anchor={contextAnchor} items={contextActions} onclose={()=>{contextAnchor=null;}} /></div>{/if}
      </div>
    </section>
  {/snippet}
</Tabs>
{#if rollback}<RollbackDialog {browser} {ids} close={()=>{rollback=false;}} />{/if}
{#if deleteRow?.path}<DeleteChangesDialog {browser} paths={[deleteRow.path]} expected={browser.status?.fingerprint ?? ''} close={()=>{deleteRow=null;}} />{/if}
<style>
  .commit-panel { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .files { flex: 1; min-height: 0; --list-row-height: var(--commit-list-row-height); --space-inline: var(--commit-tree-gap); --tree-indent: var(--commit-tree-indent); }
  .commit-panel :global(.toolbar) { gap: 0; padding: var(--commit-tree-gap); }
  .view-options { --popup-height: calc(var(--list-row-height) + var(--popup-padding) * 2 + var(--border-width) * 2); }
  .form { padding: var(--space-inline); display: flex; flex-direction: column; gap: var(--space-inline); border-top: var(--border-width) solid var(--separator); }
  .form > div { display: flex; gap: var(--space-inline); }
  label { color: var(--text-secondary); font-size: var(--font-small-size); }
  textarea { height: calc(var(--list-row-height) * 3); resize: none; }
  button, textarea { font: inherit; color: var(--text-primary); background: var(--input-bg); border: var(--border-width) solid var(--input-border); padding: var(--space-panel-y); }
  pre {white-space:pre-wrap;max-height:30vh;overflow:auto;}
</style>
