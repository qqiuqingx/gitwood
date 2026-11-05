import type { RepoPath } from '../browse/types';
export interface ConflictFile { path: RepoPath; conflictType: string; binary: boolean; large?: boolean; submodule: boolean }
export interface MergeBlock { from: number; to: number; left: string; right: string; base: string; resolved?: boolean }
export interface MergeContent { fingerprint: string; left: string; right: string; result: string; blocks: MergeBlock[]; readOnly: 'binaryOrLarge' | 'unsupported' | 'encoding' | null }
export type Choice = 'left' | 'right' | 'both' | 'ignore';
export const replacement = (block: MergeBlock, side: Choice) => side === 'both' ? block.left + block.right : side === 'ignore' ? null : block[side];
export const needsCloseConfirmation = (blocks: readonly MergeBlock[], dirty: boolean) => dirty || blocks.some(b => !b.resolved);
export const requestResolution = () => window.dispatchEvent(new Event('resolve-conflicts'));
