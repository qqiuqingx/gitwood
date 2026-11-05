import type { PopupItem } from '../../components/popup';
import { nativeMenuText, type NativeMenuMessage } from '../../i18n/native-menu';

export const vcsOperationActionIds = [
  'CheckinProject', 'ChangesView.ToggleCommitUi', 'ChangesView.CommitFile', 'ChangesView.Revert',
  'Git.CurrentFile.ShowHistory', 'Git.CurrentFile.Annotate', 'Git.CurrentFile.CompareRevision',
  'Git.Branches', 'Vcs.Push', 'Git.StashChanges', 'Git.UnstashChanges', 'ChangesView.AddUnversioned',
  'Git.ResolveConflicts',
] as const;

type Entry = { id: typeof vcsOperationActionIds[number]; label: NativeMenuMessage } | { kind: 'separator' } | { kind: 'heading'; label: NativeMenuMessage };

const entries: readonly Entry[] = [
  { kind: 'heading', label: 'gitGroup' },
  { id: 'CheckinProject', label: 'commitAction' },
  { id: 'ChangesView.ToggleCommitUi', label: 'toggleCommitUi' },
  { id: 'ChangesView.CommitFile', label: 'commitFile' },
  { id: 'ChangesView.Revert', label: 'rollback' },
  { kind: 'separator' },
  { id: 'Git.CurrentFile.ShowHistory', label: 'showHistory' },
  { id: 'Git.CurrentFile.Annotate', label: 'annotate' },
  { id: 'Git.CurrentFile.CompareRevision', label: 'compareRevision' },
  { kind: 'separator' },
  { id: 'Git.Branches', label: 'branches' },
  { id: 'Vcs.Push', label: 'push' },
  { id: 'Git.StashChanges', label: 'stash' },
  { id: 'Git.UnstashChanges', label: 'unstash' },
  { kind: 'separator' },
  { id: 'ChangesView.AddUnversioned', label: 'addToVcs' },
  { id: 'Git.ResolveConflicts', label: 'resolveConflicts' },
];

function quickKey(index: number): string {
  return index < 9 ? String(index + 1) : String.fromCharCode(97 + index - 9);
}

export function vcsOperationsItems(
  enabled: Readonly<Record<string, boolean>>,
  run: (action: string) => void,
  text: (key: NativeMenuMessage) => string = key => nativeMenuText(key),
): PopupItem[] {
  let actionIndex = 0;
  return entries.map(entry => {
    if ('kind' in entry) {
      return entry.kind === 'separator'
        ? { id: `vcs-operations-separator-${actionIndex}`, label: '', kind: 'separator' }
        : { id: 'vcs-operations-git-heading', label: text(entry.label), kind: 'heading', groupHeader: true };
    }
    const key = quickKey(actionIndex++);
    return {
      id: entry.id,
      label: text(entry.label),
      shortcut: key,
      disabled: enabled[entry.id] !== true,
      run: () => run(entry.id),
    };
  });
}
