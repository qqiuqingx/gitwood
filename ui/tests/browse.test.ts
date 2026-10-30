import { describe, expect, it } from 'vitest';
import { branchTree, buildFilter, fittingFilters, directoryTree, emptyFilter, formatDate, mergeRefresh, pathId, preserveViewport, RequestGate, fileStatus, findPagedMatch } from '../src/features/browse/model';
import { graphCommands } from '../src/features/browse/graph';
import { flattenTree } from '../src/components/tree';
import type { HistoryRow, References } from '../src/features/browse/types';
const path = (display: string) => ({ display, bytes: [...new TextEncoder().encode(display)] });

describe('browse filters and regional dates', () => {
  it('moves trailing filter buttons behind the overflow button only when they do not fit', () => {
    expect(fittingFilters(400, [60, 50, 50, 50], 20, 4)).toBe(4);
    expect(fittingFilters(150, [60, 50, 50, 50], 20, 4)).toBe(2);
    expect(fittingFilters(70, [60, 50, 50, 50], 20, 4)).toBe(0);
  });
  it('builds the real typed IPC filter with literal paths and intersects optional fields', () => {
    expect(buildFilter({ ...emptyFilter(), text: ' hash ', branch: 'refs/heads/main', author: 'Alice', since: '2026-01-01', path: ' -name \n' })).toEqual({
      text: 'hash', branch: 'refs/heads/main', author: 'Alice', since: '2026-01-01', until: null, path: path(' -name \n'), regex: false, match_case: false,
    });
    expect(buildFilter(emptyFilter()).path).toBeNull();
    expect(buildFilter({ ...emptyFilter(), text: '^fix', regex: true, matchCase: true })).toMatchObject({ text: '^fix', regex: true, match_case: true });
  });
  it('uses local calendar days, system time formatting, and includes year for older dates', () => {
    const now = new Date(2026, 9, 30, 12), today = new Date(2026, 9, 30, 9, 5), yesterday = new Date(2026, 9, 29, 23, 59);
    const clock = new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit' });
    expect(formatDate(today.getTime() / 1000, 'en-GB', now)).toBe(`Today ${clock.format(today)}`);
    expect(formatDate(yesterday.getTime() / 1000, 'en-GB', now)).toBe(`Yesterday ${clock.format(yesterday)}`);
    expect(formatDate(today.getTime() / 1000, 'zh-CN', now, { today: '今天', yesterday: '昨天' })).toContain('今天');
    expect(formatDate((now.getTime() - 30000) / 1000, 'en-GB', now)).toBe('Moments ago');
    expect(formatDate((now.getTime() - 90000) / 1000, 'en-GB', now)).toBe('2 minutes ago');
    expect(formatDate((now.getTime() - 61 * 60000) / 1000, 'en-GB', now)).toBe('1 hour ago');
    expect(formatDate(new Date(2025, 0, 1).getTime() / 1000, 'en-GB', now)).toContain('2025');
  });
});
describe('graph drawing instructions', () => {
  const row = (index: number): HistoryRow => ({ index, column: 0, color: -4,
    segments: [[0, 0, 1, -4, index === 0 ? 1 : -1, true]],
    commit: { hash: `hash${index}`, parents: [], author: '', email: '', author_time: 0, commit_time: 0, subject: '', message: '', refs: '' } });
  it('keeps half-row endpoints identical across viewports and draws nodes after edges', () => {
    const a = row(0), b = row(1); b.segments[0][1] = 1; b.segments[0][2] = 0;
    const commands = graphCommands([a, b], 7, 24, 60, 'hash0');
    expect(commands[0].kind).toBe('edge'); expect(commands[1].kind).toBe('edge');
    if (commands[0].kind === 'edge' && commands[1].kind === 'edge') {
      expect(commands[0].to).toEqual(commands[1].to); expect(commands[0].dotted).toBe(true);
    }
    expect(commands[2]).toMatchObject({ kind: 'node', head: true, color: -4 });
    const first = graphCommands([a], 7, 24, 60, null)[0];
    expect(first).toEqual(commands[0]);
    expect(graphCommands([row(10000)], 0, 24, 60, null)).toEqual([]);
  });
});
describe('refresh and stale responses', () => {
  it('rejects earlier requests including errors after selection, relayout, or disposal', async () => {
    const gate = new RequestGate();
    let resolve!: (value: string) => void;
    const old = gate.next(); const pending = new Promise<string>(r => { resolve = r; });
    const next = gate.next(); let displayed = 'new';
    const completion = pending.then(value => { if (gate.accepts(old)) displayed = value; });
    resolve('old'); await completion;
    expect(displayed).toBe('new'); expect(gate.accepts(next)).toBe(true);
    gate.invalidate(); expect(gate.accepts(next)).toBe(false);
  });
  it('keeps selection, scroll offset and independent refresh regions', () => {
    expect(preserveViewport({ selected: 'abc', index: 100, scrollTop: 2407 }, 103, 100, 103, 24))
      .toEqual({ selected: 'abc', index: 103, scrollTop: 2479 });
    expect(preserveViewport({ selected: 'abc', index: 100, scrollTop: 2407 }, null, 100, null, 24))
      .toEqual({ selected: '', index: -1, scrollTop: 2407 });
    expect(mergeRefresh({ status: true, refs: false, history: false }, { status: false, refs: true, history: false }))
      .toEqual({ status: true, refs: true, history: false });
  });
});
describe('byte-safe grouped trees', () => {
  it('groups directories, keeps paths as text, has stable IDs, and supports flat view', () => {
    const groups = [{ id: 'changes', label: 'Changes', items: ['src/a.ts', 'src/sub/b.ts', '<script>\n.txt'].map(name => ({ id: name, path: path(name) })) }];
    const tree = directoryTree(groups);
    const rows = flattenTree(tree.source, tree.directories);
    expect(rows.filter(row => !row.hasChildren).map(row => row.label)).toEqual(['a.ts', 'b.ts', '<script>\n.txt']);
    expect(flattenTree(directoryTree(groups, true).source, new Set(['changes']))).toHaveLength(4);
    const invalid = { bytes: [255], display: '�' }, other = { bytes: [254], display: '�' };
    expect(pathId(invalid)).not.toBe(pathId(other));
    const refresh = directoryTree(groups);
    expect([...tree.directories]).toEqual([...refresh.directories]);
  });
  it('colors combined status from the actual displayed worktree state', () => {
    expect(fileStatus({ status: 'MD', staged: true, unstaged: true, conflicted: false })).toBe('deleted');
    expect(fileStatus({ status: 'UU', staged: false, unstaged: false, conflicted: true })).toBe('conflicted');
  });
  it('searches uncached pages with the existing matcher and cancels obsolete scans', async () => {
    const read = async (start: number, end: number) => Array.from({ length: end - start }, (_, offset) => start + offset === 420 ? 'GitBranchPanel' : 'other');
    expect(await findPagedMatch(1000, read, 'gbp', 0, 1, false)).toBe(420);
    expect(await findPagedMatch(1000, async () => null, 'gbp', 0, 1, false)).toBe(-1);
  });
  it('groups remote branches by remote, marks HEAD and tracking, and retains tag ref IDs', () => {
    const ref = (name: string) => ({ name, hash: 'abc', peeled: null, upstream: null, track: 'ahead 2' });
    const refs: References = { local: [ref('refs/heads/main')], remotes: { origin: [ref('refs/remotes/origin/main')] }, tags: [ref('refs/tags/v1')], remote_heads: {},
      head: { oid: 'abc', head: 'main', upstream: null, ahead: 2, behind: 0 } };
    const tree = branchTree(refs, { local: 'Local', remote: 'Remote', tags: 'Tags', head: 'HEAD', gone: 'Unavailable' });
    expect(tree.nodes.get('refs/heads/main')?.note).toContain('HEAD');
    expect(tree.nodes.get('refs/tags/v1')?.ref).toBe('refs/tags/v1');
    expect([...tree.nodes.values()].some(node => node.label === 'origin' && node.hasChildren)).toBe(true);
  });
});
