<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import Popup from '../../components/Popup.svelte';
  import PagedFileTree from '../browse/PagedFileTree.svelte';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import type { References } from '../browse/types';
  import {
    branchPopupItems, branchForPreview, commitPushFeedback, defaultPushOptions, defaultUpdateStrategy, isNewPushTarget, updateRequest,
    pushRejectionText, remoteErrorText, type BranchPopupHandlers, type CheckoutMode, type CommitPushResult,
    type PushOptions, type PushPreview, type RemoteError, type RemoteOutcome, type UpdateStrategy,
  } from './model';

  let { browser, oncommitpush }: { browser: RepositoryBrowser; oncommitpush: () => void } = $props();
  let branchAnchor = $state<HTMLButtonElement>();
  let popup = $state(false);
  let dialog = $state<'new' | 'rename' | 'delete' | 'checkout' | 'update' | 'push' | null>(null);
  let name = $state('');
  let newName = $state('');
  let start = $state('');
  let checkout = $state(true);
  let force = $state(false);
  let mode = $state<CheckoutMode>('normal');
  let updateStrategy = $state<UpdateStrategy>('merge');
  let updateStrategyLoading = $state(false);
  let pushOptions = $state<PushOptions>({ remote: null, branch: null, forceWithLease: false, tags: false });
  let preview = $state<PushPreview | null>(null);
  let previewSelection = $state('');
  let previewLoading = $state(false);
  let busy = $state(false);
  let progress = $state('');
  let error = $state<RemoteError | null>(null);
  let outcome = $state<RemoteOutcome | null>(null);
  let affectedFiles = $state<string[]>([]);
  let result = $state<CommitPushResult | null>(null);

  const current = $derived(browser.refs?.head.head ?? 'HEAD');
  const refs = $derived(browser.refs);
  const expected = () => browser.status?.fingerprint ?? '';
  const displayError = $derived(error ? remoteErrorText(error) : null);
  const newTarget = $derived(isNewPushTarget(refs, pushOptions));
  const handlers: BranchPopupHandlers = {
    checkout: (value) => { name = value; mode = 'normal'; dialog = 'checkout'; popup = false; },
    checkoutRemote: (remote, value) => { name = `${remote}/${value}`; mode = 'normal'; dialog = 'checkout'; popup = false; },
    newFrom: (value) => { start = value; name = ''; checkout = true; dialog = 'new'; popup = false; },
    update: () => { void openUpdate(); popup = false; },
    push: (options) => { openPush(options); popup = false; },
    rename: (value) => { name = value; newName = ''; dialog = 'rename'; popup = false; },
    delete: (value) => { name = value; force = false; dialog = 'delete'; popup = false; },
    newBranch: () => { name = ''; start = ''; checkout = true; dialog = 'new'; popup = false; },
    commit: () => { popup = false; document.querySelector<HTMLElement>('.commit-panel textarea')?.focus(); },
    fetch: () => { popup = false; void fetchRemote(); },
  };
  const branchItems = $derived(refs ? branchPopupItems(refs, handlers) : []);
  export function branchHandlers(): BranchPopupHandlers { return handlers; }

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<[number, string]>('remote_progress', ({ payload }) => {
      if (!disposed && payload[0] === browser.generation) progress = payload[1];
    }).then(value => { unlisten = value; if (disposed) value(); });
    return () => { disposed = true; unlisten?.(); };
  });

  function closeDialog() { if (!busy) { dialog = null; outcome = null; error = null; result = null; } }
  function openPush(options: PushOptions | null) {
    pushOptions = options ?? defaultPushOptions(browser.refs!);
    preview = null; previewSelection = ''; dialog = 'push'; outcome = null; error = null; void loadPreview();
  }
  async function loadPreview(startAt = 0) {
    if (!browser.project || !refs) return;
    previewLoading = true; error = null;
    try {
      const value = await invoke<PushPreview>('push_preview', { generation: browser.generation, options: pushOptions, start: startAt, limit: 200 });
      preview = startAt === 0 ? value : { ...value, commits: [...(preview?.commits ?? []), ...value.commits] };
      previewSelection ||= value.commits[0] ?? '';
    } catch (cause) { error = cause as RemoteError; }
    finally { previewLoading = false; }
  }
  async function write(command: string, args: Record<string, unknown>) {
    if (!browser.project || busy) return;
    busy = true; progress = ''; error = null; outcome = null;
    try {
      const value = await invoke<RemoteOutcome | { kind: string; files?: { display: string }[]; name?: string; detail?: string }>(command, { generation: browser.generation, expected: expected(), ...args });
      if (value.kind === 'notMerged') {
        force = true; dialog = 'delete'; error = { key: 'remote.failed', detail: `Branch ${value.name ?? name} is not fully merged. Confirm Force Delete to continue.` }; return;
      }
      if (value.kind === 'localChanges') {
        affectedFiles = value.files?.map(file => file.display) ?? []; dialog = 'checkout'; mode = 'smart'; error = { key: 'remote.failed', detail: `Checkout would overwrite ${affectedFiles.length} local file(s). Choose Smart Checkout or Force Checkout.` }; return;
      }
      if (value.kind === 'stashRecovery') {
        affectedFiles = value.files?.map(file => file.display) ?? []; const detail = 'detail' in value ? value.detail : undefined; error = { key: 'remote.failed', detail: `Working tree recovery is required${detail ? `: ${detail}` : '.'}` };
      }
      outcome = value as RemoteOutcome;
      await browser.refresh({ status: true, refs: true, history: true });
      if (outcome.kind === 'conflicts' || outcome.kind === 'stashRecovery' || outcome.kind === 'interrupted') error = outcome.kind === 'interrupted' ? outcome.error : { key: 'remote.failed', detail: outcome.kind === 'conflicts' ? 'Update stopped because files conflict.' : 'Working tree recovery is required.' };
    } catch (cause) { error = cause as RemoteError; }
    finally { busy = false; }
  }
  async function submitNew() { if (!name.trim() || !/^[^\s~^:?*\[\\]+$/.test(name.trim())) return; await write('create_branch', { name: name.trim(), start: start.trim() || null, checkout, }); }
  async function submitRename() { if (!newName.trim()) return; await write('rename_branch', { name, newName: newName.trim() }); }
  async function submitDelete() { await write('delete_branch', { name, force }); }
  async function submitCheckout() {
    const remote = name.includes('/') && !refs?.local.some(ref => ref.name === `refs/heads/${name}`) ? name.split('/')[0] : null;
    const branch = remote ? name.slice(remote.length + 1) : name;
    await write(remote ? 'checkout_remote_branch' : 'checkout_branch', remote ? { remote, branch, mode } : { name: branch, mode });
  }
  async function fetchRemote() { await write('fetch', { remote: null, all: true }); }
  async function openUpdate() {
    updateStrategyLoading = true;
    updateStrategy = 'merge';
    dialog = 'update';
    try {
      const configured = await invoke<UpdateStrategy>('default_update_strategy', { generation: browser.generation });
      updateStrategy = defaultUpdateStrategy(configured);
    } catch (cause) { error = cause as RemoteError; }
    finally { updateStrategyLoading = false; }
  }
  export function update() { void openUpdate(); }
  export function push() { openPush(null); }
  async function submitUpdate() { await write('update_project', updateRequest(updateStrategy)); }
  async function submitPush() {
    await write('push', { options: pushOptions });
  }
  export async function commitAndPush() {
    if (!browser.commit.message.trim() || !browser.commit.count) return;
    busy = true; progress = ''; error = null; result = null;
    try {
      result = await invoke<CommitPushResult>('commit_and_push', {
        generation: browser.generation, expected: expected(), selection: $state.snapshot(browser.commit.selection),
        message: browser.commit.message, amend: browser.commit.amend, options: defaultPushOptions(browser.refs!),
      });
      browser.commit.message = '';
      browser.commit.amend = false;
      await browser.refresh({ status: true, refs: true, history: true });
      if (result.push && result.push.kind === 'push' && !result.push.success) error = { key: 'remote.failed', detail: result.push.refs.filter(ref => ref.flag === '!').map(ref => pushRejectionText(ref.rejection)).join(' ') };
      if (result.push == null && result.kind === 'committed') oncommitpush();
    } catch (cause) { error = cause as RemoteError; }
    finally { busy = false; }
  }
  function chooseCommit(hash: string) { previewSelection = hash; }
  function outcomeText(value: RemoteOutcome | null) {
    if (!value) return '';
    if (value.kind === 'completed') return 'Completed.';
    if (value.kind === 'push') return value.success ? `Push completed (${value.refs.length} ref${value.refs.length === 1 ? '' : 's'}).` : 'Push was rejected; update first or inspect the remote rule.';
    if (value.kind === 'conflicts') return `Update stopped with ${value.files.length} conflicted file(s). Conflict resolution is planned for v0.2.`;
    if (value.kind === 'stashRecovery') return `Working tree recovery is required; stash ${value.stash} was retained.`;
    return 'Operation interrupted; inspect the retained stash before retrying.';
  }
  function outcomeStash(value: RemoteOutcome | null) { return value && 'stash' in value ? value.stash ?? null : null; }
</script>

<button class="branch-control" bind:this={branchAnchor} aria-expanded={popup} disabled={!browser.project} onclick={() => { popup = !popup; }}>{current} ↑{refs?.head.ahead ?? 0} ↓{refs?.head.behind ?? 0}⌄</button>
<span class="remote-actions">
  <button title="Update Project" disabled={!browser.project || busy} onclick={() => handlers.update()}>↓</button>
  <button title="Push" disabled={!browser.project || busy} onclick={() => openPush(null)}>↑</button>
  <button title="Fetch" disabled={!browser.project || busy} onclick={() => void fetchRemote()}>↻</button>
</span>
{#if popup && refs}<Popup items={branchItems} label="Git Branches" anchor={branchAnchor} onclose={() => { popup = false; }} />{/if}

{#if dialog}
  <dialog open class="workflow-dialog" aria-labelledby="workflow-title">
    {#if dialog === 'new'}
      <h2 id="workflow-title">New Branch</h2>
      <label>Name <input bind:value={name} aria-invalid={name.length > 0 && !/^[^\s~^:?*\[\\]+$/.test(name)} /></label>
      {#if name.length > 0 && !/^[^\s~^:?*\[\\]+$/.test(name)}<p class="invalid">Branch name contains invalid characters.</p>{/if}
      <label>Start point <input bind:value={start} placeholder={current} /></label>
      <label><input type="checkbox" bind:checked={checkout} /> Checkout branch</label>
      <footer><button onclick={closeDialog}>Cancel</button><button disabled={!name.trim() || !/^[^\s~^:?*\[\\]+$/.test(name)} onclick={() => void submitNew()}>Create</button></footer>
    {:else if dialog === 'rename'}
      <h2 id="workflow-title">Rename Branch</h2><p>{name}</p><label>New name <input bind:value={newName} /></label>
      <footer><button onclick={closeDialog}>Cancel</button><button disabled={!newName.trim()} onclick={() => void submitRename()}>Rename</button></footer>
    {:else if dialog === 'delete'}
      <h2 id="workflow-title">Delete Branch</h2><p>Delete <code>{name}</code>?</p><label><input type="checkbox" bind:checked={force} /> Force delete if not merged</label>
      <p class="hint">An unmerged branch requires the force option and a second confirmation in IDEA.</p>
      <footer><button onclick={closeDialog}>Cancel</button><button onclick={() => void submitDelete()}>Delete</button></footer>
    {:else if dialog === 'checkout'}
      <h2 id="workflow-title">Checkout {name}</h2><p>Choose how to handle local changes.</p>
      {#if affectedFiles.length}<ul>{#each affectedFiles as file}<li>{file}</li>{/each}</ul>{/if}
      <label><input type="radio" name="checkout" value="normal" bind:group={mode} /> Checkout</label>
      <label><input type="radio" name="checkout" value="smart" bind:group={mode} /> Smart Checkout (stash, checkout, restore)</label>
      <label><input type="radio" name="checkout" value="force" bind:group={mode} /> Force Checkout (discard local changes)</label>
      <footer><button onclick={closeDialog}>Cancel</button><button onclick={() => void submitCheckout()}>Checkout</button></footer>
    {:else if dialog === 'update'}
      <h2 id="workflow-title">Update Project</h2><p>Fetch and update the current branch.</p>
      <label><input type="radio" name="update" value="merge" bind:group={updateStrategy} /> Merge</label>
      <label><input type="radio" name="update" value="rebase" bind:group={updateStrategy} /> Rebase</label>
      <p class="hint">Default: {updateStrategy}.</p>
      <footer><button onclick={closeDialog}>Cancel</button><button disabled={updateStrategyLoading} onclick={() => void submitUpdate()}>Update</button></footer>
    {:else if dialog === 'push'}
      <h2 id="workflow-title">Push</h2>
      <div class="push-grid"><section><label>Remote <input bind:value={pushOptions.remote} placeholder="origin" /></label><label>Branch <input bind:value={pushOptions.branch} placeholder={branchForPreview(refs, pushOptions) ?? ''} /> {#if newTarget}<span class="new-target">New</span>{/if}</label>
        <label><input type="checkbox" bind:checked={pushOptions.forceWithLease} /> Force Push (with lease)</label><label><input type="checkbox" bind:checked={pushOptions.tags} /> Push Tags</label>
        <button onclick={() => void loadPreview()} disabled={previewLoading}>Preview</button><ol>{#each preview?.commits ?? [] as hash}<li><button class:selected={previewSelection === hash} onclick={() => chooseCommit(hash)}>{hash}</button></li>{/each}</ol>
        {#if preview && preview.total > preview.commits.length}<button onclick={() => void loadPreview(preview!.commits.length)}>Load next page</button>{/if}</section>
        <section class="push-detail"><strong>{previewSelection || 'Select a commit'}</strong>{#if previewSelection}<div class="push-files"><PagedFileTree hash={previewSelection} generation={browser.generation} onerror={cause => { error = cause as RemoteError; }} /></div>{:else}<p>Select a commit to view changed files.</p>{/if}</section></div>
      <footer><button onclick={closeDialog}>Cancel</button><button disabled={!pushOptions.remote && !pushOptions.branch} onclick={() => void submitPush()}>Push</button></footer>
    {/if}
    {#if busy}<p role="status">{progress || 'Working…'} <progress></progress><button onclick={() => void invoke('cancel_write', { generation: browser.generation })}>Cancel</button></p>{/if}
    {#if outcome}<p role="status">{outcomeText(outcome)}</p>{#if outcome.kind === 'conflicts' || outcome.kind === 'interrupted'}<button onclick={() => void write('abort_update', { stash: outcomeStash(outcome) })}>Abort Update</button>{/if}{/if}
    {#if displayError}<div class="error" role="alert"><strong>{displayError.title}</strong><p>{displayError.detail}</p><p>{displayError.action}</p><p class="hint">v0.1 does not provide a credential input dialog.</p></div>{/if}
    {#if result}<p role="status">Commit {result.hash}: {commitPushFeedback(result).push === 'success' ? 'commit and push completed.' : commitPushFeedback(result).push === 'recovery' ? 'commit completed; index recovery required, push was not started.' : commitPushFeedback(result).push === 'failed' ? 'commit completed, push failed.' : 'commit completed; push was not run.'}</p>{/if}
  </dialog>
{/if}

<style>
  .branch-control,.remote-actions button { border: 0; background: none; color: inherit; font: inherit; padding: var(--space-panel-y) var(--space-inline); }
  .branch-control:hover,.remote-actions button:hover { background: var(--toolbar-hover-bg); }
  .remote-actions { display: inline-flex; gap: 2px; margin-inline-start: auto; }
  .workflow-dialog { width: min(760px, 88vw); max-height: 86vh; overflow: auto; color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  .workflow-dialog::backdrop { background: #0008; }
  h2 { margin-block-start: 0; font-size: calc(var(--font-ui-size) * 1.15); }
  label { display: block; margin-block: var(--space-panel-y); } input:not([type='checkbox']):not([type='radio']) { display: block; width: 100%; font: inherit; color: var(--input-fg); background: var(--input-bg); border: var(--border-width) solid var(--input-border); padding: var(--space-panel-y); }
  footer { display: flex; justify-content: end; gap: var(--space-inline); margin-top: var(--space-panel-x); } footer button { font: inherit; padding: var(--space-panel-y) var(--space-panel-x); }
  .invalid,.error { color: var(--status-conflicted); } .hint { color: var(--text-secondary); font-size: var(--font-small-size); }
  .new-target { color: var(--accent); font-size: var(--font-small-size); }
  .push-grid { display: grid; grid-template-columns: minmax(250px, 1fr) minmax(220px, 1fr); gap: var(--space-panel-x); min-height: 260px; }
  ol { max-height: 32vh; overflow: auto; padding-inline-start: 26px; } ol button { width: 100%; text-align: start; border: 0; background: none; color: inherit; padding: 3px; } ol button.selected { background: var(--selection-bg); }
  .push-detail { border-inline-start: var(--border-width) solid var(--separator); padding-inline-start: var(--space-panel-x); }
  .push-files { height: 260px; min-height: 0; margin-top: var(--space-panel-y); border-top: var(--border-width) solid var(--separator); }
  code { font-family: var(--font-editor-family); }
  @media (max-width: 650px) { .push-grid { grid-template-columns: 1fr; } .push-detail { border-inline-start: 0; border-top: var(--border-width) solid var(--separator); padding: var(--space-panel-y) 0; } }
</style>
