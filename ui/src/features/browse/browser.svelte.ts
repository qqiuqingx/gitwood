import { CommitSession } from '../commit/session.svelte';
import { tick } from 'svelte';
import { invoke } from '@tauri-apps/api/core';
import { buildFilter, emptyFilter, mergeRefresh, preserveViewport, RequestGate } from './model';
import type { Commit, FilterInput, HistoryRow, Opened, Recent, References, Refresh, RepositoryError, Rows, StatusSummary } from './types';

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
  private selection = new RequestGate();
  private filterRevision = 0;
  private disposed = false;
  private operations: Promise<unknown> = Promise.resolve();

  async recentProjects() { this.recent = await invoke<Recent[]>('recent_repositories'); }
  async open(path: string) {
    if (this.commit.busy) return;
    this.commit = new CommitSession();
    const ticket = this.session.next();
    this.pages.invalidate(); this.selection.invalidate();
    this.previewFile = null; this.project = null; this.generation = 0; this.opening = true; this.error = null;
    this.rows = new Map(); this.total = 0; this.status = null; this.statusRevision = 0; this.statusGeneration = 0; this.refs = null;
    this.selected = ''; this.activeIndex = -1; this.scrollTop = 0; this.details = null;
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
    this.session.invalidate(); this.pages.invalidate(); this.selection.invalidate();
    this.previewFile = null; this.project = null; this.generation = 0; this.rows = new Map(); this.status = null; this.statusRevision = 0; this.statusGeneration = 0; this.refs = null;
    this.details = null; this.pending = nothing(); this.opening = false;
    this.operations = this.operations.catch(() => {}).then(() => invoke('close_repository'));
    await this.operations;
  }
  dispose() { this.disposed = true; this.session.invalidate(); this.pages.invalidate(); this.selection.invalidate(); }
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
    this.loading = true; this.pages.invalidate(); this.selection.invalidate();
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
      if (this.selected) await this.selectHash(this.selected);
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
        result.nodes.forEach(([index, column, color, from, to], i) => rows.set(index, { index, column, color, segments: result.segments.slice(from, to), commit: result.commits[i] }));
        // Bound retained metadata; all permanent graph information remains in Rust.
        for (const index of rows.keys()) if (index < start - 200 || index > end + 200) rows.delete(index);
        this.rows = rows;
      }
    } catch (error) { if (this.current(project, generation) && this.pages.accepts(ticket)) this.report(error); }
  }
  async selectIndex(index: number) {
    if (index < 0 || index >= this.total) return;
    this.activeIndex = index;
    const ticket = this.selection.next();
    const generation = this.generation, project = this.project!;
    try {
      let row = this.rows.get(index);
      if (!row) {
        // Navigation may target an uncached row (End/PageDown). Its response is
        // guarded by the selection ticket, independent of scrolling page reads.
        const result = await invoke<Rows>('history_rows', { generation, start: index, end: index + 1 });
        if (!this.selection.accepts(ticket) || !this.current(project, generation)) return;
        const node = result.nodes[0], commit = result.commits[0];
        if (node && commit) {
          row = { index: node[0], column: node[1], color: node[2], segments: result.segments.slice(node[3], node[4]), commit };
          this.rows = new Map([...this.rows, [index, row]]);
        }
      }
      if (!this.selection.accepts(ticket) || !this.current(project, generation)) return;
      if (row) await this.selectHash(row.commit.hash);
    } catch (error) { if (this.selection.accepts(ticket) && this.current(project, generation)) this.report(error); }
  }
  private async selectHash(hash: string) {
    const ticket = this.selection.next(), generation = this.generation, project = this.project!;
    this.selected = hash; this.details = null;
    try {
      const details = await invoke<Commit>('commit_details', { generation, hash });
      if (!this.current(project, generation) || !this.selection.accepts(ticket)) return;
      this.details = details;
    } catch (error) { if (this.current(project, generation) && this.selection.accepts(ticket)) this.report(error); }
  }
}
