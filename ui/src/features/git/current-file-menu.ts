import type { PopupItem } from '../../components/popup';
import { currentFileText, type CurrentFileMessage } from '../../i18n/current-file';

export type CurrentFileAction = 'history' | 'annotate' | 'annotatePrevious' | 'compareRevision' | 'compareBranch';
export type CurrentFileAvailability = boolean | Partial<Record<CurrentFileAction, boolean>>;
export function currentFileItems(run: (action: CurrentFileAction) => void, text: (key: CurrentFileMessage) => string = currentFileText, enabled: CurrentFileAvailability = true, hosting: PopupItem[] = []): PopupItem[] {
  const isEnabled = (action: CurrentFileAction) => typeof enabled === 'boolean' ? enabled : enabled[action] === true;
  return [
    { id: 'Git.CurrentFile.ShowHistory', label: text('history'), disabled: !isEnabled('history'), run: () => run('history') },
    { id: 'Git.CurrentFile.Annotate', label: text('annotate'), disabled: !isEnabled('annotate'), run: () => run('annotate') },
    { id: 'Git.CurrentFile.CompareRevision', label: text('compareRevision'), disabled: !isEnabled('compareRevision'), run: () => run('compareRevision') },
    { id: 'Git.CurrentFile.CompareBranch', label: text('compareBranch'), disabled: !isEnabled('compareBranch'), run: () => run('compareBranch') },
    ...hosting,
  ];
}
