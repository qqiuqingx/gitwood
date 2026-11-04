import { CommitSession } from '../commit/session.svelte';
import { tick } from 'svelte';
import { invoke } from '@tauri-apps/api/core';
import { buildFilter, emptyFilter, mergeRefresh, preserveViewport, RequestGate } from './model';
import type { Commit, FilterInput, HistoryRow, Opened, Recent, References, Refresh, RepositoryError, Rows, StatusSummary } from './types';

/** List row ID of a commit row that is not loaded yet. */
export const placeholder = 'row:';
/** Sorted row indices to half-open ranges. */
export function rowRanges(indices: readonly number[]): [number, number][] {
  const ranges: [number, number][] = [];
  for (const index of indices) {
    const last = ranges[ranges.length - 1];
    if (last && last[1] === index) last[1] = index + 1; else if (!last || index >= last[1]) ranges.push([index, index + 1]);
  }
  return ranges;
}
const nothing = (): Refresh => ({ status: false, refs: false, history: false });
const everything = (): Refresh => ({ status: true, refs: true, history: true });
export class RepositoryBrowser {
  commit = new CommitSession();
  previewFile = $state<{ path: import('./types').RepoPath; staged: boolean; unstaged: boolean; hash: string } | null>(null);
  project = $state<Opened['repository'] | null>(null);
  recent = $state<Recent[]>([]);
  generation = $state(0);
  status = $state<StatusSummary | null>(null);
  statusRevision = $state(0);
  statusGeneration = $state(0);
  refs = $state<References | null>(null);
  rows = $state.raw(new Map<number, HistoryRow>());
  total = $state(0);
  selected = $state('');
  /** Log multi-selection as commit IDs; `selected` is the lead row shown in the details. */
  selection = $state.raw<ReadonlySet<string>>(new Set());
  activeIndex = $state(-1);
  scrollTop = $state(0);
  rowHeight = 24;
  details = $state<Commit | null>(null);
  filter = $state<FilterInput>(emptyFilter());
  error = $state<RepositoryError | null>(null);
  opening = $state(false);
  loading = $state(false);
  private range = { start: 0, end: 40 };
  private pending = nothing();
  private draining = false;
  private session = new RequestGate();
  private pages = new RequestGate();
  private selectionGate = new RequestGate();
  private filterRevision = 0;
  private disposed = false;
  private operations: Promise<unknown> = Promise.resolve();

  async undoCommit(hash: string) {
    if (this.commit.busy || !this.project) return;
    const project = this.project, session = this.commit;
    session.busy = true;
    try {
      const message = await invoke<string>('undo_commit', { generation: this.generation, expected: hash });
      if (this.project === project && this.commit === session) session.restoreUndoMessage(message);
    } catch (error) { if (this.project === project) this.report(error); }
    finally { session.busy = false; }
    if (this.project === project) await this.refresh(everything());
  }
  async recentProjects() { this.recent = await invoke<Recent[]>('recent_repositories'); }
  async open(path: string) {
    if (this.commit.busy) return;
    this.commit = new CommitSession();
    const ticket = this.session.next();
    this.pages.invalidate(); this.selectionGate.invalidate();
    this.previewFile = null; this.project = null; this.generation = 0; this.opening = true; this.error = null;
    this.rows = new Map(); this.total = 0; this.status = null; this.statusRevision = 0; this.statusGeneration = 0; this.refs = null;
    this.selected = ''; this.selection = new Set(); this.activeIndex = -1; this.scrollTop = 0; this.details = null;
    this.filter = emptyFilter(); this.pending = nothing();
    try {
      // Discovery precedes backend session installation. Serialize open/close so
      // a slow earlier discovery can never replace a newer selected repository.
      const operation = this.operations.catch(() => {}).then(() => this.session.accepts(ticket)
        ? invoke<Opened>('open_repository', { path }) : null);
      this.operations = operation;
      const opened = await operation;
      if (!opened) return;
      if (!this.session.accepts(ticket) || this.disposed) return;
      this.project = opened.repository; this.generation = opened.generation;
      await this.refresh(everything());
    } catch (error) { if (this.session.accepts(ticket)) this.report(error); }
    finally { if (this.session.accepts(ticket)) { this.opening = false; void this.recentProjects(); } }
  }
  async close() {
    if (this.commit.busy) return;
    this.commit = new CommitSession();
    this.session.invalidate(); this.pages.invalidate(); this.selectionGate.invalidate();
    this.previewFile = null; this.project = null; this.generation = 0; this.rows = new Map(); this.selection = new Set(); this.status = null; this.statusRevision = 0; this.statusGeneration = 0; this.refs = null;
    this.details = null; this.pending = nothing(); this.opening = false;
    this.operations = this.operations.catch(() => {}).then(() => invoke('close_repository'));
    await this.operations;
  }
  dispose() { this.disposed = true; this.session.invalidate(); this.pages.invalidate(); this.selectionGate.invalidate(); }
  report(error: unknown) {
    const value: RepositoryError = typeof error === 'object' && error !== null ? error : { detail: String(error) };
    if (value.key !== 'repository.stale') this.error = value;
  }
  async applyFilter(input = this.filter) {
    if (JSON.stringify(input) === JSON.stringify(this.filter)) return;
    this.filter = { ...input }; this.filterRevision++;
    await this.refresh({ ...nothing(), history: true });
  }
  async refresh(areas: Refresh, eventGeneration = this.generation) {
    if (!this.project || this.disposed || eventGeneration < this.generation
      || (eventGeneration > this.generation && !this.draining)) return;
    this.pending = mergeRefresh(this.pending, areas);
    if (this.draining || this.commit.busy) return;
    this.draining = true;
    try {
      while (this.project && Object.values(this.pending).some(Boolean) && !this.disposed) {
        const work = this.pending; this.pending = nothing();
        const revision = this.filterRevision;
        const project: Opened['repository'] = this.project;
        try {
          if (work.history) await this.relayout(revision);
          if (project !== this.project) continue;
          const generation = this.generation;
          await Promise.all([
            work.status ? invoke<StatusSummary>('repository_status', { generation }).then(status => { if (this.current(project, generation)) { this.status = status; this.statusGeneration = generation; this.statusRevision++; } }) : null,
            work.refs ? invoke<References>('repository_refs', { generation }).then(refs => { if (this.current(project, generation)) this.refs = refs; }) : null,
          ]);
        } catch (error) { if (project === this.project) this.report(error); }
      }
    } finally { this.draining = false; }
  }
  private current(project: Opened['repository'], generation: number) { return !this.disposed && this.project === project && this.generation === generation; }
  private async relayout(revision: number) {
    const project = this.project!;
    const oldState = { selected: this.selected, index: this.activeIndex, scrollTop: this.scrollTop };
    const anchorIndex = Math.floor(this.scrollTop / this.rowHeight);
    const anchor = this.rows.get(anchorIndex)?.commit.hash;
    this.loading = true; this.pages.invalidate(); this.selectionGate.invalidate();
    try {
      const result = await invoke<{ generation: number; total: number }>('history_layout', { generation: this.generation, filter: buildFilter(this.filter) });
      if (project !== this.project || this.disposed) return;
      this.generation = result.generation;
      // A history generation keeps the independent status snapshot in Rust.
      if (this.status) this.statusGeneration = result.generation;
      if (revision !== this.filterRevision) return;
      this.total = result.total; this.rows = new Map();
      const position = (hash: string | undefined) => hash ? invoke<number | null>('history_position', { generation: result.generation, hash }) : Promise.resolve(null);
      const [selectedIndex, anchorPosition] = await Promise.all([position(oldState.selected), position(anchor)]);
      if (!this.current(project, result.generation)) return;
      const restored = preserveViewport(oldState, selectedIndex, anchorIndex, anchorPosition, this.rowHeight);
      this.selected = restored.selected; this.activeIndex = restored.index; this.scrollTop = restored.scrollTop;
      this.details = null;
      await tick();
      await this.loadRange(this.range, true);
      if (!this.current(project, result.generation)) return;
      // Refresh keeps the multi-selection; IDs no longer shown are dropped when used.
      if (this.selected) await this.selectHash(this.selected, true);
      else if (this.total) await this.selectIndex(Math.min(this.total - 1, Math.floor(this.scrollTop / this.rowHeight)));
    } catch (error) {
      const value = error as RepositoryError;
      if (project === this.project && value.generation) this.generation = value.generation;
      throw error;
    } finally { if (project === this.project) this.loading = false; }
  }
  async viewport(range: { start: number; end: number; rowHeight: number }) {
    this.range = { start: range.start, end: range.end };
    if (range.rowHeight > 0) this.rowHeight = range.rowHeight;
    if (!this.loading) await this.loadRange(this.range);
  }
  private async loadRange(range: { start: number; end: number }, force = false) {
    if (!this.project || !this.total || (!force && this.loading) || range.end <= range.start) return;
    const generation = this.generation, project = this.project;
    const ticket = this.pages.next();
    const start = Math.min(this.total, range.start), end = Math.min(this.total, range.end);
    try {
      for (let offset = start; offset < end; offset += 200) {
        const limit = Math.min(end, offset + 200);
        if (Array.from({ length: limit - offset }, (_, i) => offset + i).every(i => this.rows.has(i))) continue;
        const result = await invoke<Rows>('history_rows', { generation, start: offset, end: limit });
        if (!this.current(project, generation) || !this.pages.accepts(ticket) || result.generation !== generation) return;
        const rows = new Map(this.rows);
        result.nodes.forEach(([index, column, color, from, to], i) => rows.set(index, { index, column, color, segments: result.segments.slice(from, to), commit: result.commits[i], current: result.current[i] ?? false }));
        // Bound retained metadata; all permanent graph information remains in Rust.
        for (const index of rows.keys()) if (index < start - 200 || index > end + 200) rows.delete(index);
        this.rows = rows;
      }
    } catch (error) { if (this.current(project, generation) && this.pages.accepts(ticket)) this.report(error); }
  }
  async historyPosition(hash: string): Promise<number | null> {
    if (!this.project) return null;
    const project = this.project, generation = this.generation;
    const position = await invoke<number | null>('history_position', { generation, hash });
    return this.current(project, generation) ? position : null;
  }
  async goToCommit(hash: string) {
    const project = this.project, generation = this.generation;
    if (!project) return;
    const ticket = this.selectionGate.next();
    try {
      const position = await this.historyPosition(hash);
      if (position === null || !this.selectionGate.accepts(ticket)) return;
      this.scrollTop = position * this.rowHeight;
      await this.selectIndex(position);
    } catch (error) { if (this.current(project, generation) && this.selectionGate.accepts(ticket)) this.report(error); }
  }
  async selectIndex(index: number, keepSelection = false) {
    if (index < 0 || index >= this.total) return;
    this.activeIndex = index;
    const ticket = this.selectionGate.next();
    const generation = this.generation, project = this.project!;
    try {
      let row = this.rows.get(index);
      if (!row) {
        // Navigation may target an uncached row (End/PageDown). Its response is
        // guarded by the selection ticket, independent of scrolling page reads.
        const result = await invoke<Rows>('history_rows', { generation, start: index, end: index + 1 });
        if (!this.selectionGate.accepts(ticket) || !this.current(project, generation)) return;
        const node = result.nodes[0], commit = result.commits[0];
        if (node && commit) {
          row = { index: node[0], column: node[1], color: node[2], segments: result.segments.slice(node[3], node[4]), commit, current: result.current[0] ?? false };
          this.rows = new Map([...this.rows, [index, row]]);
        }
      }
      if (!this.selectionGate.accepts(ticket) || !this.current(project, generation)) return;
      if (row) await this.selectHash(row.commit.hash, keepSelection);
    } catch (error) { if (this.selectionGate.accepts(ticket) && this.current(project, generation)) this.report(error); }
  }
  /**
   * List selection change. IDs of rows not loaded yet (`row:N`, from Shift ranges) are
   * resolved in Rust from the current layout; only commit IDs cross IPC.
   */
  async setSelection(ids: ReadonlySet<string>, index: number) {
    const generation = this.generation, project = this.project;
    if (!project) return;
    const pending = [...ids].filter(id => id.startsWith(placeholder)).map(id => Number(id.slice(placeholder.length))).sort((a, b) => a - b);
    let hashes = [...ids].filter(id => !id.startsWith(placeholder));
    if (pending.length) {
      try {
        const resolved = await invoke<string[]>('history_hashes', { generation, ranges: rowRanges(pending) });
        if (!this.current(project, generation)) return;
        hashes = [...hashes, ...resolved];
      } catch (error) { if (this.current(project, generation)) this.report(error); return; }
    }
    this.selection = new Set(hashes);
    await this.selectIndex(index, true);
  }
  private async selectHash(hash: string, keepSelection = false) {
    const ticket = this.selectionGate.next(), generation = this.generation, project = this.project!;
    this.selected = hash; this.details = null;
    if (!keepSelection || !this.selection.has(hash)) this.selection = new Set([hash]);
    try {
      const details = await invoke<Commit>('commit_details', { generation, hash });
      if (!this.current(project, generation) || !this.selectionGate.accepts(ticket)) return;
      this.details = details;
    } catch (error) { if (this.current(project, generation) && this.selectionGate.accepts(ticket)) this.report(error); }
  }
}
