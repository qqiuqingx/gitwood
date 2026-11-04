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
const multiCommitItems = new Set(['copy']);
export function forSelection(items: PopupItem[], count: number): PopupItem[] {
  if (count <= 1) return items;
  return items.map(item => item.kind === 'separator' || multiCommitItems.has(item.id) ? item
    : { ...item, disabled: true, run: undefined, children: undefined, hint: undefined });
}

export function historyMenuItems(commit: Commit, refs: References | null, handlers: BranchPopupHandlers | undefined,
  parentAvailable: boolean, actions: { copy: () => void; parent: () => void }, t: (key: HistoryMenuMessage) => string, undo?: { info: UndoInfo | null; run: () => void; busy: boolean }): PopupItem[] {
  const branches = refs && handlers ? [...refs.local, ...Object.values(refs.remotes).flat()].filter(ref => ref.hash === commit.hash).map(ref => branchItem(ref, refs, handlers)) : [];
  const checkout: PopupItem[] = branches.map(item => ({ ...item.children![0], id: `checkout:${item.id}`, label: item.label, children: undefined }));
  // ops/branch.rs validates Checkout against refs/heads; arbitrary revisions are unsupported.
  checkout.push({ id: 'revision', label: t('revision').replace('{hash}', commit.hash.slice(0, 8)), disabled: true });
  const disabled = (id: HistoryMenuMessage): PopupItem => ({ id, label: t(id), disabled: true });
  const separator = (id: string): PopupItem => ({ id: `separator:${id}`, label: '', kind: 'separator' });
  const reason = undoReason(commit, refs, undo?.info ?? null);
  return [
    { id: 'copy', label: t('copy'), run: actions.copy }, disabled('patch'),
    separator('checkout'), { id: 'checkout', label: t('checkout'), children: checkout, disabled: checkout.every(item => item.disabled) }, disabled('browse'),
    separator('compare'), disabled('compare'), disabled('localDiff'),
    separator('reset'), disabled('reset'), disabled('revert'), { id: 'undo', label: t('undo'), disabled: !undo || undo.busy || reason !== null, hint: undo?.busy ? t('undoBusy') : reason ? t(reason) : undefined, run: undo?.run },
    separator('edit'), disabled('edit'), disabled('fixup'), disabled('squashInto'), disabled('drop'), disabled('squash'), disabled('rebase'), disabled('push'),
    separator('branch'), { id: 'branch', label: t('branch'), children: branches, disabled: !branches.length },
    { id: 'newBranch', label: t('newBranch'), disabled: !handlers, run: handlers ? () => handlers.newFrom(commit.hash) : undefined }, disabled('tag'),
    separator('navigate'), disabled('child'), { id: 'parent', label: t('parent'), disabled: !commit.parents.length || !parentAvailable, run: actions.parent },
  ];
}
