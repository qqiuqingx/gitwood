<script lang="ts">
  import IntegrationDialog from './IntegrationDialog.svelte';
  import ComparisonDialog from './ComparisonDialog.svelte';
  import UpstreamDialog from './UpstreamDialog.svelte';
  import HistoryTreeDialog from '../browse/HistoryTreeDialog.svelte';
  import HistoryChildrenDialog from './HistoryChildrenDialog.svelte';
  import StashChangesDialog from './StashChangesDialog.svelte';
  import StashManagerDialog from './StashManagerDialog.svelte';
  import { stashText } from '../../i18n/stash';
  import type { Commit } from '../browse/types';
  import { browseText as b } from '../../i18n/browse';
  import { integrationText as m } from '../../i18n/integration';
  import { integrationEnabled, integrationErrorText, type Integration } from './integration';
  import { createActionRegistry } from '../../keymap/registry';
  import ConflictDialog from '../conflicts/ConflictDialog.svelte';
  import { requestResolution } from '../conflicts/model';
  import { conflictText } from '../../i18n/conflict';
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
    type HistoryAction, type PushOptions, type PushPreview, type RemoteError, type RemoteOutcome, type UpdateStrategy,
  } from './model';
  import { abortConfirmation, operationActions, operationLabel, sequenceErrorText, sequenceFeedback, type SequenceAction, type SequenceOutcome } from './operation';
  import { notifications } from '../notifications/center.svelte';
  import type { NoticeAction } from '../notifications/model';
  import { operationText } from '../../i18n/operation';
  import { historyMenuText as ht } from '../../i18n/history-menu';

  let { browser, oncommitpush }: { browser: RepositoryBrowser; oncommitpush: () => void } = $props();
  let integration = $state<{ kind: Integration['kind']; seed: string } | null>(null);
  let comparison = $state<{ left: string; right: string | null; working: boolean } | null>(null);
  let upstreamBranch = $state<string | null>(null);
  let historyTreeRevision = $state<string | null>(null);
  let historyChildren = $state<string[]>([]);
  let stashCreateOpen = $state(false), stashManagerOpen = $state(false);
  let historyTarget = $state('');
  let historyDetail = $state<Commit | null>(null);
  let historyCommits = $state<string[]>([]);
  let resetMode = $state<'soft' | 'mixed' | 'hard' | 'keep'>('mixed');
  let historyCheckoutMode = $state<CheckoutMode>('normal');
  let tagName = $state('');
  let tagMessage = $state('');
  let isAnnotatedTag = $state(false);
  let remoteTagName = $state('');
  let remoteTagRemote = $state('');
  $effect(() => {
    browser.project?.root;
    untrack(() => { integration = null; comparison = null; upstreamBranch = null; historyTreeRevision = null; historyChildren = []; });
  });
  const canIntegrate = () => integrationEnabled(browser.refs, browser.status?.operation, busy || browser.commit.busy || !!integration || !!comparison || upstreamBranch !== null, browser.status?.conflicts, browser.status?.update_stash);
  const integrationActions = ['merge', 'rebase', 'pull'] as const;
  const actions = createActionRegistry();
  const registeredHistoryActions: Partial<Record<HistoryAction, string>> = {
    cherryPick: 'Git.CherryPick', revert: 'Git.Revert', reset: 'Git.ResetCurrentBranchToHere',
    checkoutRevision: 'Git.CheckoutRevision', newTag: 'Git.NewTag', deleteTag: 'Git.DeleteTag',
    deleteRemoteTag: 'Git.DeleteRemoteTag', pushTag: 'Git.PushTag', pushUpTo: 'Git.PushUpTo',
    compare: 'Git.CompareVersions', localDiff: 'Git.ShowDiffWithLocal', browse: 'Git.ShowRepositoryAtRevision', child: 'Git.GoToChildCommit',
  };
  let historySelection: string[] = [];
  let actionTarget = '';
  for (const kind of integrationActions) actions.register(`Git.${kind[0].toUpperCase()}${kind.slice(1)}`, { run: () => openIntegration(kind, actionTarget), enabled: canIntegrate });
  for (const [action, id] of Object.entries(registeredHistoryActions) as [HistoryAction, string][]) {
    actions.register(id, { run: () => { void openHistoryAction(action, actionTarget, historySelection); }, enabled: () => canRunHistoryAction(action) });
  }
  actions.register('Git.CheckoutWithRebase', { run: () => openIntegration('checkoutRebase', actionTarget), enabled: canIntegrate });
  actions.register('Git.CompareWithCurrent', { run: () => openComparison(false), enabled: () => !!browser.refs && !busy });
  actions.register('Git.ShowDiffWithWorkingTree', { run: () => openComparison(true), enabled: () => !!browser.refs && !busy });
  actions.register('Git.SetUpstream', { run: () => openUpstream(actionTarget), enabled: canIntegrate });
  actions.register('Git.UnsetUpstream', { run: () => { popup = false; void write('set_upstream', { name: actionTarget, upstream: null }); }, enabled: canIntegrate });
  actions.register('Git.StashChanges', { run: () => { stashCreateOpen = true; }, enabled: () => !!browser.project && !busy && !browser.commit.busy && !browser.status?.operation && !browser.status?.conflicts && (browser.status?.changes ?? 0) + (browser.status?.untracked ?? 0) > 0 });
  actions.register('Git.UnstashChanges', { run: () => { stashManagerOpen = true; }, enabled: () => !!browser.project && !busy && !browser.commit.busy });
  function canHistoryWrite() {
    return !!browser.refs?.head.head && browser.refs.head.head !== '(detached)' && !!browser.refs.head.oid && browser.refs.head.oid !== '(initial)' && !busy && !browser.commit.busy && !integration && !comparison && !upstreamBranch
      && !browser.status?.operation && !browser.status?.conflicts && !browser.status?.update_stash;
  }
  function canHistoryApply() { return canHistoryWrite() && browser.status?.staged === 0; }
  function currentHistorySelection() {
    return historySelection.length ? historySelection : browser.selection.size ? [...browser.selection] : browser.selected ? [browser.selected] : [];
  }
  function canRunHistoryAction(action: HistoryAction) {
    if (['compare', 'localDiff', 'browse', 'child'].includes(action)) {
      const count = currentHistorySelection().length;
      return !!browser.refs && !busy && (action !== 'compare' || count === 2) && count > 0;
    }
    if (action === 'cherryPick' || action === 'revert') return canHistoryApply();
    return canHistoryWrite();
  }
  export function runAction(id: string, target = '') {
    actionTarget = target;
    try { return actions.run(id); } finally { actionTarget = ''; }
  }
  function dispatchHistoryAction(action: HistoryAction, hash: string, selected: string[]) {
    const id = registeredHistoryActions[action];
    if (!id) return;
    historySelection = selected;
    try { runAction(id, hash); } finally { historySelection = []; }
  }
  function openComparison(working: boolean) { if (!actionTarget) return; popup = false; comparison = { left: actionTarget, right: null, working }; }
  function openIntegration(kind: Integration['kind'], seed = '') { if (!canIntegrate()) return; error = null; dialog = null; popup = false; integration = { kind, seed }; }
  function openUpstream(name: string) { dialog = null; popup = false; upstreamBranch = name; }
  let branchAnchor = $state<HTMLButtonElement>();
  let popup = $state(false);
  let dialog = $state<'new' | 'rename' | 'delete' | 'checkout' | 'update' | 'push' | 'abort' | 'historyReset' | 'historyTag' | 'historyCheckout' | 'historyLocalTagDelete' | 'historyRemoteTagDelete' | null>(null);
  let name = $state('');
  let newName = $state('');
  let start = $state('');
  let checkout = $state(true);
  let force = $state(false);
  let mode = $state<CheckoutMode>('normal');
  let updateStrategy = $state<UpdateStrategy>('merge');
  let updateStrategyLoading = $state(false);
  let pushOptions = $state<PushOptions>({ remote: null, branch: null, forceWithLease: false, tags: false, upTo: null });
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
  let historyCancelButton = $state<HTMLButtonElement>();
  let editingTarget = $state(false);
  let pushMenu = $state<HTMLElement | null>(null);
  const projectName = $derived(browser.project?.root.split(/[\\/]/).filter(Boolean).pop() ?? '');
  // Modal so the dialog is centred above the window instead of anchored to the title bar.
  $effect(() => {
    const element = workflowDialog;
    if (element && dialog && !element.open) {
      element.showModal();
      if ((dialog === 'historyReset' && resetMode === 'hard') || dialog === 'historyLocalTagDelete' || dialog === 'historyRemoteTagDelete') {
        requestAnimationFrame(() => historyCancelButton?.focus());
      }
    }
  });
  $effect(() => {
    if (dialog === 'historyReset' && resetMode === 'hard') requestAnimationFrame(() => historyCancelButton?.focus());
  });

  const current = $derived(browser.refs?.head.head ?? 'HEAD');
  const operation = $derived(browser.status?.operation ?? null);
  const ongoing = $derived(operationActions(operation));
  const branchText = $derived(operation ? operationLabel(operation, browser.refs?.head.head ?? null) : current);
  const refs = $derived(browser.refs);
  const ahead = $derived(refs?.head.ahead ?? 0);
  const behind = $derived(refs?.head.behind ?? 0);
  const expected = () => browser.status?.fingerprint ?? '';
  const displayError = $derived(error ? (dialog === 'abort' ? { ...sequenceErrorText(operation?.kind ?? null, error), action: '' } : remoteErrorText(error)) : null);
  const newTarget = $derived(isNewPushTarget(refs, pushOptions));
  const handlers: BranchPopupHandlers = {
    canIntegrate,
    canHistoryWrite,
    canHistoryApply,
    merge: ref => { runAction('Git.Merge', ref); },
    rebase: ref => { runAction('Git.Rebase', ref); },
    checkoutRebase: ref => { runAction('Git.CheckoutWithRebase', ref); },
    compare: (left, working) => { runAction(working ? 'Git.ShowDiffWithWorkingTree' : 'Git.CompareWithCurrent', left); },
    upstream: name => { runAction('Git.SetUpstream', name); },
    unsetUpstream: name => { runAction('Git.UnsetUpstream', name); },
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
    historyAction: (action, hash, selected) => dispatchHistoryAction(action, hash, selected),
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
  // IDEA Git.Branches.List starts with Git.Ongoing.Rebase.Actions while an operation is in progress.
  const ongoingItems = $derived(ongoing.length ? [...ongoing.map(item => ({ id: `ongoing:${item.action}`, label: item.label, disabled: busy, run: () => { popup = false; requestSequence(item.action); } })),
    { id: 'ongoing:separator', label: '', kind: 'separator' as const }] : []);
  const branchItems = $derived(refs ? [...ongoingItems, ...branchPopupItems(refs, handlers, branchFolders, toggleFolder)].map(item =>
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
    const shown = dialog;
    busy = true; progress = ''; error = null; outcome = null;
    try {
      const value = await invoke<RemoteOutcome | { kind: string; files?: { display: string }[]; name?: string; detail?: string }>(command, { generation: browser.generation, expected: expected(), ...args });
      if (value.kind === 'notMerged') {
        force = true; dialog = 'delete'; error = { key: 'remote.failed', detail: `Branch ${value.name ?? name} is not fully merged. Confirm Force Delete to continue.` }; return;
      }
      if (value.kind === 'localChanges') {
        affectedFiles = value.files?.map(file => file.display) ?? []; dialog = 'checkout'; mode = 'smart'; error = { key: 'remote.failed', detail: `Checkout would overwrite ${affectedFiles.length} local file(s). Choose Smart Checkout or Force Checkout.` }; return;
      }
      await browser.refresh({ status: true, refs: true, history: true });
      // Push keeps its dialog open with the result (user choice); other actions report as notifications.
      if (shown === 'push') { outcome = value as RemoteOutcome; return; }
      if (dialog === shown) { dialog = null; affectedFiles = []; }
      notifyOutcome(command, value as RemoteOutcome);
    } catch (cause) {
      if (shown && dialog === shown) error = cause as RemoteError;
      else { const text = remoteErrorText(cause as RemoteError); notifications.notify({ kind: 'error', title: text.title, detail: [text.detail, text.action].filter(Boolean).join('\n'), group: 'git' }); }
    }
    finally { busy = false; }
  }
  const commandTitle: Record<string, string> = {
    set_upstream: m('completed'),
    continue_update: conflictText('updateCompleted'), create_branch: 'Branch created', rename_branch: 'Branch renamed', delete_branch: 'Branch deleted', checkout_branch: 'Checked out',
    checkout_remote_branch: 'Checked out', fetch: 'Fetch completed', update_project: 'Project updated', abort_update: 'Update aborted',
  };
  /** IDEA VcsNotifier: success balloons hide; conflicts and recovery stay with their actions. */
  function notifyOutcome(command: string, value: RemoteOutcome) {
    const stash = outcomeStash(value);
    const resolve: NoticeAction = { label: conflictText('resolve'), run: requestResolution };
    const abort: NoticeAction = { label: 'Abort Update', run: () => void write('abort_update', { stash }) };
    if (command === 'integrate') { notifyIntegration(value); return; }
    if (value.kind === 'completed') notifications.notify({ kind: 'success', title: commandTitle[command] ?? 'Completed', group: 'git' });
    else if (value.kind === 'conflicts') notifications.notify({ kind: 'warning', title: 'Update stopped with conflicts', detail: operationText('conflicts', { count: value.files.length }), files: value.files.map(file => file.display), actions: [resolve, abort], group: 'git' });
    else if (value.kind === 'interrupted') { const text = remoteErrorText(value.error); notifications.notify({ kind: 'error', title: 'Update interrupted', detail: [text.detail, stash ? `Stash ${stash} was retained.` : ''].filter(Boolean).join('\n'), actions: [abort], group: 'git' }); }
    else if (value.kind === 'stashRecovery') {
      const detail = 'detail' in value && typeof value.detail === 'string' ? value.detail : '';
      notifications.notify({ kind: 'error', title: 'Working tree recovery is required', detail: [`Stash ${value.stash} was retained; inspect it before retrying.`, detail].filter(Boolean).join('\n'), files: value.files.map(file => file.display), actions: value.files.length ? [resolve] : [], group: 'git' });
    }
    else if (value.kind === 'push') notifications.notify(value.success ? { kind: 'success', title: 'Push completed', group: 'git' } : { kind: 'error', title: 'Push rejected', detail: value.refs.filter(ref => ref.flag === '!').map(ref => pushRejectionText(ref.rejection)).join(' '), group: 'git' });
  }
  function notifyIntegration(value: RemoteOutcome) {
    const resolve: NoticeAction = { label: conflictText('resolve'), run: requestResolution };
    const followups = operationActions(browser.status?.operation ?? null).map(item => ({ label: item.label, run: () => requestSequence(item.action) }));
    if (value.kind === 'completed') notifications.notify({ kind: 'success', title: m('completed'), group: 'git' });
    else if (value.kind === 'conflicts') notifications.notify({ kind: 'warning', title: m('conflicts'), files: value.files.map(f => f.display), actions: [resolve, ...followups], group: 'git' });
    else if (value.kind === 'interrupted') notifications.notify({ kind: value.error.key === 'integration.paused' ? 'info' : 'error', title: m(value.error.key === 'integration.paused' ? 'paused' : 'interrupted'), detail: [integrationErrorText(value.error), value.stash ? m('retained', { stash: value.stash }) : ''].filter(Boolean).join('\n'), actions: followups, group: 'git' });
    else if (value.kind === 'stashRecovery') notifications.notify({ kind: 'error', title: m('recover'), detail: [m('retained', { stash: value.stash }), value.detail].filter(Boolean).join('\n'), files: value.files.map(f => f.display), actions: value.files.length ? [resolve] : [], group: 'git' });
  }
  /** Abort asks first, as IDEA GitAbortOperationAction.confirmAbort. */
  function requestSequence(action: SequenceAction) {
    if (!operation) {
      if (action === 'continue' && browser.status?.update_stash) void write('continue_update', { stash: browser.status.update_stash, action, message: null });
      return;
    }
    if (action === 'abort') { error = null; dialog = 'abort'; return; }
    if (action === 'continue' && browser.status?.conflicts) { requestResolution(); return; }
    void runSequence(action);
  }
  async function runSequence(action: SequenceAction) {
    const state = operation;
    if (!browser.project || busy || !state) return;
    if (browser.status?.update_stash) {
      await write('continue_update', { stash: browser.status?.update_stash, action, message: null });
      return;
    }
    busy = true; progress = ''; error = null;
    try {
      const value = await invoke<SequenceOutcome>('sequence', { generation: browser.generation, expected: expected(), action, message: null });
      if (dialog === 'abort') dialog = null;
      await browser.refresh({ status: true, refs: true, history: true });
      const feedback = sequenceFeedback(state.kind, action, value);
      const next = value.kind === 'completed' ? [] : operationActions(value.operation)
        .map(item => ({ label: item.label, run: () => requestSequence(item.action) }));
      if (value.kind === 'conflicts') next.unshift({ label: conflictText('resolve'), run: requestResolution });
      notifications.notify({ kind: feedback.kind, title: feedback.title, detail: feedback.detail, files: feedback.files, actions: feedback.offerActions ? next : [], group: 'operation' });
    } catch (cause) {
      const text = sequenceErrorText(state.kind, cause as RemoteError);
      if (dialog === 'abort') error = cause as RemoteError;
      else notifications.notify({ kind: 'error', title: text.title, detail: text.detail, group: 'operation' });
    } finally { busy = false; }
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
  async function openHistoryAction(action: HistoryAction, hash: string, selected: string[]) {
    const selection = selected.length ? selected : currentHistorySelection();
    const target = hash || selection[0] || browser.selected || '';
    historyTarget = target; historyCommits = selection; historyDetail = browser.details?.hash === target ? browser.details : null; error = null; outcome = null;
    if (action === 'compare') {
      const positions = new Map([...browser.rows.values()].map(row => [row.commit.hash, row.index]));
      const ordered = selection.slice().sort((a, b) => (positions.get(a) ?? Number.MAX_SAFE_INTEGER) - (positions.get(b) ?? Number.MAX_SAFE_INTEGER));
      if (ordered.length === 2) comparison = { left: ordered[1], right: ordered[0], working: false };
      return;
    }
    if (action === 'localDiff') { comparison = { left: target, right: null, working: true }; return; }
    if (action === 'browse') { historyTreeRevision = target; return; }
    if (action === 'child') {
      try {
        const result = await invoke<{ children: string[] }>('history_children', { generation: browser.generation, hash: target });
        const positions = await Promise.all(result.children.map(child => browser.historyPosition(child)));
        const visible = result.children.map((child, index) => ({ child, position: positions[index] }))
          .filter((item): item is { child: string; position: number } => item.position !== null)
          .sort((a, b) => a.position - b.position)
          .map(item => item.child);
        if (visible.length === 1) await browser.goToCommit(visible[0]);
        else historyChildren = visible;
      } catch (cause) { browser.report(cause); }
      return;
    }
    if (action === 'reset') { resetMode = 'mixed'; dialog = 'historyReset'; if (!historyDetail) void loadHistoryDetail(target); return; }
    if (action === 'newTag') { tagName = ''; tagMessage = ''; isAnnotatedTag = false; dialog = 'historyTag'; return; }
    if (action === 'checkoutRevision') { historyCheckoutMode = 'normal'; affectedFiles = []; dialog = 'historyCheckout'; return; }
    if (action === 'pushUpTo') { openPush({ ...defaultPushOptions(browser.refs!), upTo: target }); return; }
    if (action === 'cherryPick') { void submitHistory({ kind: 'cherryPick', commits: selection, recordOrigin: true, empty: 'skip' }); return; }
    if (action === 'revert') { void submitHistory({ kind: 'revert', commits: selection }); return; }
    const name = target.replace(/^refs\/tags\//, '');
    if (action === 'deleteTag') { tagName = name; dialog = 'historyLocalTagDelete'; return; }
    if (action === 'deleteRemoteTag') {
      const remote = currentRemote();
      if (!remote) { notifications.notify({ kind: 'error', title: ht('deleteRemoteTag'), detail: m('upstreamRequired'), group: 'git' }); return; }
      remoteTagName = name; remoteTagRemote = remote; dialog = 'historyRemoteTagDelete'; return;
    }
    if (action === 'pushTag') {
      const remote = currentRemote();
      if (!remote) { notifications.notify({ kind: 'error', title: ht('pushTag').replace('{name}', name), detail: m('upstreamRequired'), group: 'git' }); return; }
      void submitHistory({ kind: 'pushTag', remote, name });
    }
  }
  let historyDetailTicket = 0;
  async function loadHistoryDetail(hash: string) {
    const generation = browser.generation, ticket = ++historyDetailTicket;
    try {
      const detail = await invoke<Commit>('commit_details', { generation, hash });
      if (ticket === historyDetailTicket && generation === browser.generation) historyDetail = detail;
    } catch (cause) {
      if (ticket === historyDetailTicket && generation === browser.generation) error = cause as RemoteError;
    }
  }
  function currentRemote(): string | null {
    const upstream = refs?.local.find(ref => ref.name === `refs/heads/${current}`)?.upstream;
    const remote = upstream?.match(/^refs\/remotes\/([^/]+)\//)?.[1];
    const remotes = Object.keys(refs?.remotes ?? {});
    return remote ?? (remotes.includes('origin') ? 'origin' : remotes[0] ?? null);
  }
  async function submitHistory(request: Record<string, unknown>) {
    if (!browser.project || busy) return;
    busy = true; error = null;
    try {
      const value = await invoke<{ kind: string; files?: { display: string }[]; stash?: string; detail?: string }>('history_write', { generation: browser.generation, expected: expected(), request });
      await browser.refresh({ status: true, refs: true, history: true });
      if (value.kind === 'completed') {
        dialog = null;
        notifications.notify({ kind: 'success', title: m('completed'), group: 'git' });
      } else if (value.kind === 'conflicts') {
        dialog = null;
        notifications.notify({ kind: 'warning', title: m('conflicts'), detail: operationText('conflicts', { count: value.files?.length ?? 0 }), files: value.files?.map(file => file.display), actions: [{ label: conflictText('resolve'), run: requestResolution }], group: 'git' });
      } else if (value.kind === 'localChanges') {
        affectedFiles = value.files?.map(file => file.display) ?? []; historyCheckoutMode = 'normal'; dialog = 'historyCheckout';
      } else if (value.kind === 'stashRecovery') {
        dialog = null;
        notifications.notify({ kind: 'error', title: m('recover'), detail: [`Stash ${value.stash} was retained.`, value.detail].filter(Boolean).join('\n'), group: 'git' });
      } else if (value.kind === 'stopped') {
        if (dialog) error = { key: 'history.failed', detail: value.detail ?? '', guidance: 'remote.tryFetchInTerminal' };
        else notifications.notify({ kind: 'error', title: m('interrupted'), detail: value.detail ?? '', group: 'operation' });
      }
    } catch (cause) {
      const text = remoteErrorText(cause as RemoteError);
      if (dialog) error = cause as RemoteError;
      else notifications.notify({ kind: 'error', title: text.title, detail: [text.detail, text.action].filter(Boolean).join('\n'), group: 'git' });
    }
    finally { busy = false; }
  }
  function submitReset() { void submitHistory({ kind: 'reset', target: historyTarget, mode: resetMode }); }
  function submitTag() { if (!tagName.trim() || (isAnnotatedTag && !tagMessage.trim())) return; void submitHistory({ kind: 'createTag', name: tagName.trim(), target: historyTarget, message: isAnnotatedTag ? tagMessage.trim() : null }); }
  function submitRevisionCheckout() { void submitHistory({ kind: 'checkoutRevision', target: historyTarget, mode: historyCheckoutMode }); }
  function submitDeleteLocalTag() { void submitHistory({ kind: 'deleteTag', name: tagName }); }
  function submitDeleteRemoteTag() { void submitHistory({ kind: 'deleteRemoteTag', remote: remoteTagRemote, name: remoteTagName }); }
  export async function commitAndPush() {
    if (!browser.commit.message.trim() || (!browser.commit.count && operation?.kind !== 'merge')) return;
    if (browser.status?.update_stash && operation) { await continueAndPush(); return; }
    busy = true; progress = ''; error = null; result = null;
    try {
      result = await invoke<CommitPushResult>('commit_and_push', {
        generation: browser.generation, expected: expected(), selection: $state.snapshot(browser.commit.selection),
        message: browser.commit.message, amend: browser.commit.amend, options: defaultPushOptions(browser.refs!),
      });
      browser.commit.message = '';
      browser.commit.amend = false;
      await browser.refresh({ status: true, refs: true, history: true });
      const feedback = commitPushFeedback(result);
      const committed = `Commit ${result.hash.slice(0, 8)}`;
      if (feedback.push === 'success') notifications.notify({ kind: 'success', title: `${committed}: commit and push completed`, group: 'git' });
      else if (feedback.push === 'recovery') notifications.notify({ kind: 'error', title: `${committed}: index recovery required`, detail: 'Push was not started.', group: 'git' });
      else if (feedback.push === 'failed') notifications.notify({ kind: 'error', title: `${committed}: commit completed, push failed`,
        detail: result.push && result.push.kind === 'push' ? result.push.refs.filter(ref => ref.flag === '!').map(ref => pushRejectionText(ref.rejection)).join(' ') : '', group: 'git' });
      if (result.push == null && result.kind === 'committed') oncommitpush();
    } catch (cause) {
      const text = remoteErrorText(cause as RemoteError);
      notifications.notify({ kind: 'error', title: text.title, detail: [text.detail, text.action].filter(Boolean).join('\n'), group: 'git' });
    }
    finally { busy = false; }
  }
  async function continueAndPush() {
    busy = true;
    try {
      const value = await invoke<RemoteOutcome>('continue_update', { generation: browser.generation, expected: expected(), stash: browser.status?.update_stash, action: 'continue', message: browser.commit.message });
      if (value.kind === 'completed' || value.kind === 'stashRecovery') browser.commit.message = '';
      await browser.refresh({ status: true, refs: true, history: true });
      notifyOutcome('continue_update', value);
      if (value.kind !== 'completed') return;
      const pushed = await invoke<RemoteOutcome>('push', { generation: browser.generation, expected: expected(), options: defaultPushOptions(browser.refs!) });
      notifyOutcome('push', pushed);
      await browser.refresh({ status: true, refs: true, history: true });
    } catch (cause) {
      const text = remoteErrorText(cause as RemoteError);
      notifications.notify({ kind: 'error', title: text.title, detail: text.detail, group: 'git' });
    } finally { busy = false; }
  }
  function chooseCommit(hash: string) { previewSelection = hash; }
  function outcomeText(value: RemoteOutcome | null) {
    if (!value) return '';
    if (value.kind === 'completed') return 'Completed.';
    if (value.kind === 'push') return value.success ? `Push completed (${value.refs.length} ref${value.refs.length === 1 ? '' : 's'}).` : 'Push was rejected; update first or inspect the remote rule.';
    if (value.kind === 'conflicts') return `Update stopped with ${value.files.length} conflicted file(s). ${conflictText('resolve')}.`;
    if (value.kind === 'stashRecovery') return `Working tree recovery is required; stash ${value.stash} was retained.`;
    return 'Operation interrupted; inspect the retained stash before retrying.';
  }
  function outcomeStash(value: RemoteOutcome | null) { return value && 'stash' in value ? value.stash ?? null : null; }
</script>

{#if integration}{#key JSON.stringify(integration)}<IntegrationDialog {browser} kind={integration.kind} seed={integration.seed} bind:busy bind:progress onclose={() => { integration = null; }} onresult={value => notifyIntegration(value)} />{/key}{/if}
{#if comparison}<ComparisonDialog {browser} left={comparison.left} right={comparison.right} working={comparison.working} onclose={() => { comparison = null; }} />{/if}
{#if historyTreeRevision}<HistoryTreeDialog {browser} revision={historyTreeRevision} onclose={() => { historyTreeRevision = null; }} />{/if}
{#if historyChildren.length}<HistoryChildrenDialog children={historyChildren} onselect={hash => { historyChildren = []; void browser.goToCommit(hash); }} onclose={() => { historyChildren = []; }} />{/if}
{#if upstreamBranch !== null}<UpstreamDialog {browser} name={upstreamBranch} bind:busy onclose={() => { upstreamBranch = null; }} />{/if}
{#if stashCreateOpen}<StashChangesDialog {browser} onclose={() => { stashCreateOpen = false; }} />{/if}
{#if stashManagerOpen}<StashManagerDialog {browser} onclose={() => { stashManagerOpen = false; }} />{/if}
<ConflictDialog {browser} oncontinue={() => requestSequence('continue')} />
<button class="branch-control" bind:this={branchAnchor} aria-expanded={popup} disabled={!browser.project}
  title={ahead || behind ? `${current} ↑${ahead} ↓${behind}` : current} onclick={() => { popup = !popup; }}>
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" fill="none" stroke="currentColor"><circle cx="4.5" cy="3.5" r="1.5" /><circle cx="4.5" cy="12.5" r="1.5" /><circle cx="11.5" cy="4.5" r="1.5" /><path d="M4.5 5v6M11.5 6c0 3-7 2.5-7 5" /></svg>
  <span class="name">{branchText}</span>
  {#if ahead}<svg class="outgoing" viewBox="0 0 16 16" width="12" height="12" aria-label={`↑${ahead}`} fill="none" stroke="currentColor" stroke-width="1.5"><path d="M4 12 12 4M6 4h6v6" /></svg>{/if}
  {#if behind}<svg class="incoming" viewBox="0 0 16 16" width="12" height="12" aria-label={`↓${behind}`} fill="none" stroke="currentColor" stroke-width="1.5"><path d="M12 4 4 12M10 12H4V6" /></svg>{/if}
  <svg class="chevron" viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="m4 6 4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>
</button>
{#if ongoing.length}<span class="operation-actions" role="group" aria-label={branchText}>{#each ongoing as item}<button class={item.tone} disabled={busy} onclick={() => requestSequence(item.action)}>{item.label}</button>{/each}</span>{/if}
{#if busy && !dialog && !integration && upstreamBranch === null}<span class="header-progress" role="status">{progress || 'Working…'}<progress></progress><button onclick={() => void invoke('cancel_write', { generation: browser.generation })}>Cancel</button></span>{/if}
{#if browser.status?.update_stash && !operation && !browser.status.conflicts}<button disabled={busy} onclick={() => requestSequence('continue')}>{m('restoreSaved')}</button>{/if}
<span class="remote-actions">
  <button title={stashText('stash')} disabled={!browser.project || busy || browser.commit.busy || !browser.status || browser.status.changes + browser.status.untracked === 0 || !!browser.status.operation || !!browser.status.conflicts} onclick={() => actions.run('Git.StashChanges')}>{stashText('stash')}</button>
  <button title={stashText('unstash')} disabled={!browser.project || busy || browser.commit.busy} onclick={() => actions.run('Git.UnstashChanges')}>{stashText('unstash')}</button>
  <button title={gitText('updateProject')} disabled={!browser.project || busy} onclick={() => handlers.update()}>↓</button>
  <button title={gitText('pushAction')} disabled={!browser.project || busy} onclick={() => openPush(null)}>↑</button>
  {#each integrationActions as kind}<button disabled={!canIntegrate()} onclick={() => runAction(`Git.${kind[0].toUpperCase()}${kind.slice(1)}`)}>{m(kind)}</button>{/each}
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
    {:else if dialog === 'historyReset'}
      <h2 id="workflow-title">{ht('resetTitle')}</h2><p>{historyTarget.slice(0, 12)}</p>
      {#if historyDetail}<p>{ht('commitSubject')}: {historyDetail.subject}</p><p>{ht('commitAuthor')}: {historyDetail.author}</p>{:else}<p role="status">{b('loading')}</p>{/if}
      <label><input type="radio" name="history-reset" value="soft" bind:group={resetMode} /> {ht('soft')}</label>
      <label><input type="radio" name="history-reset" value="mixed" bind:group={resetMode} /> {ht('mixed')}</label>
      <label><input type="radio" name="history-reset" value="hard" bind:group={resetMode} /> {ht('hard')}</label>
      <label><input type="radio" name="history-reset" value="keep" bind:group={resetMode} /> {ht('keep')}</label>
      {#if resetMode === 'hard'}<p class="warning">{ht('resetHardWarning')}</p>{/if}
      <footer><button bind:this={historyCancelButton} class:default={resetMode === 'hard'} onclick={closeDialog}>{ht('cancel')}</button><button class:default={resetMode !== 'hard'} disabled={busy} onclick={submitReset}>{ht('resetButton')}</button></footer>
    {:else if dialog === 'historyTag'}
      <h2 id="workflow-title">{ht('newTagTitle')}</h2><p>{historyTarget.slice(0, 12)}</p>
      <label>{ht('tagName')} <input bind:value={tagName} /></label>
      <label><input type="checkbox" bind:checked={isAnnotatedTag} /> {ht('annotated')}</label>
      {#if isAnnotatedTag}<label>{ht('tagMessage')} <textarea bind:value={tagMessage}></textarea></label>{/if}
      <footer><button onclick={closeDialog}>{ht('cancel')}</button><button class="default" disabled={busy || !tagName.trim() || (isAnnotatedTag && !tagMessage.trim())} onclick={submitTag}>{ht('newTagTitle')}</button></footer>
    {:else if dialog === 'historyCheckout'}
      <h2 id="workflow-title">{ht('revision').replace('{hash}', historyTarget.slice(0, 8))}</h2><p>{ht('checkoutMode')}</p>
      {#if affectedFiles.length}<ul>{#each affectedFiles as file}<li>{file}</li>{/each}</ul>{/if}
      <label><input type="radio" name="revision-checkout" value="normal" bind:group={historyCheckoutMode} /> {ht('normalCheckout')}</label>
      <label><input type="radio" name="revision-checkout" value="smart" bind:group={historyCheckoutMode} /> {ht('smartCheckout')}</label>
      <label><input type="radio" name="revision-checkout" value="force" bind:group={historyCheckoutMode} /> {ht('forceCheckout')}</label>
      <footer><button onclick={closeDialog}>{ht('cancel')}</button><button onclick={submitRevisionCheckout}>{ht('checkout')}</button></footer>
    {:else if dialog === 'historyLocalTagDelete'}
      <h2 id="workflow-title">{ht('deleteTag').replace('{name}', tagName)}</h2><p>{tagName}</p>
      <footer><button bind:this={historyCancelButton} class="default" onclick={closeDialog}>{ht('cancel')}</button><button disabled={busy} onclick={submitDeleteLocalTag}>{ht('deleteTag').replace('{name}', tagName)}</button></footer>
    {:else if dialog === 'historyRemoteTagDelete'}
      <h2 id="workflow-title">{ht('deleteRemoteTag').replace('{name}', remoteTagName)}</h2>
      <p>{ht('deleteRemoteTagConfirm').replace('{name}', remoteTagName).replace('{remote}', remoteTagRemote)}</p>
      <footer><button bind:this={historyCancelButton} class="default" onclick={closeDialog}>{ht('cancel')}</button><button disabled={busy} onclick={submitDeleteRemoteTag}>{ht('deleteRemoteTag').replace('{name}', remoteTagName)}</button></footer>
    {:else if dialog === 'update'}
      <h2 id="workflow-title">Update Project</h2><p>Fetch and update the current branch.</p>
      <label><input type="radio" name="update" value="merge" bind:group={updateStrategy} /> Merge</label>
      <label><input type="radio" name="update" value="rebase" bind:group={updateStrategy} /> Rebase</label>
      <p class="hint">Default: {updateStrategy}.</p>
      <footer><button onclick={closeDialog}>Cancel</button><button disabled={updateStrategyLoading} onclick={() => void submitUpdate()}>Update</button></footer>
    {:else if dialog === 'abort' && operation}
      {@const confirm = abortConfirmation(operation.kind, projectName)}
      <h2 id="workflow-title">{confirm.title}</h2><p>{confirm.message}</p>
      <!-- IDEA confirmAbort: Abort is the default button. -->
      <footer><button onclick={closeDialog}>{operationText('cancel')}</button><button class="default" disabled={busy} onclick={() => void runSequence('abort')}>{operationText('abort')}</button></footer>
    {:else if dialog === 'push'}
      <h2 id="workflow-title">Push Commits to {projectName}</h2>
      {#if !refs?.local.find(ref => ref.name === `refs/heads/${current}`)?.upstream}<p>{m('upstreamRequired')} <button class="link" disabled={!canIntegrate()} onclick={() => runAction('Git.SetUpstream', current)}>{m('setUpstream')}</button></p>{/if}
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
    {#if outcome}<p role="status">{outcomeText(outcome)}</p>{/if}
    {#if displayError}<div class="error" role="alert"><strong>{displayError.title}</strong><p>{displayError.detail}</p><p>{displayError.action}</p></div>{/if}
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
  .operation-actions { display: inline-flex; gap: 2px; }
  .operation-actions button { font: inherit; font-size: var(--font-small-size); border: 0; padding: 2px var(--space-inline); color: var(--text-header); background: var(--toolbar-hover-bg); }
  .operation-actions .resolve { color: var(--background-primary); background: var(--status-added); }
  .operation-actions .abort { color: var(--background-primary); background: var(--status-conflicted); }
  .header-progress { display: inline-flex; align-items: center; gap: var(--space-inline); font-size: var(--font-small-size); }
  .header-progress progress { width: calc(var(--font-ui-size) * 5); }
  .header-progress button { font: inherit; border: 0; background: none; color: var(--accent); padding: 0; }
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
