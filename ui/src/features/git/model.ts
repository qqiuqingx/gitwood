import type { PopupItem } from '../../components/popup';
import type { Reference, References } from '../browse/types';
import { gitText } from '../../i18n/git';
import { integrationText as t } from '../../i18n/integration';

export type CheckoutMode = 'normal' | 'smart' | 'force';
export type UpdateStrategy = 'merge' | 'rebase';
export interface PushOptions { remote: string | null; branch: string | null; forceWithLease: boolean; tags: boolean }
export interface PushPreview { remote: string; branch: string; commits: string[]; subjects: string[]; total: number; start: number }
export interface PushRef { flag: string; source: string; target: string; summary: string; rejection?: string | null }
export type RemoteOutcome =
  | { kind: 'completed' }
  | { kind: 'push'; success: boolean; refs: PushRef[] }
  | { kind: 'conflicts'; files: { display: string }[]; stash?: string | null }
  | { kind: 'stashRecovery'; detail?: string; stash: string; files: { display: string }[] }
  | { kind: 'interrupted'; stash?: string | null; error: RemoteError };
export interface RemoteError { key?: string; detail?: string; guidance?: string }
export type CommitPushResult =
  | { kind: 'committed'; hash: string; push?: null }
  | { kind: 'recovery'; hash: string; directory: string; instructions: string; reason: string; push?: null }
  | { kind: 'committed'; hash: string; push: RemoteOutcome };

const localName = (name: string) => name.replace(/^refs\/heads\//, '');
const remoteName = (name: string) => name.replace(/^refs\/remotes\//, '');
const currentName = (refs: References) => refs.head.head ? `refs/heads/${refs.head.head}` : '';
const currentBranch = (refs: References) => refs.head.head && refs.head.head !== '(detached)' ? refs.head.head : null;

/** Counts from `%(upstream:track,nobracket)`, e.g. "ahead 2, behind 1". */
export function trackCounts(track: string): { ahead: number; behind: number } {
  return { ahead: Number(/ahead (\d+)/.exec(track)?.[1] ?? 0), behind: Number(/behind (\d+)/.exec(track)?.[1] ?? 0) };
}
/** IDEA marks the main/master branches as favorites by default; favorites cannot be edited yet. */
export const defaultFavorite = (name: string) => name === 'main' || name === 'master';

/** Branch row with its action submenu; `label` defaults to the full branch name. */
export function branchItem(ref: Reference, refs: References, handlers: BranchPopupHandlers, label?: string, level = 0): PopupItem {
  const name = ref.name.startsWith('refs/heads/') ? localName(ref.name) : remoteName(ref.name);
  const local = ref.name.startsWith('refs/heads/');
  const current = ref.name === currentName(refs);
  const branch = local || ref.name.startsWith('refs/remotes/');
  const canIntegrate = branch && !current && handlers.canIntegrate();
  const checkout = local ? () => handlers.checkout(name) : () => {
    const [remote, ...parts] = name.split('/'); handlers.checkoutRemote(remote, parts.join('/'));
  };
  const children: PopupItem[] = [
    { id: `${ref.name}:checkout`, label: gitText(current ? 'checkoutCurrent' : 'checkout'), disabled: current, run: checkout },
    { id: `${ref.name}:new`, label: gitText('newFrom').replace('{name}', name), run: () => handlers.newFrom(name) },
    { id: `${ref.name}:checkout-rebase`, label: t('checkoutRebase'), disabled: !canIntegrate, run: () => handlers.checkoutRebase(ref.name) },
    { id: `${ref.name}:compare`, label: t('compare'), disabled: !branch || current, run: () => handlers.compare(ref.name, false) },
    { id: `${ref.name}:diff`, label: t('diff'), disabled: !branch, run: () => handlers.compare(ref.name, true) },
    { id: `${ref.name}:separator`, label: '', kind: 'separator' },
    { id: `${ref.name}:merge`, label: t('mergeCurrent'), disabled: !canIntegrate, run: () => handlers.merge(ref.name) },
    { id: `${ref.name}:rebase`, label: t('rebaseCurrent'), disabled: !canIntegrate, run: () => handlers.rebase(ref.name) },
    { id: `${ref.name}:update`, label: gitText('updateBranch'), disabled: !ref.upstream, run: () => handlers.update() },
    { id: `${ref.name}:push`, label: gitText('push'), run: () => handlers.push(local ? { remote: null, branch: name, forceWithLease: false, tags: false } : null) },
  ];
  if (local) {
    children.push(
      { id: `${ref.name}:upstream`, label: t('setUpstream'), disabled: !handlers.canIntegrate(), run: () => handlers.upstream(name) },
      { id: `${ref.name}:unset-upstream`, label: t('unsetUpstream'), disabled: !ref.upstream || !handlers.canIntegrate(), run: () => handlers.unsetUpstream(name) },
      { id: `${ref.name}:rename`, label: gitText('rename'), run: () => handlers.rename(name) },
      { id: `${ref.name}:delete`, label: gitText(current ? 'deleteCurrent' : 'delete'), disabled: current, run: () => handlers.delete(name, false) },
    );
  }
  return { id: ref.name, label: label ?? name, level, children };
}

export const branchFolderPrefix = 'folder:';
/** Collapsible top-level groups (Recent, Local, Remote, Tags); expanded unless listed in the toggle set. */
export const branchSectionPrefix = 'section:';
/**
 * IDEA BranchesTree: `/` prefixes become expandable folders; within a level the current
 * branch and favorites come first, then folders, then the remaining branches.
 */
function branchTree(entries: { ref: Reference; name: string }[], refs: References, handlers: BranchPopupHandlers,
  expanded: ReadonlySet<string>, toggle: (id: string) => void, scope: string, level: number): PopupItem[] {
  const leaves: { ref: Reference; name: string }[] = [];
  const folders = new Map<string, { ref: Reference; name: string }[]>();
  for (const entry of entries) {
    const slash = entry.name.indexOf('/');
    if (slash < 0) leaves.push(entry);
    else folders.set(entry.name.slice(0, slash), [...(folders.get(entry.name.slice(0, slash)) ?? []), { ref: entry.ref, name: entry.name.slice(slash + 1) }]);
  }
  const rank = (entry: { ref: Reference }) => entry.ref.name === currentName(refs) ? 0 : defaultFavorite(localName(entry.ref.name)) ? 1 : 2;
  const pinned = leaves.filter(entry => rank(entry) < 2).sort((a, b) => rank(a) - rank(b));
  const result = pinned.map(entry => branchItem(entry.ref, refs, handlers, entry.name, level));
  for (const [folder, children] of [...folders].sort(([a], [b]) => a.localeCompare(b))) {
    const id = `${branchFolderPrefix}${scope}/${folder}`;
    result.push({ id, label: folder, level, keepOpen: true, run: () => toggle(id) });
    if (expanded.has(id)) result.push(...branchTree(children, refs, handlers, expanded, toggle, `${scope}/${folder}`, level + 1));
  }
  result.push(...leaves.filter(entry => rank(entry) === 2).map(entry => branchItem(entry.ref, refs, handlers, entry.name, level)));
  return result;
}

export interface BranchPopupHandlers {
  canIntegrate: () => boolean;
  merge: (ref: string) => void;
  rebase: (ref: string) => void;
  checkoutRebase: (ref: string) => void;
  compare: (ref: string, working: boolean) => void;
  upstream: (name: string) => void;
  unsetUpstream: (name: string) => void;
  checkout: (name: string) => void;
  checkoutRemote: (remote: string, branch: string) => void;
  newFrom: (name: string) => void;
  update: () => void;
  push: (options: PushOptions | null) => void;
  rename: (name: string) => void;
  delete: (name: string, force: boolean) => void;
  newBranch: () => void;
  commit: () => void;
  fetch: () => void;
}

export function branchPopupItems(refs: References, handlers: BranchPopupHandlers,
  expanded: ReadonlySet<string> = new Set(), toggle: (id: string) => void = () => {}): PopupItem[] {
  const local = refs.local.slice().sort((a, b) => a.name.localeCompare(b.name));
  const remoteGroups = Object.entries(refs.remotes).sort(([a], [b]) => a.localeCompare(b));
  const recent = local.filter(ref => ref.name === currentName(refs) || ref.track.includes('ahead') || ref.track.includes('behind')).slice(0, 5);
  const result: PopupItem[] = [
    { id: 'action:new', label: gitText('newBranch'), run: handlers.newBranch },
    { id: 'action:checkout-revision', label: gitText('checkoutRevision'), disabled: true },
    { id: 'action:separator', label: '', kind: 'separator' },
    { id: 'action:update', label: gitText('update'), run: handlers.update },
    { id: 'action:commit', label: gitText('commit'), run: handlers.commit },
    { id: 'action:push', label: gitText('push'), run: () => handlers.push(null) },
    { id: 'action:fetch', label: gitText('fetch'), run: handlers.fetch },
  ];
  // IDEA BranchesTree top-level nodes: expanded by default, folding their rows in place.
  const section = (key: 'recent' | 'local' | 'remote' | 'tags', rows: () => PopupItem[]) => {
    const id = `${branchSectionPrefix}${key}`;
    result.push({ id, label: gitText(key), keepOpen: true, run: () => toggle(id) });
    if (!expanded.has(id)) result.push(...rows());
  };
  if (recent.length) section('recent', () => recent.map(ref => ({ ...branchItem(ref, refs, handlers, undefined, 1), id: `recent:${ref.name}` })));
  section('local', () => branchTree(local.map(ref => ({ ref, name: localName(ref.name) })), refs, handlers, expanded, toggle, 'local', 1));
  if (remoteGroups.length) section('remote', () => remoteGroups.flatMap(([remote, values]) => {
    const id = `${branchFolderPrefix}remote/${remote}`;
    const folder: PopupItem = { id, label: remote, level: 1, keepOpen: true, run: () => toggle(id) };
    return expanded.has(id) ? [folder, ...branchTree(values.slice().sort((a, b) => a.name.localeCompare(b.name))
      .map(ref => ({ ref, name: remoteName(ref.name).slice(remote.length + 1) })), refs, handlers, expanded, toggle, `remote/${remote}`, 2)] : [folder];
  }));
  if (refs.tags.length) section('tags', () => refs.tags.slice().sort((a, b) => a.name.localeCompare(b.name))
    .map(ref => branchItem(ref, refs, handlers, ref.name.replace(/^refs\/tags\//, ''), 1)));
  return result;
}

export function defaultPushOptions(refs: References): PushOptions {
  const head = refs.head.head;
  const local = refs.local.find(ref => ref.name === `refs/heads/${head}`);
  const upstream = local?.upstream?.match(/^refs\/remotes\/([^/]+)\/(.+)$/);
  return { remote: upstream?.[1] ?? null, branch: upstream?.[2] ?? head ?? null, forceWithLease: false, tags: false };
}

export function defaultUpdateStrategy(configured?: string | null): UpdateStrategy {
  return configured === 'rebase' || configured === 'true' ? 'rebase' : 'merge';
}
export function updateRequest(strategy: UpdateStrategy): { strategy: UpdateStrategy } {
  return { strategy };
}

export function pushPageRanges(total: number, pageSize = 200): Array<{ start: number; limit: number }> {
  const ranges: Array<{ start: number; limit: number }> = [];
  for (let start = 0; start < total; start += pageSize) ranges.push({ start, limit: Math.min(pageSize, total - start) });
  return ranges;
}

export function remoteErrorText(error: RemoteError): { title: string; detail: string; action: string } {
  const title = error.key === 'remote.authentication' ? 'Authentication required'
    : error.key === 'remote.authCancelled' ? 'Authentication cancelled'
    : error.key === 'remote.hostKey' ? 'Host key verification failed'
    : error.key === 'remote.unreachable' ? 'Remote is unreachable'
    : error.key === 'remote.timeout' ? 'Remote operation timed out'
    : error.key === 'remote.cancelled' ? 'Remote operation cancelled' : 'Remote operation failed';
  const action = error.guidance === 'remote.checkCredentials' ? 'Check the credential helper or SSH agent, then retry.'
    : error.guidance === 'remote.tryFetchInTerminal' ? 'Run git fetch in a terminal, save credentials or unlock the key, then retry.'
    : 'Check the remote configuration and Git output, then retry.';
  return { title, detail: error.detail ?? '', action };
}

export function pushRejectionText(rejection?: string | null): string {
  return rejection === 'needsUpdate' ? 'Push rejected: update first.'
    : rejection === 'remoteRule' ? 'Push rejected by the remote hook.'
    : rejection === 'staleLease' ? 'Push rejected: the remote changed; review before retrying.'
    : 'Push rejected.';
}

export function commitPushFeedback(result: CommitPushResult): { commit: string; push: 'not-run' | 'success' | 'failed' | 'recovery' } {
  if (result.kind === 'recovery') return { commit: result.hash, push: 'recovery' };
  if (!result.push) return { commit: result.hash, push: 'not-run' };
  return { commit: result.hash, push: result.push.kind === 'push' && result.push.success ? 'success' : result.push.kind === 'push' ? 'failed' : 'failed' };
}

export function branchForPreview(refs: References | null, options: PushOptions): string | null {
  if (options.branch) return options.branch;
  return refs ? currentBranch(refs) : null;
}

export function isNewPushTarget(refs: References | null, options: PushOptions): boolean {
  if (!refs || !options.remote || !options.branch) return false;
  return !refs.remotes[options.remote]?.some(ref => ref.name === `refs/remotes/${options.remote}/${options.branch}`);
}

