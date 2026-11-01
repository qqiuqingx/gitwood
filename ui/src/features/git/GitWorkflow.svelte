<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { listenWindowEvent } from '../../windowEvents';
  import { invoke } from '@tauri-apps/api/core';
  import Popup from '../../components/Popup.svelte';
  import PagedFileTree from '../browse/PagedFileTree.svelte';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import type { Reference, References } from '../browse/types';
  import { ideaIcon } from '../../components/icons/idea';
  import { appearance } from '../../theme/appearance.svelte';
  import { gitText } from '../../i18n/git';
  import {
    branchFolderPrefix, branchSectionPrefix, defaultFavorite, trackCounts, branchPopupItems, branchForPreview, commitPushFeedback, defaultPushOptions, defaultUpdateStrategy, isNewPushTarget, updateRequest,
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
  let workflowDialog = $state<HTMLDialogElement>();
  let editingTarget = $state(false);
  let pushMenu = $state<HTMLElement | null>(null);
  const projectName = $derived(browser.project?.root.split(/[\\/]/).filter(Boolean).pop() ?? '');
  // Modal so the dialog is centred above the window instead of anchored to the title bar.
  $effect(() => {
    const element = workflowDialog;
    if (element && dialog && !element.open) element.showModal();
  });

  const current = $derived(browser.refs?.head.head ?? 'HEAD');
  const refs = $derived(browser.refs);
  const ahead = $derived(refs?.head.ahead ?? 0);
  const behind = $derived(refs?.head.behind ?? 0);
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
  // Folder expansion lives for this window only, like IDEA's branch popup tree state.
  let branchFolders = $state<ReadonlySet<string>>(new Set());
  function toggleFolder(id: string) {
    const next = new Set(branchFolders);
    if (!next.delete(id)) next.add(id);
    branchFolders = next;
  }
  const branchRefs = $derived(new Map(refs ? [...refs.local, ...Object.values(refs.remotes).flat(), ...refs.tags].map(ref => [ref.name, ref] as const) : []));
  const branchRef = (id: string) => branchRefs.get(id.replace(/^recent:/, ''));
  const branchItems = $derived(refs ? branchPopupItems(refs, handlers, branchFolders, toggleFolder).map(item =>
    item.id.startsWith(branchSectionPrefix) ? { ...item, icon: branchSectionIcon }
    : item.id.startsWith(branchFolderPrefix) ? { ...item, icon: branchFolderIcon }
    : branchRef(item.id) ? { ...item, icon: branchIcon, badge: branchBadge, trailing: branchTrailing } : item) : []);
  /** IDEA GitBranchesTreeRenderer: current branch uses the tag label icon; favorites the star. */
  function branchIconName(ref: Reference | undefined) {
    const name = ref?.name.startsWith('refs/heads/') ? ref.name.slice(11) : null;
    const favorite = name !== null && defaultFavorite(name);
    if (name !== null && name === refs?.head.head) return favorite ? 'currentBranchFavoriteLabel' : 'currentBranchLabel';
    return favorite ? 'star' : 'branch';
  }
  const shortUpstream = (upstream: string | null) => upstream?.replace(/^refs\/remotes\//, '') ?? '';
  const countText = (count: number) => count > 99 ? '99+' : String(count);
  export function branchHandlers(): BranchPopupHandlers { return handlers; }

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenWindowEvent<[number, string]>('remote_progress', ({ payload }) => {
      if (!disposed && payload[0] === browser.generation) progress = payload[1];
    }).then(value => { unlisten = value; if (disposed) value(); });
    return () => { disposed = true; unlisten?.(); };
  });

  function closeDialog() { if (!busy) { dialog = null; outcome = null; error = null; result = null; } }
  function openPush(options: PushOptions | null) {
    pushOptions = options ?? defaultPushOptions(browser.refs!);
    preview = null; previewSelection = ''; editingTarget = false; dialog = 'push'; outcome = null; error = null; void loadPreview();
  }
  // Generation the shown outgoing commits were requested for; every history refresh bumps it.
  let previewGeneration = 0;
  async function loadPreview(startAt = 0) {
    if (!browser.project || !refs) return;
    const generation = browser.generation;
    previewGeneration = generation;
    previewLoading = true;
    try {
      const value = await invoke<PushPreview>('push_preview', { generation, options: pushOptions, start: startAt, limit: 200 });
      // A newer refresh (e.g. the watcher after a push) superseded this request; the effect below reloads.
      if (generation !== browser.generation) return;
      preview = startAt === 0 ? value : { ...value, commits: [...(preview?.commits ?? []), ...value.commits], subjects: [...(preview?.subjects ?? []), ...value.subjects] };
      previewSelection ||= value.commits[0] ?? '';
    } catch (cause) {
      if (generation === browser.generation && !(cause as RemoteError).detail?.includes('repository.stale')) error = cause as RemoteError;
    } finally { previewLoading = false; }
  }
  // Reload outgoing commits whenever the repository refreshes while Push is open and idle.
  $effect(() => {
    const generation = browser.generation;
    if (dialog === 'push' && !busy && !editingTarget && generation !== previewGeneration) {
      untrack(() => { preview = null; previewSelection = ''; void loadPreview(); });
    }
  });
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
  async function submitPush(force = false) {
    // The dialog stays open with its result; the generation effect reloads the outgoing commits.
    await write('push', { options: { ...pushOptions, forceWithLease: force } });
  }
  function applyTarget() { editingTarget = false; error = null; preview = null; previewSelection = ''; void loadPreview(); }
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

<button class="branch-control" bind:this={branchAnchor} aria-expanded={popup} disabled={!browser.project}
  title={ahead || behind ? `${current} ↑${ahead} ↓${behind}` : current} onclick={() => { popup = !popup; }}>
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" fill="none" stroke="currentColor"><circle cx="4.5" cy="3.5" r="1.5" /><circle cx="4.5" cy="12.5" r="1.5" /><circle cx="11.5" cy="4.5" r="1.5" /><path d="M4.5 5v6M11.5 6c0 3-7 2.5-7 5" /></svg>
  <span class="name">{current}</span>
  {#if ahead}<svg class="outgoing" viewBox="0 0 16 16" width="12" height="12" aria-label={`↑${ahead}`} fill="none" stroke="currentColor" stroke-width="1.5"><path d="M4 12 12 4M6 4h6v6" /></svg>{/if}
  {#if behind}<svg class="incoming" viewBox="0 0 16 16" width="12" height="12" aria-label={`↓${behind}`} fill="none" stroke="currentColor" stroke-width="1.5"><path d="M12 4 4 12M10 12H4V6" /></svg>{/if}
  <svg class="chevron" viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="m4 6 4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>
</button>
<span class="remote-actions">
  <button title={gitText('updateProject')} disabled={!browser.project || busy} onclick={() => handlers.update()}>↓</button>
  <button title={gitText('pushAction')} disabled={!browser.project || busy} onclick={() => openPush(null)}>↑</button>
  <button title={gitText('fetch')} disabled={!browser.project || busy} onclick={() => void fetchRemote()}>↻</button>
</span>
{#snippet branchSectionIcon(id: string)}<svg class="folder-chevron" class:expanded={!branchFolders.has(id)} viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="m6 4 4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>{/snippet}
{#snippet branchFolderIcon(id: string)}<svg class="folder-chevron" class:expanded={branchFolders.has(id)} viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="m6 4 4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.2" /></svg><img class="branch-icon" src={ideaIcon('folder', appearance.dark)} alt="" width="16" height="16" />{/snippet}
{#snippet branchIcon(id: string)}{@const ref = branchRef(id)}<img class="branch-icon" src={ideaIcon(branchIconName(ref), appearance.dark)} alt="" width="16" height="16" />{/snippet}
{#snippet branchBadge(id: string)}{@const counts = trackCounts(branchRef(id)?.track ?? '')}
  {#if counts.ahead}<span class="track outgoing" title={gitText('outgoing').replace('{count}', String(counts.ahead))}><img src={ideaIcon('changesPush', appearance.dark)} alt="" width="12" height="12" />{countText(counts.ahead)}</span>{/if}
  {#if counts.behind}<span class="track incoming" title={gitText('incoming').replace('{count}', String(counts.behind))}><img src={ideaIcon('changesUpdate', appearance.dark)} alt="" width="12" height="12" />{countText(counts.behind)}</span>{/if}
{/snippet}
{#snippet branchTrailing(id: string)}{@const ref = branchRef(id)}
  {#if ref?.upstream}<span class="upstream">{shortUpstream(ref.upstream)}</span>{/if}
{/snippet}
{#if popup && refs}<div class="branch-popup"><Popup items={branchItems} label={gitText('branches')} anchor={branchAnchor} onclose={() => { popup = false; }} /></div>{/if}

{#if dialog}
  <dialog bind:this={workflowDialog} class="workflow-dialog" class:push={dialog === 'push'} aria-labelledby="workflow-title"
    oncancel={event => { event.preventDefault(); closeDialog(); }}>
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
      <h2 id="workflow-title">Push Commits to {projectName}</h2>
      <div class="push-grid">
        <section class="push-commits">
          <div class="push-target">
            <strong>{current}</strong> →
            {#if editingTarget}
              <form class="target-form" onsubmit={event => { event.preventDefault(); applyTarget(); }}>
                <select aria-label="Remote" bind:value={pushOptions.remote}>{#each Object.keys(refs?.remotes ?? {}) as remote}<option value={remote}>{remote}</option>{/each}</select> :
                <input aria-label="Branch" bind:value={pushOptions.branch} placeholder={branchForPreview(refs, pushOptions) ?? ''} />
              </form>
            {:else}
              <button class="link" title="Edit target" onclick={() => { editingTarget = true; }}>{preview?.remote ?? pushOptions.remote ?? ''} : {preview?.branch ?? pushOptions.branch ?? ''}</button>
            {/if}
            {#if newTarget}<span class="new-target">New</span>{/if}
          </div>
          <ol>
            {#each preview?.commits ?? [] as hash, index}<li><button class:selected={previewSelection === hash} title={hash} onclick={() => chooseCommit(hash)}>{preview?.subjects[index] || hash.slice(0, 8)}</button></li>{/each}
          </ol>
          {#if previewLoading}<p class="hint">Loading…</p>{:else if preview && !preview.total}<p class="hint">Nothing to push</p>{/if}
          {#if preview && preview.total > preview.commits.length}<button class="link" onclick={() => void loadPreview(preview!.commits.length)}>Load more commits</button>{/if}
        </section>
        <section class="push-detail">{#if previewSelection}<PagedFileTree hash={previewSelection} generation={browser.generation} onerror={cause => { error = cause as RemoteError; }} />{:else}<p class="hint">No commits selected</p>{/if}</section>
      </div>
      <footer class="push-footer">
        <label><input type="checkbox" bind:checked={pushOptions.tags} /> Push tags</label>
        <span class="spacer"></span>
        <button onclick={closeDialog}>Cancel</button>
        <span class="split"><button class="default" disabled={busy || editingTarget || (!pushOptions.remote && !pushOptions.branch)} onclick={() => void submitPush()}>Push</button><button class="default arrow" aria-label="More push options" aria-haspopup="menu" disabled={busy || editingTarget} onclick={event => { pushMenu = event.currentTarget; }}>⌄</button></span>
      </footer>
      {#if pushMenu}<Popup label="Push" anchor={pushMenu} items={[{ id: 'force', label: 'Force Push', run: () => void submitPush(true) }]} onclose={() => { pushMenu = null; }} />{/if}
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
  .branch-popup { --popup-width: calc(var(--font-ui-size) * 30); }
  .branch-icon, .folder-chevron { flex: none; }
  .folder-chevron { color: var(--text-secondary); }
  .folder-chevron.expanded { transform: rotate(90deg); }
  .track { display: inline-flex; align-items: center; margin-inline-start: var(--space-inline); font-size: var(--font-small-size); vertical-align: middle; }
  .outgoing { color: var(--status-added); }
  .incoming { color: var(--accent); }
  .upstream { flex: none; max-width: 40%; overflow: hidden; text-overflow: ellipsis; color: var(--text-secondary); font-size: var(--font-small-size); }
  .branch-control { display: inline-flex; align-items: center; gap: var(--space-inline); }
  .branch-control > svg { flex: none; color: var(--text-header-secondary); }
  .branch-control .outgoing { color: var(--text-header-outgoing); }
  .branch-control .incoming { color: var(--text-header-incoming); }
  .remote-actions { display: inline-flex; gap: 2px; margin-inline-start: auto; }
  .workflow-dialog { width: min(760px, 88vw); max-height: 86vh; overflow: auto; color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  .workflow-dialog::backdrop { background: #0008; }
  .workflow-dialog.push { width: min(900px, 92vw); }
  h2 { margin-block-start: 0; font-size: calc(var(--font-ui-size) * 1.15); }
  label { display: block; margin-block: var(--space-panel-y); } input:not([type='checkbox']):not([type='radio']) { display: block; width: 100%; font: inherit; color: var(--input-fg); background: var(--input-bg); border: var(--border-width) solid var(--input-border); padding: var(--space-panel-y); }
  footer { display: flex; justify-content: end; gap: var(--space-inline); margin-top: var(--space-panel-x); } footer button { font: inherit; padding: var(--space-panel-y) var(--space-panel-x); }
  .invalid,.error { color: var(--status-conflicted); } .hint { color: var(--text-secondary); font-size: var(--font-small-size); }
  .new-target { color: var(--accent); font-size: var(--font-small-size); }
  .push-grid { display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr); height: min(420px, 60vh); border: var(--border-width) solid var(--separator); }
  .push-commits { display: flex; flex-direction: column; min-height: 0; overflow: auto; padding: var(--space-panel-y) var(--space-inline); }
  .push-target { display: flex; align-items: center; gap: calc(var(--space-inline) / 2); padding-block: calc(var(--space-panel-y) / 2); }
  .target-form { display: flex; align-items: center; gap: calc(var(--space-inline) / 2); }
  .target-form select, .target-form input { font: inherit; color: var(--input-fg); background: var(--input-bg); border: var(--border-width) solid var(--input-border); }
  .target-form input:not([type='checkbox']) { display: inline-block; width: calc(var(--font-ui-size) * 10); padding: 0 calc(var(--space-inline) / 2); }
  .link { font: inherit; color: var(--accent); background: none; border: 0; padding: 0; cursor: pointer; text-align: start; }
  ol { list-style: none; margin: 0; padding-inline-start: calc(var(--space-inline) * 2); } ol button { width: 100%; text-align: start; border: 0; background: none; color: inherit; font: inherit; padding: 2px var(--space-inline); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; } ol button.selected { background: var(--selection-bg); }
  .push-detail { min-height: 0; border-inline-start: var(--border-width) solid var(--separator); }
  .push-footer { align-items: center; } .push-footer label { margin: 0; } .spacer { flex: 1; }
  .split { display: inline-flex; } .split .arrow { padding-inline: var(--space-inline); border-inline-start: 0; }
  code { font-family: var(--font-editor-family); }
</style>
