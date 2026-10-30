import type { PopupItem } from '../../components/popup';
import type { Reference, References } from '../browse/types';

export type CheckoutMode = 'normal' | 'smart' | 'force';
export type UpdateStrategy = 'merge' | 'rebase';
export interface PushOptions { remote: string | null; branch: string | null; forceWithLease: boolean; tags: boolean }
export interface PushPreview { remote: string; branch: string; commits: string[]; total: number; start: number }
export interface PushRef { flag: string; source: string; target: string; summary: string; rejection?: string | null }
export type RemoteOutcome =
  | { kind: 'completed' }
  | { kind: 'push'; success: boolean; refs: PushRef[] }
  | { kind: 'conflicts'; files: { display: string }[]; stash?: string | null }
  | { kind: 'stashRecovery'; stash: string; files: { display: string }[] }
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

function branchActions(ref: Reference, refs: References, handlers: BranchPopupHandlers): PopupItem {
  const name = ref.name.startsWith('refs/heads/') ? localName(ref.name) : remoteName(ref.name);
  const local = ref.name.startsWith('refs/heads/');
  const current = ref.name === currentName(refs);
  const checkout = local ? () => handlers.checkout(name) : () => {
    const [remote, ...parts] = name.split('/'); handlers.checkoutRemote(remote, parts.join('/'));
  };
  const children: PopupItem[] = [
    { id: `${ref.name}:checkout`, label: current ? 'Checkout (current branch)' : 'Checkout', disabled: current, run: checkout },
    { id: `${ref.name}:new`, label: `New Branch from '${name}'…`, run: () => handlers.newFrom(name) },
    { id: `${ref.name}:checkout-rebase`, label: 'Checkout and Rebase onto Current (P1)', disabled: true },
    { id: `${ref.name}:compare`, label: 'Compare with Current (P1)', disabled: true },
    { id: `${ref.name}:diff`, label: 'Show Diff with Working Tree (P1)', disabled: true },
    { id: `${ref.name}:separator`, label: '', kind: 'separator' },
    { id: `${ref.name}:rebase`, label: 'Rebase / Merge (P1)', disabled: true },
    { id: `${ref.name}:update`, label: 'Update', disabled: !ref.upstream, run: () => handlers.update() },
    { id: `${ref.name}:push`, label: 'Push…', run: () => handlers.push(local ? { remote: null, branch: name, forceWithLease: false, tags: false } : null) },
  ];
  if (local) {
    children.push(
      { id: `${ref.name}:rename`, label: 'Rename…', run: () => handlers.rename(name) },
      { id: `${ref.name}:delete`, label: current ? 'Delete (current branch)' : 'Delete…', disabled: current, run: () => handlers.delete(name, false) },
    );
  }
  return { id: ref.name, label: `${name}${ref.track ? ` · ${ref.track}` : ''}`, children };
}

export interface BranchPopupHandlers {
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

export function branchPopupItems(refs: References, handlers: BranchPopupHandlers): PopupItem[] {
  const local = refs.local.slice().sort((a, b) => a.name.localeCompare(b.name));
  const remoteGroups = Object.entries(refs.remotes).sort(([a], [b]) => a.localeCompare(b));
  const recent = local.filter(ref => ref.name === currentName(refs) || ref.track.includes('ahead') || ref.track.includes('behind')).slice(0, 5);
  const recentIds = new Set(recent.map(ref => ref.name));
  const result: PopupItem[] = [
    { id: 'action:new', label: 'New Branch…', run: handlers.newBranch },
    { id: 'action:checkout-revision', label: 'Checkout Tag or Revision… (P1)', disabled: true },
    { id: 'action:separator', label: '', kind: 'separator' },
    { id: 'action:update', label: 'Update Project…', run: handlers.update },
    { id: 'action:commit', label: 'Commit…', run: handlers.commit },
    { id: 'action:push', label: 'Push…', run: () => handlers.push(null) },
    { id: 'action:fetch', label: 'Fetch', run: handlers.fetch },
    { id: 'heading:recent', label: 'Recent', kind: 'heading' },
    ...recent.map(ref => branchActions(ref, refs, handlers)),
    { id: 'heading:local', label: 'Local', kind: 'heading' },
    ...local.filter(ref => !recentIds.has(ref.name)).map(ref => branchActions(ref, refs, handlers)),
  ];
  for (const [remote, values] of remoteGroups) {
    result.push({ id: `heading:remote:${remote}`, label: `Remote · ${remote}`, kind: 'heading' });
    result.push(...values.slice().sort((a, b) => a.name.localeCompare(b.name)).map(ref => branchActions(ref, refs, handlers)));
  }
  if (refs.tags.length) {
    result.push({ id: 'heading:tags', label: 'Tags', kind: 'heading' });
    result.push(...refs.tags.slice().sort((a, b) => a.name.localeCompare(b.name)).map(ref => branchActions(ref, refs, handlers)));
  }
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
    : error.key === 'remote.hostKey' ? 'Host key verification failed'
    : error.key === 'remote.unreachable' ? 'Remote is unreachable'
    : error.key === 'remote.timeout' ? 'Remote operation timed out'
    : error.key === 'remote.cancelled' ? 'Remote operation cancelled' : 'Remote operation failed';
  const action = error.guidance === 'remote.tryFetchInTerminal' ? 'Run git fetch in a terminal, save credentials or unlock the key, then retry.' : 'Check the remote configuration and Git output, then retry.';
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

