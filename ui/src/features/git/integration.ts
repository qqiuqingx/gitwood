import { integrationText as t, type IntegrationMessage } from '../../i18n/integration';
import type { RemoteError } from './model';
import type { References } from '../browse/types';
export interface MergeOptions { noFf: boolean; ffOnly: boolean; squash: boolean; noCommit: boolean; message: string | null }
export type MergeFlag = keyof MergeOptions;
export type Integration =
  | { kind: 'merge'; target: string; options: MergeOptions }
  | { kind: 'rebase'; target: string; onto: string | null }
  | { kind: 'checkoutRebase'; branch: string; localName: string | null }
  | { kind: 'pull'; remote: string; branches: string[]; rebase: boolean; options: MergeOptions };
export const emptyMergeOptions = (): MergeOptions => ({ noFf: false, ffOnly: false, squash: false, noCommit: false, message: null });
const incompatible: Record<MergeFlag, MergeFlag[]> = {
  noFf: ['ffOnly', 'squash'], ffOnly: ['noFf', 'squash', 'message'],
  squash: ['noFf', 'ffOnly', 'message'], message: ['ffOnly', 'squash', 'noCommit'], noCommit: ['message'],
};
export function mergeOptionEnabled(options: MergeOptions, option: MergeFlag, pull = false, rebase = false): boolean {
  return !rebase && !(pull && option === 'message') && incompatible[option].every(key => key === 'message' ? options.message === null : !options[key]);
}
export function validMergeOptions(options: MergeOptions): boolean {
  return (Object.keys(options) as MergeFlag[]).every(key => (key === 'message' ? options.message === null : !options[key]) || mergeOptionEnabled(options, key)) && (options.message === null || !!options.message.trim());
}
export function integrationEnabled(refs: References | null, operation: unknown, busy: boolean, conflicts = 0, recovery: unknown = null): boolean {
  return !!refs?.head.head && refs.head.head !== '(detached)' && refs.head.oid !== '(initial)' && !operation && !busy && !conflicts && !recovery;
}
export function pullDefaults(refs: References) {
  const current = refs.local.find(ref => ref.name === `refs/heads/${refs.head.head}`);
  const remotes = Object.keys(refs.remotes).sort((a,b) => b.length - a.length);
  const trackedRemote = remotes.find(name => current?.upstream?.startsWith(`refs/remotes/${name}/`));
  const trackedBranch = trackedRemote ? current?.upstream?.slice(`refs/remotes/${trackedRemote}/`.length) : undefined;
  const remote = trackedRemote ?? (refs.remotes.origin ? 'origin' : remotes[0] ?? '');
  const branches = (refs.remotes[remote] ?? []).map(ref => ref.name.slice(`refs/remotes/${remote}/`.length));
  const branch = trackedBranch && branches.includes(trackedBranch) ? trackedBranch : branches.includes(refs.head.head ?? '') ? refs.head.head! : '';
  return { remote, branches: branch ? [branch] : [] };
}

export function integrationErrorText(error: RemoteError): string {
  const detail = error.detail ?? String(error);
  const key = /(?:integration|remote)\.(invalidTarget|incompatibleOptions|messageRequired|branchRequired|currentBranch|localBranchExists|fetchChanged|operationInProgress|stashRecoveryRequired|concurrentChange|indexLocked)\b/.exec(detail)?.[1];
  return key ? t(key as IntegrationMessage) : detail;
}
