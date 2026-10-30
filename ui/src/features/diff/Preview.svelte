<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { blockSelection, toggleBlock, toggleRule, pathKey, type Hunks } from '../commit/model';
  import { commitText as c } from '../../i18n/commit';
  import { tick, untrack } from 'svelte';
  import { diffRequest } from './request';
  import DiffView from './DiffView.svelte';
  import type { Content, DiffResult } from './types';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import { browseText as b } from '../../i18n/browse';
  import { diffText as t } from '../../i18n/diff';
  let { browser }: { browser: RepositoryBrowser } = $props();
  interface Preview { left: Content; right: Content; result: DiffResult | null; leftLabel: string; rightLabel: string; conflicted: boolean; token?: string; selectable?: boolean }
  let value = $state<Preview | null>(null);
  let area = $state('');
  let rollbackDialog = $state<HTMLDialogElement>(null!); let pending = $state<Hunks | null>(null);
  const session = $derived(browser.commit);
  const chosen = $derived(browser.previewFile && value?.token ? blockSelection(session.selection,browser.previewFile.path,value.token,value.result?.blocks.length ?? 0,!browser.previewFile.staged && !browser.previewFile.unstaged) : []);
  async function confirmRollback(index: number) {
    if(!browser.previewFile || !value?.token || session.busy)return;
    pending={path:browser.previewFile.path,token:value.token,blocks:[index]}; await tick(); rollbackDialog.showModal();
  }
  async function rollback() {
    if(!pending)return;session.busy=true;session.task=c('revert');
    try {await invoke('rollback_hunk',{generation:browser.generation,hunk:pending});rollbackDialog.close();}
    catch(error){browser.report(error);}
    finally {session.busy=false; await browser.refresh({status:true,refs:true,history:true});}
  }
  let host: HTMLElement;
  let key = $state('');
  $effect(() => { browser.previewFile; area = ''; });
  $effect(() => {
    const file = browser.previewFile, generation = browser.generation;
    if (!file) { value = null; return; }
    if (!file.hash) browser.statusRevision;
    const selectedArea = area || 'commit';
    let alive = true;
    const nextKey = JSON.stringify([file.hash, file.path.bytes, selectedArea]);
    untrack(() => { if (key !== nextKey) value = null; key = nextKey; });
    const request = !file.hash && selectedArea === 'commit' ? {command:'selection_preview',args:{generation,path:file.path}} : diffRequest(file, generation, selectedArea);
    void invoke<Preview>(request.command, request.args)
      .then(result => { if (alive) value = result; }).catch(error => { if (alive) { value = null; browser.report(error); } });
    return () => { alive = false; };
  });
  function show(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'd' && browser.previewFile) {
      event.preventDefault(); host?.querySelector<HTMLElement>('.cm-content')?.focus();
    }
  }
</script>
<svelte:window onkeydown={show} />
<section bind:this={host}>
  {#if browser.previewFile}
    <header>{browser.previewFile.path.display}
      {#if !browser.previewFile.hash}
        <select aria-label={t('diff')} bind:value={area}><option value="">HEAD ↔ {b('changes')}</option><option value="staged" disabled={!browser.previewFile.staged}>{b('staged')}</option><option value="unstaged" disabled={!browser.previewFile.unstaged}>{b('unstaged')}</option></select>
      {/if}
    </header>
    {#if value}
      <header>{value.leftLabel} ↔ {value.rightLabel}</header>
      {#if value.conflicted}<p>{b('conflicted')}</p>
      {:else}<div class="body"><DiffView included={chosen} disabled={session.busy} onblock={value.token && !browser.previewFile.hash ? index=>{
          const file=browser.previewFile;if(!file || !value?.token || session.busy)return;
          if(value.selectable)session.selection=toggleBlock(session.selection,file.path,value.token,value.result?.blocks.length ?? 0,index);
          else session.selection=toggleRule(session.selection,`${file.staged || file.unstaged ? 'change' : 'untracked'}/${pathKey(file.path)}`,chosen.includes(index) ? 'all' : 'none');
        } : undefined}
        onrevert={value.selectable && !browser.previewFile.hash ? confirmRollback : undefined} fileKey={key} left={value.left} right={value.right} result={value.result ?? { blocks: [], alignment: [], folds: [], leftLines: 0, rightLines: 0 }} /></div>{/if}
    {:else}<p>{b('loading')}</p>{/if}
  {:else}<p>{b('diffPlaceholder')}</p>{/if}
</section>
{#if pending}<dialog bind:this={rollbackDialog} aria-labelledby="rollback-lines-title" oncancel={event=>{if(session.busy)event.preventDefault();}}><h2 id="rollback-lines-title">{c('linesQuestion')}</h2><p>{pending.path.display}</p><button disabled={session.busy} onclick={()=>rollbackDialog.close()}>{c('cancel')}</button><button disabled={session.busy} onclick={rollback}>{c('confirm')}</button></dialog>{/if}
<style>
section { height:100%; display:flex; flex-direction:column; min-height:0; }
header { padding:var(--space-panel-y) var(--space-panel-x); border-bottom:var(--border-width) solid var(--separator); overflow-wrap:anywhere; }
.body { flex:1; min-height:0; } p { padding:var(--space-panel-x); color:var(--text-secondary); }
dialog {color:var(--text-primary);background:var(--popup-bg);border:var(--border-width) solid var(--popup-border);}
</style>
