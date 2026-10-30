<script lang="ts">
  import { createActionRegistry } from '../../keymap/registry';
  import { keymapFor } from '../../keymap/keymaps';
  import { detectPlatform } from '../../keymap/shortcuts';
  import { invoke } from '@tauri-apps/api/core';
  import { tick } from 'svelte';
  import { canCommit, amendMessage, toggleRule } from '../commit/model';
  import RollbackDialog from '../commit/RollbackDialog.svelte';
  import { commitText as c } from '../../i18n/commit';
  import Toolbar from '../../components/Toolbar.svelte';
  import Tabs from '../../components/Tabs.svelte';
  import PagedFileTree from './PagedFileTree.svelte';
  import type { RepositoryBrowser } from './browser.svelte';
  import { browseText as t } from '../../i18n/browse';
  let { browser, refresh, commitPush = () => {} }: { browser: RepositoryBrowser; refresh: () => void; commitPush?: () => void } = $props();
  let flat = $state(false);
  let ids = $state<string[]>([]), rollback = $state(false), history = $state(false);
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
    if(!canCommit(session.message,session.count,session.amend,session.busy))return;
    try {
      await session.submit(browser.generation,browser.status?.fingerprint ?? '');
    } catch(error) {browser.report(error);}
    finally {await browser.refresh({status:true,refs:true,history:true}); messageBox?.focus();}
  }
  async function showHistory() {
    if(session.busy)return;
    if(history){history=false;return;}
    try {const messages=await invoke<string[]>('commit_message_history',{generation:browser.generation});session.history=[...new Set([...session.history,...messages])].slice(0,25);history=true;await tick();historySelect?.focus();}
    catch(error){browser.report(error);}
  }
  const actions=createActionRegistry();
  actions.register('Vcs.ShowMessageHistory',{run:()=>void showHistory(),enabled:()=>!session.busy,allowInInput:true});
  actions.register('Vcs.ToggleAmendCommitMode',{run:()=>void amend(),enabled:()=>!session.busy && browser.status?.branch.oid !== '(initial)',allowInInput:true});
  actions.register('ChangesView.Revert',{run:()=>{rollback=true;},enabled:()=>!!ids.length && !session.busy});
  function keyboard(event: KeyboardEvent) {
    if(event.key==='Escape' && history){event.preventDefault();history=false;messageBox.focus();return;}
    actions.dispatch(event,keymapFor(detectPlatform(navigator.platform)),event.target instanceof HTMLElement && !!event.target.closest('input,textarea,select'));
  }
  async function openDiff() {await tick(); document.querySelector<HTMLElement>('.diff-view .cm-content')?.focus();}

  let expanded = $state<ReadonlySet<string>>(new Set(['changes', 'unversioned']));
</script>
<Tabs label={t('commit')} activeId="commit" tabs={[{ id: 'commit', label: t('commit') }, { id: 'shelf', label: t('shelf'), disabled: true }]} onchange={() => {}}>
  {#snippet panel()}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions (Commit scope shortcuts are delegated from its tree and message input.) -->
    <section class="commit-panel" onkeydown={keyboard} aria-label={t('commit')}>
      {#snippet refreshIcon()}⟳{/snippet}{#snippet groupIcon()}▱{/snippet}{#snippet expandIcon()}⌄{/snippet}{#snippet collapseIcon()}⌃{/snippet}{#snippet rollbackIcon()}↶{/snippet}
      <Toolbar label={t('changes')} items={[
        { id: 'rollback', label: c('rollback'), icon: rollbackIcon, disabled: !ids.length || session.busy, run: () => { rollback=true; } },
        { id: 'refresh', label: t('refresh'), icon: refreshIcon, run: refresh },
        { id: 'group', label: t('groupDirectories'), icon: groupIcon, pressed: !flat, run: () => { flat = !flat; } },
        { id: 'expand', label: t('expandAll'), icon: expandIcon, run: () => { expanded = new Set(['*']); } },
        { id: 'collapse', label: t('collapseAll'), icon: collapseIcon, run: () => { expanded = new Set(); } },
      ]} />
      <div class="files"><PagedFileTree selection={session.selection} oncount={count=>{session.count=count;}} onselectedids={selected=>{ids=selected;}} onopen={openDiff} ontoggle={(id,state)=>{if(!session.busy)session.selection=toggleRule(session.selection,id,state);}} onselect={row => { if (row.path) browser.previewFile = { path: row.path, staged: row.staged, unstaged: row.unstaged, hash: '' }; }} changes generation={browser.statusGeneration} revision={browser.statusRevision} {flat} bind:expanded onerror={error => browser.report(error)} /></div>
      <div class="form">
        <label><input type="checkbox" checked={session.amend} disabled={session.busy || browser.status?.branch.oid === '(initial)'} onclick={event=>{event.preventDefault();void amend();}} /> {t('amend')}</label>
        <button title="Ctrl+M" disabled={session.busy} onclick={()=>{void showHistory();}}>{c('history')}</button>
        {#if history}<select bind:this={historySelect} aria-label={c('history')} size={Math.max(2,Math.min(6,session.history.length))} onchange={event=>{session.message=event.currentTarget.value;history=false;messageBox.focus();}}><option disabled selected>{c('history')}</option>{#each session.history as message}<option value={message}>{message}</option>{/each}</select>{/if}
        <textarea bind:this={messageBox} aria-label={t('commitMessage')} placeholder={t('commitMessage')} bind:value={session.message} disabled={session.busy}></textarea>
        <div><button disabled={!canCommit(session.message,session.count,session.amend,session.busy)} onclick={commit}>{t('commit')}</button><button disabled={!canCommit(session.message,session.count,session.amend,session.busy)} onclick={commitPush} title={c('commitPush')}>{t('commitPush')}</button></div>
        {#if session.busy}<div role="status">{session.task}<progress aria-label={c('running')}></progress><button onclick={()=>void invoke('cancel_write',{generation:browser.generation}).catch(error=>browser.report(error))}>{c('cancel')}</button></div>{/if}
        {#if session.result?.kind === 'committed'}<p role="status">{c('committed')} {session.result.hash}</p>{/if}
        {#if session.result?.kind === 'recovery'}<div role="alert"><strong>{c('recovery')}</strong><p>{session.result.directory}</p><pre>{session.result.instructions}</pre></div>{/if}
      </div>
    </section>
  {/snippet}
</Tabs>
{#if rollback}<RollbackDialog {browser} {ids} close={()=>{rollback=false;}} />{/if}
<style>
  .commit-panel { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .files { flex: 1; min-height: 0; }
  .form { padding: var(--space-inline); display: flex; flex-direction: column; gap: var(--space-inline); border-top: var(--border-width) solid var(--separator); }
  .form > div { display: flex; gap: var(--space-inline); }
  label { color: var(--text-secondary); font-size: var(--font-small-size); }
  textarea { height: calc(var(--list-row-height) * 3); resize: none; }
  button, textarea { font: inherit; color: var(--text-primary); background: var(--input-bg); border: var(--border-width) solid var(--input-border); padding: var(--space-panel-y); }
  pre {white-space:pre-wrap;max-height:30vh;overflow:auto;}
</style>
