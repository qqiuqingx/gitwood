import type { PopupItem } from '../../components/popup';
import type { Commit, References } from './types';
import { branchPopupItems, type BranchPopupHandlers } from '../git/model';
import type { HistoryMenuMessage } from '../../i18n/history-menu';

export interface UndoInfo { branches: string[]; total: number; protected: boolean; inProgress: boolean }
export function undoReason(commit: Commit, refs: References | null, info: UndoInfo | null): HistoryMenuMessage | null {
  if (!refs) return 'undoLoading';
  if (!refs.head.oid || refs.head.oid !== commit.hash) return 'undoHead';
  if (commit.parents.length > 1) return 'undoMerge';
  if (!commit.parents.length) return 'undoRoot';
  if (!info) return 'undoLoading';
  if (info.inProgress) return 'undoState';
  if (info.protected || info.branches.some(name => /^refs\/remotes\/[^/]+\/(main|master)$/.test(name))) return 'undoProtected';
  return null;
}

export function historyMenuItems(commit: Commit, refs: References | null, handlers: BranchPopupHandlers | undefined,
  parentAvailable: boolean, actions: { copy: () => void; parent: () => void }, t: (key: HistoryMenuMessage) => string, undo?: { info: UndoInfo | null; run: () => void; busy: boolean }): PopupItem[] {
  const branchIds = new Set(refs ? [...refs.local, ...Object.values(refs.remotes).flat()].filter(ref => ref.hash === commit.hash).map(ref => ref.name) : []);
  const branches = refs && handlers ? branchPopupItems(refs, handlers).filter(item => branchIds.has(item.id)) : [];
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
