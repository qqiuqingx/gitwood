import type { PopupItem } from '../../components/popup';
import { currentFileText, type CurrentFileMessage } from '../../i18n/current-file';

export type CurrentFileAction = 'history' | 'annotate' | 'annotatePrevious' | 'compareRevision' | 'compareBranch';
export function currentFileItems(run: (action: CurrentFileAction) => void, text: (key: CurrentFileMessage) => string = currentFileText, enabled = true): PopupItem[] {
  return [
    { id: 'Git.CurrentFile.ShowHistory', label: text('history'), disabled: !enabled, run: () => run('history') },
    { id: 'Git.CurrentFile.Annotate', label: text('annotate'), disabled: !enabled, run: () => run('annotate') },
    { id: 'Git.CurrentFile.CompareRevision', label: text('compareRevision'), disabled: !enabled, run: () => run('compareRevision') },
    { id: 'Git.CurrentFile.CompareBranch', label: text('compareBranch'), disabled: !enabled, run: () => run('compareBranch') },
  ];
}
