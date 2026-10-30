import { matchSpeedSearch } from '../../components/speed-search';
import type { TreeNode, TreeSource } from '../../components/tree';
import type { Filter, FilterInput, RepoPath, References, Refresh } from './types';

export const emptyFilter = (): FilterInput => ({ text: '', branch: '', author: '', since: '', until: '', path: '', regex: false, matchCase: false });
export function buildFilter(input: FilterInput): Filter {
  const optional = (value: string) => value.trim() || null;
  const path = input.path; // Paths are literal: leading/trailing whitespace is meaningful.
  return { text: optional(input.text), branch: optional(input.branch), author: optional(input.author),
    since: optional(input.since), until: optional(input.until),
    path: path ? { display: path, bytes: Array.from(new TextEncoder().encode(path)) } : null,
    regex: input.regex, match_case: input.matchCase };
}
export interface PrettyDateWords { today: string; yesterday: string; moments: string; minute: string; minutes: string; hour: string }
/** IDEA DateFormatUtil.doFormatPretty: <=61 minutes (Math.rint), then local
 * today/yesterday, otherwise the system short date and time. No idle timer. */
export function formatDate(seconds: number, locale: string, now = new Date(), words: Partial<PrettyDateWords> = {}): string {
  if (seconds < 0) return '';
  const labels = { today: 'Today', yesterday: 'Yesterday', moments: 'Moments ago', minute: 'A minute ago', minutes: '{count} minutes ago', hour: '1 hour ago', ...words };
  const date = new Date(seconds * 1000);
  const elapsed = now.getTime() - date.getTime();
  if (elapsed >= 0 && elapsed <= 61 * 60000) {
    const value = elapsed / 60000, floor = Math.floor(value);
    const minutes = value - floor === 0.5 ? floor + floor % 2 : Math.round(value);
    return minutes === 0 ? labels.moments : minutes === 1 ? labels.minute : minutes >= 60 ? labels.hour
      : labels.minutes.replace('{count}', new Intl.NumberFormat(locale).format(minutes));
  }
  const day = (value: Date) => new Date(value.getFullYear(), value.getMonth(), value.getDate()).getTime();
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  const relative = day(date) === day(now) ? labels.today : day(date) === day(yesterday) ? labels.yesterday : '';
  const time = new Intl.DateTimeFormat(locale, { hour: '2-digit', minute: '2-digit' }).format(date);
  return relative ? `${relative} ${time}` : new Intl.DateTimeFormat(locale, { dateStyle: 'short', timeStyle: 'short' }).format(date);
}
/** Separate local request tickets from backend generations (selection changes do not relayout). */
export class RequestGate {
  private ticket = 0;
  next() { return ++this.ticket; }
  accepts(ticket: number) { return this.ticket === ticket; }
  invalidate() { this.ticket++; }
}
export function mergeRefresh(a: Refresh, b: Refresh): Refresh {
  return { status: a.status || b.status, refs: a.refs || b.refs, history: a.history || b.history };
}
export function preserveViewport(state: { selected: string; index: number; scrollTop: number },
  selectedIndex: number | null, oldAnchor: number, newAnchor: number | null, rowHeight: number) {
  return { selected: selectedIndex === null ? '' : state.selected, index: selectedIndex ?? -1,
    scrollTop: Math.max(0, state.scrollTop + (newAnchor === null ? 0 : newAnchor - oldAnchor) * rowHeight) };
}
export const pathId = (path: RepoPath) => path.bytes.map(byte => byte.toString(16).padStart(2, '0')).join('');
export interface BrowseNode extends TreeNode { status?: string; note?: string; path?: RepoPath; ref?: string }
export interface BrowseTree { source: TreeSource; nodes: Map<string, BrowseNode>; directories: Set<string> }
export interface TreeItem { id: string; path: RepoPath; status?: string; note?: string; ref?: string }
export function directoryTree(groups: { id: string; label: string; items: TreeItem[] }[], flat = false): BrowseTree {
  const nodes = new Map<string, BrowseNode>();
  const children = new Map<string, BrowseNode[]>();
  const roots: BrowseNode[] = [];
  const directories = new Set<string>();
  function add(parent: string, node: BrowseNode) {
    nodes.set(node.id, node);
    const entries = children.get(parent) ?? []; entries.push(node); children.set(parent, entries);
  }
  for (const group of groups) {
    const root = { id: group.id, label: `${group.label} (${group.items.length})`, hasChildren: !!group.items.length };
    roots.push(root); nodes.set(root.id, root); directories.add(root.id);
    for (const item of group.items) {
      let parent = group.id;
      // Split bytes as well as display text so undecodable names never alias each other.
      const parts: number[][] = [[]];
      for (const byte of item.path.bytes) { if (byte === 47) parts.push([]); else parts.at(-1)!.push(byte); }
      if (!flat) {
        let prefix: number[] = [];
        for (const part of parts.slice(0, -1)) {
          prefix = [...prefix, ...part, 47];
          const id = `${group.id}/dir/${pathId({ bytes: prefix, display: '' })}`;
          if (!nodes.has(id)) { add(parent, { id, label: new TextDecoder().decode(new Uint8Array(part)), hasChildren: true }); directories.add(id); }
          parent = id;
        }
      }
      add(parent, { ...item, label: flat ? item.path.display : new TextDecoder().decode(new Uint8Array(parts.at(-1)!)), hasChildren: false });
    }
  }
  return { nodes, directories, source: { rootCount: roots.length, getRoot: index => roots[index],
    childCount: id => children.get(id)?.length ?? 0, getChild: (id, index) => children.get(id)![index] } };
}
export function statusColor(code: string): string {
  return ({ A: 'added', M: 'modified', D: 'deleted', R: 'renamed', C: 'added', U: 'conflicted', '?': 'untracked' } as Record<string, string>)[code] ?? 'modified';
}
export function branchTree(refs: References, labels: { local: string; remote: string; tags: string; head: string; gone: string }) {
  const item = (ref: References['local'][number], prefix: string): TreeItem => {
    const name = ref.name.slice(prefix.length);
    const current = ref.name === `refs/heads/${refs.head.head}`;
    const track = [ref.track.match(/ahead (\d+)/)?.[1] ? `↑${ref.track.match(/ahead (\d+)/)![1]}` : '', ref.track.match(/behind (\d+)/)?.[1] ? `↓${ref.track.match(/behind (\d+)/)![1]}` : '', ref.track.includes('gone') ? labels.gone : ''].filter(Boolean).join(' ');
    const note = [current ? 'HEAD' : '', track, ref.upstream ?? ''].filter(Boolean).join(' · ');
    return { id: ref.name, ref: ref.name, path: { display: name, bytes: [...new TextEncoder().encode(name)] }, note };
  };
  const tree = directoryTree([
    { id: 'local', label: labels.local, items: refs.local.map(ref => item(ref, 'refs/heads/')) },
    { id: 'remote', label: labels.remote, items: Object.values(refs.remotes).flat().map(ref => item(ref, 'refs/remotes/')) },
    { id: 'tags', label: labels.tags, items: refs.tags.map(ref => item(ref, 'refs/tags/')) },
  ]);
  const root = tree.source;
  const head = { id: 'HEAD', label: labels.head, hasChildren: false, ref: !refs.head.oid || refs.head.oid === '(initial)' ? '' : refs.head.head && refs.head.head !== '(detached)' ? `refs/heads/${refs.head.head}` : 'HEAD' };
  tree.nodes.set('HEAD', head);
  tree.source = { ...root, rootCount: root.rootCount + 1, getRoot: index => index === 0 ? head : root.getRoot(index - 1) };
  return tree;
}

/** Search only expanded rows. Keep at most one page, using the same IDEA matcher
 * as local List/Tree. A null page cancels an obsolete search. */
export async function findPagedMatch(count: number, read: (start: number, end: number) => Promise<string[] | null>, query: string, active: number, direction: 1 | -1, includeCurrent: boolean): Promise<number> {
  if (!count) return -1;
  const origin = active < 0 ? (direction === 1 ? -1 : 0) : active;
  let pageStart = -1, page: string[] = [];
  for (let offset = includeCurrent && active >= 0 ? 0 : 1; offset <= count; offset++) {
    const index = ((origin + offset * direction) % count + count) % count;
    const start = Math.floor(index / 200) * 200;
    if (pageStart !== start) {
      const incoming = await read(start, Math.min(count, start + 200));
      if (incoming === null) return -1;
      page = incoming; pageStart = start;
    }
    if (page[index - start] !== undefined && matchSpeedSearch(query, page[index - start]) !== null) return index;
  }
  return -1;
}
export function fileStatus(node: { status: string | null; staged: boolean; unstaged: boolean; conflicted: boolean }) {
  if (node.conflicted) return 'conflicted';
  const code = node.status ?? '';
  return statusColor(node.unstaged && code.length === 2 ? code[1] : code[0]);
}
export interface RefLabel { name: string; kind: 'head' | 'branch-local' | 'branch-remote' | 'tag' }
/** Labels per commit hash: HEAD first, then local branches, remote branches and tags (vcs-log order). */
export function refLabelsByHash(refs: References | null): Map<string, RefLabel[]> {
  const map = new Map<string, RefLabel[]>();
  if (!refs) return map;
  const groups = [[refs.local, 'branch-local'], [Object.values(refs.remotes).flat(), 'branch-remote'], [refs.tags, 'tag']] as const;
  for (const [list, kind] of groups) {
    for (const ref of list) {
      const hash = ref.peeled ?? ref.hash;
      map.set(hash, [...(map.get(hash) ?? []), { name: ref.name.replace(/^refs\/(heads|remotes|tags)\//, ''), kind }]);
    }
  }
  if (refs.head.oid) map.set(refs.head.oid, [{ name: 'HEAD', kind: 'head' }, ...(map.get(refs.head.oid) ?? [])]);
  return map;
}
/** Absolute date and time used by the commit details header (IDEA: `2026/10/30` and `15:36`). */
export function detailsDateTime(seconds: number): { date: string; time: string } {
  const value = new Date(seconds * 1000), pad = (n: number) => String(n).padStart(2, '0');
  return { date: `${value.getFullYear()}/${pad(value.getMonth() + 1)}/${pad(value.getDate())}`, time: `${pad(value.getHours())}:${pad(value.getMinutes())}` };
}
/** Number of leading filter buttons that fit; the rest move behind IDEA's `>` overflow button. */
export function fittingFilters(available: number, widths: readonly number[], overflowWidth: number, gap: number): number {
  const total = (count: number) => widths.slice(0, count).reduce((sum, width) => sum + width + gap, 0);
  if (total(widths.length) <= available) return widths.length;
  let count = widths.length - 1;
  while (count > 0 && total(count) + overflowWidth > available) count--;
  return count;
}
