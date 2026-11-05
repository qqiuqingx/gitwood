<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import List from '../../components/List.svelte';
  import RefPicker from './RefPicker.svelte';
  import { gitText } from '../../i18n/git';
  import { integrationText as t } from '../../i18n/integration';
  import { emptyMergeOptions, integrationErrorText, mergeOptionEnabled, validMergeOptions, pullDefaults, type Integration, type MergeFlag } from './integration';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import type { RemoteError, RemoteOutcome } from './model';
  let { browser, kind, seed = '', busy = $bindable(false), progress = $bindable(''), onclose, onresult }:
    { browser: RepositoryBrowser; kind: Integration['kind']; seed?: string; busy?: boolean; progress: string; onclose: () => void; onresult: (value: RemoteOutcome) => void } = $props();
  let element: HTMLDialogElement;
  let localName = $state('');
  let target = $state(untrack(() => seed)), onto = $state(''), advanced = $state(false);
  let options = $state(emptyMergeOptions()), rebase = $state(false);
  let remote = $state(''), selected = $state<ReadonlySet<string>>(new Set());
  let error = $state('');
  const title = $derived(t(kind === 'checkoutRebase' ? 'checkoutRebase' : kind));
  const refs = $derived([...(browser.refs?.local ?? []), ...Object.values(browser.refs?.remotes ?? {}).flat()]);
  const choices = $derived((browser.refs?.remotes[remote] ?? []).map(ref => ({ id: ref.name.slice(`refs/remotes/${remote}/`.length), label: ref.name.slice(`refs/remotes/${remote}/`.length) })));
  const flags: { key: Exclude<MergeFlag, 'message'>; flag: string }[] = [{ key: 'noFf', flag: '--no-ff' }, { key: 'ffOnly', flag: '--ff-only' }, { key: 'squash', flag: '--squash' }, { key: 'noCommit', flag: '--no-commit' }];
  const valid = $derived(validMergeOptions(options) && (kind === 'pull' ? !!remote && selected.size > 0 && (!rebase || selected.size === 1) : !!target.trim()) && (!advanced || !!onto.trim()));
  onMount(() => {
    if (browser.refs) { const defaults = pullDefaults(browser.refs); remote = defaults.remote; selected = new Set(defaults.branches); }
    if (kind === 'merge' && !target && browser.refs) {
      const current = browser.refs.local.find(ref => ref.name === `refs/heads/${browser.refs?.head.head}`);
      target = refs.find(ref => ref.name === current?.upstream)?.name ?? refs.filter(ref => ref.name !== current?.name).sort((a,b) => a.name.localeCompare(b.name))[0]?.name ?? '';
    }
    element.showModal();
  });
  function request(): Integration {
    if (kind === 'merge') return { kind, target, options };
    if (kind === 'checkoutRebase') return { kind, branch: target, localName: localName.trim() || null };
    if (kind === 'rebase') return { kind, target, onto: advanced ? onto : null };
    return { kind, remote, branches: [...selected], rebase, options };
  }
  async function fetchBranches() {
    if (!remote || busy) return;
    busy = true; progress = ''; error = '';
    try {
      await invoke('fetch', { generation: browser.generation, expected: browser.status?.fingerprint, remote, all: false });
      await browser.refresh({ status: true, refs: true, history: true });
      if (!selected.size && browser.refs) { const defaults = pullDefaults(browser.refs); if (defaults.remote === remote) selected = new Set(defaults.branches); }
    } catch (cause) { error = integrationErrorText(cause as RemoteError); }
    finally { busy = false; }
  }
  async function submit() {
    if (!valid || busy) return;
    busy = true; progress = ''; error = '';
    try {
      const value = await invoke<RemoteOutcome>('integrate', { generation: browser.generation, expected: browser.status?.fingerprint, request: request() });
      await browser.refresh({ status: true, refs: true, history: true });
      onresult(value); onclose();
    } catch (cause) {
      error = integrationErrorText(cause as RemoteError);
      await browser.refresh({ status: true, refs: true, history: true });
    } finally { busy = false; }
  }
</script>
<dialog bind:this={element} aria-labelledby="integration-title" oncancel={event => { event.preventDefault(); if (!busy) onclose(); }}>
  <form onsubmit={event => { event.preventDefault(); void submit(); }}>
    <h2 id="integration-title">{title}</h2>
    <fieldset disabled={busy}>
      {#if kind === 'pull'}
        <label>{t('remote')}<select bind:value={remote} onchange={() => { selected = new Set(); }}>{#each Object.keys(browser.refs?.remotes ?? {}) as name}<option value={name}>{name}</option>{/each}</select></label><button type="button" disabled={!remote || busy} onclick={() => void fetchBranches()}>{gitText('fetch')}</button>
        <p>{t('branches')}</p><div class="branches"><List rowCount={choices.length} getRow={i => ({ ...choices[i], disabled: busy })} label={t('branches')} multi bind:selectedIds={selected} /></div>
        <label><input type="checkbox" bind:checked={rebase} disabled={!rebase && (selected.size !== 1 || flags.some(({ key }) => options[key]))} /> {t('rebaseRun')} (--rebase)</label>
        {#if selected.size > 1}<p class="hint">{t('multiRebase')}</p>{/if}
      {:else if kind === 'checkoutRebase'}
        <RefPicker bind:value={target} {refs} label={t('branch')} />
        {#if target.startsWith('refs/remotes/')}<label>{t('localBranch')}<input bind:value={localName} placeholder={target.replace(/^refs\/remotes\/[^/]+\//, '')} /></label>{/if}
        <p>{t('checkoutHint')}</p>
      {:else}
        <RefPicker bind:value={target} {refs} label={kind === 'rebase' ? (advanced ? t('oldBase') : t('target')) : t('branch')} editable={kind === 'rebase'} />
      {/if}
      {#if kind === 'merge' || kind === 'pull'}
        {#each flags as { key, flag }}<label><input type="checkbox" bind:checked={options[key]} disabled={!options[key] && !mergeOptionEnabled(options, key, kind === 'pull', rebase)} /> {flag} — {t(key)}</label>{/each}
        {#if kind === 'merge'}
          <label><input type="checkbox" checked={options.message !== null} disabled={options.message === null && !mergeOptionEnabled(options, 'message')} onchange={event => { options.message = event.currentTarget.checked ? '' : null; }} /> -m — {t('message')}</label>
          {#if options.message !== null}<label>{t('message')}<textarea bind:value={options.message} required></textarea></label>{/if}
        {/if}
        {#if options.noCommit}<p class="hint">{t('noCommitHint')}</p>{/if}
        {#if options.squash}<p class="hint">{t('squashHint')}</p>{/if}
      {:else if kind === 'rebase'}
        <label><input type="checkbox" checked={advanced} onchange={event => { advanced = event.currentTarget.checked; if (advanced) { onto = target; target = ''; } else { target = onto; onto = ''; } }} /> {t('onto')}</label>
        {#if advanced}<RefPicker bind:value={onto} {refs} label={t('newBase')} editable />{/if}
        <label><input type="checkbox" disabled /> {t('interactive')}</label>
      {/if}
    </fieldset>
    <p class="hint">{t('protect')}</p>
    {#if error}<p role="alert" class="error">{error}</p>{/if}
    {#if busy}<p role="status">{progress || t('working')} <progress></progress></p>{/if}
    <footer><button type="button" onclick={() => busy ? void invoke('cancel_write', { generation: browser.generation }) : onclose()}>{t('cancel')}</button><button class="default" disabled={busy || !valid} title={!valid ? t('invalid') : undefined}>{t(kind === 'pull' ? 'pullRun' : kind === 'merge' ? 'mergeRun' : 'rebaseRun')}</button></footer>
  </form>
</dialog>
<style>
  dialog { width: min(640px, 88vw); max-height: 88vh; overflow: auto; color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  h2 { font-size: calc(var(--font-ui-size) * 1.15); margin-top: 0; } fieldset { border: 0; padding: 0; min-width: 0; } label { display: block; margin-block: var(--space-panel-y); }
  select,textarea,button { font: inherit; } select,textarea { color: var(--input-fg); background: var(--input-bg); border: var(--border-width) solid var(--input-border); } textarea { width: 100%; min-height: calc(var(--font-ui-size) * 6); }
  footer { display: flex; justify-content: end; gap: var(--space-inline); margin-top: var(--space-panel-x); } button { padding: var(--space-panel-y) var(--space-panel-x); }
  .hint { color: var(--text-secondary); } .error { color: var(--status-conflicted); white-space: pre-wrap; overflow-wrap: anywhere; } .branches { height: calc(var(--list-row-height) * 6); min-height: 120px; border: var(--border-width) solid var(--separator); }
</style>
