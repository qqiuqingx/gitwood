<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import { stashText as t } from '../../i18n/stash';
  import { notifications } from '../notifications/center.svelte';
  let { browser, onclose }: { browser: RepositoryBrowser; onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  let message = $state('');
  let keepIndex = $state(false), includeUntracked = $state(false), busy = $state(false), failure = $state('');
  const hasChanges = $derived(!!browser.status && browser.status.changes + browser.status.untracked > 0);
  onMount(() => { dialog.showModal(); });
  async function create() {
    if (!hasChanges || !browser.status?.fingerprint) return;
    busy = true; failure = '';
    browser.commit.busy = true; browser.commit.task = t('stash');
    try {
      await invoke('stash_create', { generation: browser.generation, expected: browser.status.fingerprint, message, keepIndex, includeUntracked });
      dialog.close();
      notifications.notify({ kind: 'success', title: t('stash'), detail: message, group: 'git' });
      await browser.refresh({ status: true, refs: false, history: false });
    } catch (error) { failure = String((error as { detail?: string }).detail ?? error); }
    finally { busy = false; browser.commit.busy = false; }
  }
</script>
<dialog bind:this={dialog} onclose={onclose} aria-labelledby="stash-title" oncancel={event => { if (busy) event.preventDefault(); }}>
  <h2 id="stash-title">{t('stash')}</h2>
  <label>{t('message')} <textarea bind:value={message} disabled={busy}></textarea></label>
  <label><input type="checkbox" bind:checked={keepIndex} disabled={busy} />{t('keepIndex')}</label>
  <label><input type="checkbox" bind:checked={includeUntracked} disabled={busy} />{t('includeUntracked')}</label>
  {#if !hasChanges}<p>{t('noChanges')}</p>{/if}
  {#if failure}<pre role="alert">{failure}</pre>{/if}
  <footer><button disabled={busy} onclick={() => dialog.close()}>{t('cancel')}</button><button disabled={!hasChanges || busy} onclick={() => void create()}>{t('create')}</button></footer>
</dialog>
<style>
dialog { color:var(--text-primary); background:var(--popup-bg); border:var(--border-width) solid var(--popup-border); width:min(80vw,calc(var(--font-ui-size)*38)); } label { display:block; margin-block:var(--space-panel-y); } textarea { display:block; width:100%; min-height:calc(var(--list-row-height)*3); resize:vertical; font:inherit; color:var(--input-fg); background:var(--input-bg); border:var(--border-width) solid var(--input-border); } footer { display:flex; justify-content:end; gap:var(--space-inline); padding-top:var(--space-panel-x); } pre { white-space:pre-wrap; }
</style>
