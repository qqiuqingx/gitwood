<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import RefPicker from './RefPicker.svelte';
  import { integrationText as t } from '../../i18n/integration';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import type { RemoteError } from './model';
  let { browser, name, busy = $bindable(false), onclose }: { browser: RepositoryBrowser; name: string; busy?: boolean; onclose: () => void } = $props();
  let element: HTMLDialogElement;
  let upstream = $state(''), error = $state('');
  const refs = $derived([...(browser.refs?.local ?? []), ...Object.values(browser.refs?.remotes ?? {}).flat()].filter(ref => ref.name !== `refs/heads/${name}`));
  onMount(() => { upstream = browser.refs?.local.find(ref => ref.name === `refs/heads/${name}`)?.upstream ?? ''; element.showModal(); });
  async function save() {
    if (!upstream || busy) return;
    busy = true; error = '';
    try { await invoke('set_upstream', { generation: browser.generation, expected: browser.status?.fingerprint, name, upstream }); await browser.refresh({ status: true, refs: true, history: false }); onclose(); }
    catch (cause) { error = (cause as RemoteError).detail || String(cause); }
    finally { busy = false; }
  }
</script>
<dialog bind:this={element} aria-labelledby="upstream-title" oncancel={event => { event.preventDefault(); if (!busy) onclose(); }}>
  <form onsubmit={event => { event.preventDefault(); void save(); }}><h2 id="upstream-title">{t('setUpstream')}</h2><p>{name}</p>
    <RefPicker bind:value={upstream} {refs} label={t('upstream')} disabled={busy} />
    {#if error}<p role="alert">{error}</p>{/if}
    <footer><button type="button" disabled={busy} onclick={onclose}>{t('cancel')}</button><button disabled={!upstream || busy}>{t('set')}</button></footer>
  </form>
</dialog>
<style>
  dialog { width: min(560px, 88vw); color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  h2 { font-size: calc(var(--font-ui-size) * 1.15); margin-top: 0; } footer { display: flex; justify-content: end; gap: var(--space-inline); margin-top: var(--space-panel-x); } button { font: inherit; padding: var(--space-panel-y) var(--space-panel-x); } [role='alert'] { color: var(--status-conflicted); white-space: pre-wrap; }
</style>
