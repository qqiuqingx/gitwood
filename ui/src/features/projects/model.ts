import type { PopupItem } from '../../components/popup';
import type { Recent } from '../browse/types';
import { projectName } from './avatar';
export interface ProjectPopupItem extends PopupItem { avatar?: { path: string; name: string; failed: boolean } }
export function projectPopupItems(current: string | undefined, recent: readonly Recent[],
  labels: { new: string; open: string; clone: string; opened: string; recent: string; failed: string },
  open: () => void, select: (path: string) => void): ProjectPopupItem[] {
  const project = (path: string, description: string, failed = false): ProjectPopupItem => ({
    id: path, label: projectName(path), description: description + (failed ? ` (${labels.failed})` : ''),
    avatar: { path, name: projectName(path), failed }, run: () => select(path),
  });
  return [
    { id: 'new', label: labels.new, disabled: true }, { id: 'open', label: labels.open, run: open },
    { id: 'clone', label: labels.clone, disabled: true },
    ...(current ? [{ id: 'opened', label: labels.opened, kind: 'heading' as const, groupHeader: true, firstGroup: true },
      project(current, recent.find(item => item.path === current)?.displayPath ?? current)] : []),
    { id: 'recent', label: labels.recent, kind: 'heading', groupHeader: true, firstGroup: !current },
    ...recent.filter(item => item.path !== current).map(item => project(item.path, item.displayPath, item.failed)),
  ];
}
