import type { RepoPath } from '../browse/types';
export type Inclusion = 'all' | 'some' | 'none';
export interface Rule { id: string; included: boolean }
export interface Hunks { path: RepoPath; token: string; blocks: number[] }
export interface Selection { rules: Rule[]; hunks: Hunks[] }
export type CommitResult = { kind: 'committed'; hash: string } | { kind: 'recovery'; hash: string; directory: string; instructions: string; reason: string };
export const emptySelection = (): Selection => ({ rules: [], hunks: [] });
export const pathKey = (path: RepoPath) => path.bytes.map(b => b.toString(16).padStart(2, '0')).join('');
export function matches(id: string, path: RepoPath, untracked: boolean) {
  const group = untracked ? 'unversioned' : 'changes', file = untracked ? 'untracked' : 'change';
  return id === group || id === `${file}/${pathKey(path)}` || (id.startsWith(`${group}/dir/`) && pathKey(path).startsWith(id.slice(`${group}/dir/`.length)));
}
export function toggleRule(selection: Selection, id: string, state: Inclusion): Selection {
  const group = id.startsWith('untracked/') || id.startsWith('unversioned');
  return { rules: [...selection.rules.filter(r => r.id !== id), { id, included: state !== 'all' }],
    hunks: selection.hunks.filter(h => !matches(id, h.path, group)) };
}
export function included(selection: Selection, path: RepoPath, untracked = false): boolean {
  return [...selection.rules].reverse().find(r => matches(r.id, path, untracked))?.included ?? !untracked;
}
export function blockSelection(selection: Selection, path: RepoPath, token: string, count: number, untracked = false): number[] {
  const existing = selection.hunks.find(h => pathKey(h.path) === pathKey(path));
  // A changed preview never silently retargets an old block selection.
  if (existing) return existing.token === token ? existing.blocks : [];
  return included(selection, path, untracked) ? Array.from({ length: count }, (_, i) => i) : [];
}
export function toggleBlock(selection: Selection, path: RepoPath, token: string, count: number, index: number, untracked = false): Selection {
  const blocks = new Set(blockSelection(selection, path, token, count, untracked));
  if (blocks.has(index)) blocks.delete(index); else blocks.add(index);
  const hunks = selection.hunks.filter(h => pathKey(h.path) !== pathKey(path));
  // Whole inclusion consumes staging intentionally; partial inclusion preserves disjoint staging.
  if (blocks.size === count) return { rules: [...selection.rules, { id: `change/${pathKey(path)}`, included: true }], hunks };
  return { rules: selection.rules, hunks: [...hunks, { path, token, blocks: [...blocks].sort((a,b) => a-b) }] };
}
export function canCommit(message: string, count: number, amend: boolean, busy: boolean) { return !busy && !!message.trim() && (count > 0 || amend); }
export function amendMessage(message: string, draft: string, enabled: boolean, last: string) { return enabled ? { message: last, draft: message } : { message: draft, draft }; }
export function remember(history: readonly string[], message: string) { return message.trim() ? [message, ...history.filter(m => m !== message)].slice(0,25) : [...history]; }
export function feedback(result: CommitResult | null, message: string) {
  return result ? { message: '', success: result.kind === 'committed', recovery: result.kind === 'recovery' ? result : null } : { message, success: false, recovery: null };
}
