export interface RepoPath { bytes: number[]; display: string }
export interface CurrentFileSelection { path: RepoPath; revision: string; source: 'changes' | 'commit'; versioned: boolean; untracked: boolean; conflicted: boolean; regularFile?: boolean }
export interface NativeMenuActions { hasRepository: boolean; actions: Record<string, boolean> }
export interface Branch { oid: string | null; head: string | null; upstream: string | null; ahead: number; behind: number }
export interface Entry { path: RepoPath; original: RepoPath | null; xy: string; kind: string }
export interface Status { branch: Branch; staged: Entry[]; unstaged: Entry[]; untracked: Entry[]; conflicts: Entry[] }
export interface Reference { name: string; hash: string; peeled: string | null; upstream: string | null; track: string }
export interface References { local: Reference[]; remotes: Record<string, Reference[]>; tags: Reference[]; remote_heads: Record<string, string>; head: Branch }
export interface HostedRemote { name: string; platform: 'github' | 'gitlab' }
export interface Commit { hash: string; parents: string[]; author: string; email: string; author_time: number; commit_time: number; subject: string; message: string; refs: string }
export interface FileChange { status: string; path: RepoPath; original: RepoPath | null }
export interface FilePage { total: number; start: number; files: FileChange[] }
export type Segment = [edge: number, from: number, to: number, color: number, direction: number, dotted: boolean];
export interface Rows { generation: number; nodes: [row: number, column: number, color: number, start: number, end: number][]; segments: Segment[]; commits: Commit[]; current: boolean[] }
export interface HistoryRow { index: number; column: number; color: number; segments: Segment[]; commit: Commit; current: boolean }
export interface Filter { text: string | null; branch: string | null; author: string | null; since: string | null; until: string | null; path: RepoPath | null; regex: boolean; match_case: boolean }
export interface FilterInput { text: string; branch: string; author: string; since: string; until: string; path: string; regex: boolean; matchCase: boolean }
export interface Opened { generation: number; repository: { root: string; linked: boolean } }
export interface Recent { path: string; displayPath: string; failed: boolean }
export interface Refresh { status: boolean; refs: boolean; history: boolean }
export interface RepositoryError { generation?: number; key?: string; path?: string; detail?: string }

export type OperationKind = 'merge' | 'rebase' | 'applyMailbox' | 'cherryPick' | 'revert' | 'bisect';
/** Merge / rebase / cherry-pick / revert / bisect in progress, read from the Git directory. */
export interface Operation { kind: OperationKind; branch: string | null; step: number | null; total: number | null }
export interface StatusSummary { update_stash?: string | null; conflict_files: import('../conflicts/model').ConflictFile[]; merge_message: string | null; fingerprint: string; branch: Branch; changes: number; staged: number; untracked: number; conflicts: number; operation: Operation | null }
