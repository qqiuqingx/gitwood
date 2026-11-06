import type { PopupItem } from '../../components/popup';
import { hostingText, type HostingMessage } from '../../i18n/hosting';
import type { HostedRemote } from '../browse/types';

export type HostingTarget =
  | { kind: 'file'; revision: string; path: import('../browse/types').RepoPath; start?: number; end?: number }
  | { kind: 'commit'; revision: string }
  | { kind: 'branch'; branch: string }
  | { kind: 'createRequest'; base?: string; head: string };

export function hostingMenuItems(
  remotes: HostedRemote[], target: HostingTarget, open: (remote: string, target: HostingTarget) => void,
  copy: (remote: string, target: HostingTarget) => void,
  text: (key: HostingMessage) => string = hostingText,
): PopupItem[] {
  return (['github', 'gitlab'] as const).flatMap(platform => {
    const matching = remotes.filter(remote => remote.platform === platform);
    if (!matching.length) return [];
    return [{
      id: `hosting:${platform}`, label: text(platform),
      children: matching.map(remote => ({
        id: `hosting:${platform}:${remote.name}`, label: remote.name,
        children: [
          { id: `hosting:open:${remote.name}`, label: text('open'), run: () => open(remote.name, target) },
          { id: `hosting:copy:${remote.name}`, label: text('copy'), run: () => copy(remote.name, target) },
        ],
      })),
    }];
  });
}
