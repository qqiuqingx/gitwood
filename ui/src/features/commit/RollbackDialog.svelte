<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import List from '../../components/List.svelte';
  import type { RepoPath } from '../browse/types';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import { commitText as t } from '../../i18n/commit';
  let { browser, ids, close }: { browser: RepositoryBrowser; ids: string[]; close: () => void } = $props();
  let dialog: HTMLDialogElement; let deleteAdded = $state(false), total = $state(0), fingerprint=$state('');
  let paths=$state.raw(new Map<number, RepoPath>()), loading=$state(true), failure=$state('');
  let ticket=0;
  async function load(start=0,end=40) {
    const current=++ticket;
    try {
      const page=await invoke<{ paths: RepoPath[]; start:number; total:number; fingerprint:string }>('rollback_files',{generation:browser.generation,ids,start,end:Math.min(end,start+200)});
      if(current!==ticket)return;
      if(fingerprint && fingerprint!==page.fingerprint) { failure=t('stale'); return; }
      fingerprint=page.fingerprint; total=page.total;
      paths=new Map(page.paths.map((p,i)=>[page.start+i,p])); loading=false;
    } catch(error) { failure=String((error as {detail?:string}).detail ?? error); loading=false; }
  }
  onMount(()=> { const previous=document.activeElement; dialog.showModal(); void load(); return()=>{ ticket++; if(previous instanceof HTMLElement)previous.focus(); }; });
  async function rollback() {
    browser.commit.busy=true; browser.commit.task=t('rollback'); failure='';
    try { await invoke('rollback',{generation:browser.generation,ids,deleteAdded,expected:fingerprint}); close(); }
    catch(error) { failure=String((error as {detail?:string}).detail ?? error); }
    finally { browser.commit.busy=false; await browser.refresh({status:true,refs:true,history:true}); }
  }
</script>
<dialog bind:this={dialog} onclose={close} oncancel={event=>{if(browser.commit.busy)event.preventDefault();}} aria-labelledby="rollback-title">
  <h2 id="rollback-title">{t('rollback')}</h2><p>{t('rollbackQuestion')}</p>
  <div class="files"><List label={t('rollback')} rowCount={total} getRow={i=>({id:String(i),label:paths.get(i)?.display ?? '…'})} onrangechange={r=>{if(r.end>r.start)void load(r.start,r.end);}} search={false} /></div>
  <label><input type="checkbox" bind:checked={deleteAdded} disabled={browser.commit.busy} />{t('deleteAdded')}</label>
  {#if failure}<pre role="alert">{failure}</pre>{/if}
  {#if browser.commit.busy}<progress aria-label={t('running')}></progress><button onclick={()=>void invoke('cancel_write',{generation:browser.generation})}>{t('cancel')}</button>{/if}
  <footer><button disabled={browser.commit.busy} onclick={()=>dialog.close()}>{t('cancel')}</button><button disabled={loading || !!failure || !total || browser.commit.busy} onclick={rollback}>{t('confirm')}</button></footer>
</dialog>
<style>
dialog { color:var(--text-primary); background:var(--popup-bg); border:var(--border-width) solid var(--popup-border); width: min(80vw, calc(var(--font-ui-size) * 48)); }
.files { height:calc(var(--list-row-height) * 8); } footer { display:flex; justify-content:end; gap:var(--space-inline); padding-top:var(--space-panel-x); } pre { white-space:pre-wrap; max-height:30vh; overflow:auto; } button {font:inherit;color:var(--button-fg);background:var(--button-bg);border:var(--border-width) solid var(--button-border);}
</style>
