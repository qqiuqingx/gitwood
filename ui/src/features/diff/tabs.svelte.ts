import type { RepoPath } from '../browse/types';

export type DiffTabSource = 'worktree' | 'commit';
export type DiffArea = 'commit' | 'staged' | 'unstaged';
export interface DiffTabInput {
  source: DiffTabSource;
  path: RepoPath;
  /** Commit OID for history diffs; empty for a live worktree diff. */
  hash: string;
  /** Revision identity, deliberately independent of the repository generation. */
  revision: string;
  leftRevision: string;
  rightRevision: string;
  staged: boolean;
  unstaged: boolean;
  area: DiffArea;
}
export interface DiffTab extends DiffTabInput {
  id: string;
  preview: boolean;
  position: { left: number; right: number; selected: number };
  empty: boolean;
  error: string;
}
/** `recent` lists tab IDs most recently activated first (IDEA's editor history order). */
export interface DiffTabsState { tabs: DiffTab[]; activeId: string; nextId: number; recent?: string[] }
export const emptyDiffTabs = (): DiffTabsState => ({ tabs: [], activeId: '', nextId: 1, recent: [] });
/** IDEA's default `editorTabLimit`; the persisted setting overrides it. */
export const DEFAULT_DIFF_TAB_LIMIT = 10;

export function diffTabKey(tab: DiffTabInput): string {
  return JSON.stringify([tab.source, tab.revision, tab.hash, tab.path.bytes, tab.area]);
}
/** The renderer receives only the selected tab, so inactive editors are always destroyed. */
export function activeDiffTabs(tabs: readonly DiffTab[], activeId: string): DiffTab[] {
  const active = tabs.find(tab => tab.id === activeId);
  return active ? [active] : [];
}
function createTab(input: DiffTabInput, id: string, preview: boolean): DiffTab {
  return { ...input, id, preview, position: { left: 0, right: 0, selected: -1 }, empty: false, error: '' };
}
function ordered(tabs: readonly DiffTab[]): DiffTab[] {
  return [...tabs.filter(tab => !tab.preview), ...tabs.filter(tab => tab.preview)];
}

export function previewDiff(state: DiffTabsState, input: DiffTabInput): DiffTabsState {
  const existing = state.tabs.find(tab => !tab.preview && diffTabKey(tab) === diffTabKey(input));
  if (existing) return { ...state, activeId: existing.id };
  const preview = state.tabs.find(tab => tab.preview);
  const next = preview
    ? state.tabs.map(tab => tab.id === preview.id ? createTab(input, tab.id, true) : tab)
    : [...state.tabs, createTab(input, `diff-${state.nextId}`, true)];
  return { tabs: ordered(next), activeId: preview?.id ?? `diff-${state.nextId}`, nextId: preview ? state.nextId : state.nextId + 1 };
}

export function openDiff(state: DiffTabsState, input: DiffTabInput): DiffTabsState {
  const key = diffTabKey(input);
  const fixed = state.tabs.find(tab => !tab.preview && diffTabKey(tab) === key);
  if (fixed) return { ...state, activeId: fixed.id };
  const preview = state.tabs.find(tab => tab.preview && diffTabKey(tab) === key);
  if (preview) {
    const tabs = ordered(state.tabs.map(tab => tab.id === preview.id ? { ...tab, preview: false } : tab));
    return { ...state, tabs, activeId: preview.id };
  }
  const id = `diff-${state.nextId}`;
  return { ...state, tabs: ordered([...state.tabs, createTab(input, id, false)]), activeId: id, nextId: state.nextId + 1 };
}

export function pinDiff(state: DiffTabsState, id: string): DiffTabsState {
  const tab = state.tabs.find(entry => entry.id === id);
  if (!tab) return state;
  if (!tab.preview) return { ...state, activeId: id };
  const duplicate = state.tabs.find(entry => !entry.preview && diffTabKey(entry) === diffTabKey(tab));
  if (duplicate) return { ...state, tabs: state.tabs.filter(entry => entry.id !== id), activeId: duplicate.id };
  return { ...state, tabs: ordered(state.tabs.map(entry => entry.id === id ? { ...entry, preview: false } : entry)), activeId: id };
}

export function closeDiff(state: DiffTabsState, id: string): DiffTabsState {
  const index = state.tabs.findIndex(tab => tab.id === id);
  if (index < 0) return state;
  const tabs = state.tabs.filter(tab => tab.id !== id);
  if (state.activeId !== id) return { ...state, tabs };
  // IDEA defaults to the previous neighbour; at the left edge it selects the right neighbour.
  const nextIndex = index > 0 ? index - 1 : 0;
  return { ...state, tabs, activeId: tabs.length ? tabs[nextIndex].id : '' };
}

export function changeDiffArea(state: DiffTabsState, id: string, area: DiffArea): DiffTabsState {
  const current = state.tabs.find(tab => tab.id === id);
  if (!current || current.source !== 'worktree' || current.area === area) return state;
  const labels = area === 'staged' ? ['HEAD', 'Staged'] : area === 'unstaged' ? ['Staged', 'Working tree'] : ['HEAD', 'Working tree'];
  const changed = { ...current, area, leftRevision: labels[0], rightRevision: labels[1],
    position: { left: 0, right: 0, selected: -1 }, empty: false, error: '' };
  const duplicate = state.tabs.find(tab => tab.id !== id && diffTabKey(tab) === diffTabKey(changed));
  if (duplicate) return { ...state, activeId: duplicate.id };
  return { ...state, tabs: state.tabs.map(tab => tab.id === id ? changed : tab), activeId: id };
}

/** Record the active tab as most recently used. */
export function touchDiffTab(state: DiffTabsState): DiffTabsState {
  const live = (state.recent ?? []).filter(id => id !== state.activeId && state.tabs.some(tab => tab.id === id));
  return { ...state, recent: state.activeId ? [state.activeId, ...live] : live };
}

/**
 * Close tabs beyond `limit`, never the active one. Mirrors IDEA `EditorWindow.trimToSize`:
 * tabs without history go first, then the least recently used.
 */
export function trimDiffTabs(state: DiffTabsState, limit: number): DiffTabsState {
  const recent = (state.recent ?? []).filter(id => state.tabs.some(tab => tab.id === id));
  const order = [...state.tabs.filter(tab => !recent.includes(tab.id)).map(tab => tab.id), ...[...recent].reverse()];
  let tabs = state.tabs;
  for (const id of order) {
    if (tabs.length <= Math.max(1, limit)) break;
    if (id !== state.activeId) tabs = tabs.filter(tab => tab.id !== id);
  }
  return { ...state, tabs, recent: recent.filter(id => tabs.some(tab => tab.id === id)) };
}

export class DiffTabsModel {
  tabs = $state<DiffTab[]>([]);
  activeId = $state('');
  private nextId = 1;
  private recent: string[] = [];
  private limit = DEFAULT_DIFF_TAB_LIMIT;
  private apply(state: DiffTabsState) {
    const next = trimDiffTabs(touchDiffTab(state), this.limit);
    this.tabs = next.tabs;
    this.activeId = next.activeId;
    this.nextId = next.nextId;
    this.recent = next.recent ?? [];
  }
  setLimit(limit: number) {
    if (limit === this.limit) return;
    this.limit = limit;
    this.apply(this.snapshot());
  }
  preview(input: DiffTabInput) { this.apply(previewDiff(this.snapshot(), input)); }
  open(input: DiffTabInput) { this.apply(openDiff(this.snapshot(), input)); }
  pin(id: string) { this.apply(pinDiff(this.snapshot(), id)); }
  close(id: string) { this.apply(closeDiff(this.snapshot(), id)); }
  setArea(id: string, area: DiffArea) { this.apply(changeDiffArea(this.snapshot(), id, area)); }
  select(id: string) { if (this.tabs.some(tab => tab.id === id)) this.apply({ ...this.snapshot(), activeId: id }); }
  update(id: string, update: Partial<Pick<DiffTab, 'empty' | 'error'>>) {
    const tab = this.tabs.find(entry => entry.id === id);
    if (tab) Object.assign(tab, update);
  }
  clear() { this.tabs = []; this.activeId = ''; this.nextId = 1; this.recent = []; }
  private snapshot(): DiffTabsState { return { tabs: this.tabs, activeId: this.activeId, nextId: this.nextId, recent: this.recent }; }
}
