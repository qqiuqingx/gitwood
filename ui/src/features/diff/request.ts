import type { RepoPath } from '../browse/types';
export interface SelectedFile { path: RepoPath; hash: string; staged: boolean; unstaged: boolean }
export function diffRequest(file: SelectedFile, generation: number, area = '') {
  return { command: file.hash ? 'commit_diff' : 'worktree_diff', args: { generation, path: file.path, hash: file.hash,
    area: area || (file.unstaged ? 'unstaged' : file.staged ? 'staged' : 'untracked') } };
}
export function preservedScroll(previousKey: string, nextKey: string, scroll: number) { return previousKey === nextKey ? scroll : 0; }
