import type { Commit, Operation, References } from './types';

/**
 * Shared availability of commit-editing actions (IDEA GitUncommitAction and friends):
 * repository state comes from Rust (`containing_branches`, `operation_state`) when the menu opens;
 * Rust re-checks every condition before writing.
 */
export interface EditInfo { branches: string[]; total: number; protected: boolean; operation: Operation | null }
export interface EditRequirements { head?: boolean; nonMerge?: boolean; nonRoot?: boolean; unprotected?: boolean }
export type EditReason = 'loading' | 'head' | 'merge' | 'root' | 'state' | 'protected';

/** IDEA GitSharedSettings default protected branches: main and master on any remote. */
export const protectedRemoteBranch = (name: string) => /^refs\/remotes\/[^/]+\/(main|master)$/.test(name);

export function historyEditReason(commit: Commit, refs: References | null, info: EditInfo | null, required: EditRequirements): EditReason | null {
  if (!refs) return 'loading';
  if (required.head && (!refs.head.oid || refs.head.oid !== commit.hash)) return 'head';
  if (required.nonMerge && commit.parents.length > 1) return 'merge';
  if (required.nonRoot && !commit.parents.length) return 'root';
  if (!info) return 'loading';
  if (info.operation && info.operation.kind !== 'bisect') return 'state';
  if (required.unprotected && (info.protected || info.branches.some(protectedRemoteBranch))) return 'protected';
  return null;
}
