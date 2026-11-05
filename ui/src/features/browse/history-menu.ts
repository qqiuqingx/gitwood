import type { PopupItem } from '../../components/popup';
import type { Commit, References } from './types';
import { branchItem, type BranchPopupHandlers } from '../git/model';
import type { HistoryMenuMessage } from '../../i18n/history-menu';
import { historyEditReason, type EditInfo, type EditReason } from './history-edit';

export type UndoInfo = EditInfo;
const undoMessages: Record<EditReason, HistoryMenuMessage> = {
  loading: 'undoLoading', head: 'undoHead', merge: 'undoMerge', root: 'undoRoot', state: 'undoState', protected: 'undoProtected',
};
export function undoReason(commit: Commit, refs: References | null, info: UndoInfo | null): HistoryMenuMessage | null {
  const reason = historyEditReason(commit, refs, info, { head: true, nonMerge: true, nonRoot: true, unprotected: true });
  return reason ? undoMessages[reason] : null;
}

/** IDEA VcsLog actions with several selected commits: only multi-commit actions stay enabled. */
const multiCommitItems = new Set(['copy', 'compare', 'cherryPick', 'revert']);
export function forSelection(items: PopupItem[], count: number): PopupItem[] {
  if (count <= 1) return items;
  return items.map(item => item.kind === 'separator' || multiCommitItems.has(item.id) ? item
    : { ...item, disabled: true, run: undefined, children: undefined, hint: undefined });
}

export function historyMenuItems(commit: Commit, refs: References | null, handlers: BranchPopupHandlers | undefined,
  parentAvailable: boolean, actions: { copy: () => void; parent: () => void; run?: (action: import('../git/model').HistoryAction, hash: string) => void }, t: (key: HistoryMenuMessage) => string, undo?: { info: UndoInfo | null; run: () => void; busy: boolean; writable?: boolean; applyAllowed?: boolean }, selectionCount = 1, selectionHasMerge: boolean | null = false): PopupItem[] {
  const branches = refs && handlers ? [...refs.local, ...Object.values(refs.remotes).flat()].filter(ref => ref.hash === commit.hash).map(ref => branchItem(ref, refs, handlers)) : [];
  const checkout: PopupItem[] = branches.map(item => ({ ...item.children![0], id: `checkout:${item.id}`, label: item.label, children: undefined }));
  // ops/branch.rs validates Checkout against refs/heads; arbitrary revisions are unsupported.
  checkout.push({ id: 'revision', label: t('revision').replace('{hash}', commit.hash.slice(0, 8)), disabled: !undo?.writable, run: () => actions.run?.('checkoutRevision', commit.hash) });
  const disabled = (id: HistoryMenuMessage): PopupItem => ({ id, label: t(id), disabled: true });
  const separator = (id: string): PopupItem => ({ id: `separator:${id}`, label: '', kind: 'separator' });
  const readable = !!refs;
  const reason = undoReason(commit, refs, undo?.info ?? null);
  return [
    { id: 'copy', label: t('copy'), run: actions.copy }, disabled('patch'),
    separator('checkout'), { id: 'checkout', label: t('checkout'), children: checkout, disabled: checkout.every(item => item.disabled) },
    { id: 'browse', label: t('browse'), disabled: !readable, run: () => actions.run?.('browse', commit.hash) },
    separator('compare'), { id: 'compare', label: t('compare'), disabled: !readable || selectionCount !== 2, run: () => actions.run?.('compare', commit.hash) },
    { id: 'localDiff', label: t('localDiff'), disabled: !readable, run: () => actions.run?.('localDiff', commit.hash) },
    separator('reset'), { id: 'reset', label: t('reset'), disabled: !undo || undo.busy || !undo.writable, run: () => actions.run?.('reset', commit.hash) },
    { id: 'revert', label: t('revert'), disabled: !undo?.applyAllowed || commit.parents.length > 1 || selectionHasMerge !== false, hint: commit.parents.length > 1 || selectionHasMerge === true ? t('revertMerge') : selectionHasMerge === null ? t('undoLoading') : !undo?.applyAllowed ? t('historyStagedChanges') : undefined, run: () => actions.run?.('revert', commit.hash) },
    { id: 'cherryPick', label: t('cherryPick'), disabled: !undo?.applyAllowed, hint: !undo?.applyAllowed ? t('historyStagedChanges') : undefined, run: () => actions.run?.('cherryPick', commit.hash) },
    { id: 'undo', label: t('undo'), disabled: !undo || undo.busy || reason !== null, hint: undo?.busy ? t('undoBusy') : reason ? t(reason) : undefined, run: undo?.run },
    separator('edit'), disabled('edit'), disabled('fixup'), disabled('squashInto'), disabled('drop'), disabled('squash'), disabled('rebase'),
    { id: 'push', label: t('push'), disabled: !undo?.writable, run: () => actions.run?.('pushUpTo', commit.hash) },
    separator('branch'), { id: 'branch', label: t('branch'), children: branches, disabled: !branches.length },
    { id: 'newBranch', label: t('newBranch'), disabled: !handlers, run: handlers ? () => handlers.newFrom(commit.hash) : undefined },
    { id: 'tag', label: t('tag'), disabled: !undo?.writable, run: () => actions.run?.('newTag', commit.hash) },
    ...((refs?.tags.filter(ref => ref.hash === commit.hash || ref.peeled === commit.hash) ?? []).flatMap(ref => [
      { id: `push-tag:${ref.name}`, label: t('pushTag').replace('{name}', ref.name.replace(/^refs\/tags\//, '')), disabled: !undo?.writable, run: () => actions.run?.('pushTag', ref.name) },
      { id: `delete-tag:${ref.name}`, label: t('deleteTag').replace('{name}', ref.name.replace(/^refs\/tags\//, '')), disabled: !undo?.writable, run: () => actions.run?.('deleteTag', ref.name) },
      { id: `delete-remote-tag:${ref.name}`, label: t('deleteRemoteTag').replace('{name}', ref.name.replace(/^refs\/tags\//, '')), disabled: !undo?.writable, run: () => actions.run?.('deleteRemoteTag', ref.name) },
    ])),
    separator('navigate'), { id: 'child', label: t('child'), disabled: !readable, run: () => actions.run?.('child', commit.hash) }, { id: 'parent', label: t('parent'), disabled: !commit.parents.length || !parentAvailable, run: actions.parent },
  ];
}
