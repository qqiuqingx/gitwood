import type { RepoPath } from '../browse/types';
export interface SelectedFile { path: RepoPath; hash: string; staged: boolean; unstaged: boolean }
export function diffRequest(file: SelectedFile, generation: number, area = '') {
  return { command: file.hash ? 'commit_diff' : 'worktree_diff', args: { generation, path: file.path, hash: file.hash,
    area: area || (file.unstaged ? 'unstaged' : file.staged ? 'staged' : 'untracked') } };
}
/** Same file keeps its live scroll; otherwise fall back to the position saved for that tab (0 for a new file). */
export function preservedScroll(previousKey: string, nextKey: string, scroll: number, saved = 0) { return previousKey === nextKey ? scroll : saved; }
