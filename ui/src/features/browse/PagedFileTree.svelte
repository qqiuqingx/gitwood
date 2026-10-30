<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Tree from '../../components/Tree.svelte';
  import CheckBox from '../../components/CheckBox.svelte';
  import type { Selection, Inclusion } from '../commit/model';
  import Highlight from '../../components/Highlight.svelte';
  import type { TreeRow } from '../../components/tree';
  import { RequestGate, fileStatus, findPagedMatch } from './model';
  import { browseText as t } from '../../i18n/browse';
  import type { RepoPath } from './types';
  export interface FileRow extends TreeRow { path: RepoPath | null; status: string | null; note: string | null; staged: boolean; unstaged: boolean; conflicted: boolean; count: number }
  interface Page { total: number; files: number; start: number; rows: FileRow[]; positions: (number | null)[]; inclusion?: { states: Inclusion[]; count: number } }
  let { hash = '', generation, onerror, changes = false, revision = 0, flat = false,
    onselect, selection, ontoggle, oncount, onselectedids, onopen,
    expanded = $bindable(new Set(['files', 'changes', 'unversioned']) as ReadonlySet<string>),
  }: { selection?: Selection; ontoggle?: (id: string, state: Inclusion) => void; oncount?: (count: number) => void; onselectedids?: (ids: string[]) => void; onopen?: () => void; onselect?: (row: FileRow) => void; hash?: string; generation: number; onerror: (error: unknown) => void; changes?: boolean; revision?: number; flat?: boolean; expanded?: ReadonlySet<string> } = $props();
  const target = $derived(changes ? (revision ? 'changes' : '') : hash);
  let selected = $state<ReadonlySet<string>>(new Set());
  let rows = $state.raw(new Map<number, FileRow>());
  let states = $state.raw(new Map<string, Inclusion>());
  let total = $state(0), files = $state(0);
  let scrollTop = $state(0), activeIndex = $state(-1);
  let rowHeight = 24;
  let range = { start: 0, end: 40 };
  const requests = new RequestGate();
  const searches = new RequestGate();
  const rowAt = (index: number): FileRow => rows.get(index) ?? { id: `loading:${index}`, label: t('loading'), hasChildren: false, parentIndex: -1, level: 1, path: null, status: null, note: null, staged: false, unstaged: false, conflicted: false, count: 0 };
  async function load(next = range, reset = false) {
    range = next;
    const ticket = requests.next(), selectedHash = target, epoch = generation;
    if (!selectedHash) return;
    const ids = [...expanded];
    const oldAnchor = Math.floor(scrollTop / rowHeight), oldTop = scrollTop;
    const positionIds = [[...selected][0] ?? '', rows.get(oldAnchor)?.id ?? ''];
    try {
      const incoming = new Map(reset ? [] : rows);
      for (let start = next.start; start < next.end; start += 200) {
        const end = Math.min(next.end, start + 200);
        const page = await invoke<Page>(changes ? 'repository_changes' : 'commit_file_tree', { generation: epoch, hash, viewport: { expanded: ids, flat, start, end, positionIds, selection } });
        if (!requests.accepts(ticket) || selectedHash !== target || epoch !== generation) return;
        total = page.total; files = page.files;
        if (page.inclusion) { const next = new Map(states); page.rows.forEach((r,i) => next.set(r.id, page.inclusion!.states[i])); states=next; oncount?.(page.inclusion.count); }
        if (reset) {
          activeIndex = page.positions[0] ?? -1;
          if (positionIds[0] && page.positions[0] === null) selected = new Set();
          if (page.positions[1] !== null) scrollTop = Math.max(0, oldTop + (page.positions[1] - oldAnchor) * rowHeight);
        }
        page.rows.forEach((row, offset) => incoming.set(page.start + offset, { ...row, parentIndex: row.parentIndex ?? -1, label: ['files', 'changes', 'unversioned'].includes(row.id) ? `${t(row.id as 'files' | 'changes' | 'unversioned')} (${row.count})` : row.label }));
      }
      for (const index of incoming.keys()) if (index < next.start - 200 || index > next.end + 200) incoming.delete(index);
      rows = incoming;
      const visibleIds=new Set([...incoming.values()].map(row=>row.id));states=new Map([...states].filter(([id])=>visibleIds.has(id)));
    } catch (error) { if (requests.accepts(ticket) && epoch === generation && selectedHash === target) onerror(error); }
  }
  let previousHash = '';
  $effect(() => {
    target; generation; expanded; flat; revision; selection;
    untrack(() => {
      if (target !== previousHash) { previousHash = target; rows = new Map(); total = 0; files = 0; selected = new Set(); scrollTop = 0; activeIndex = -1; range = { start: 0, end: 40 }; }
      if (target) void load(range, true);
    });
    return () => { requests.invalidate(); searches.invalidate(); };
  });
  async function toggleSelected(ids: readonly string[]) {
    const chosenSelection=selection, epoch=generation;
    try {
      const values: Inclusion[]=[];
      for(let start=0;start<ids.length;start+=200)values.push(...await invoke<Inclusion[]>('inclusion_states',{generation:epoch,selection:chosenSelection,ids:ids.slice(start,start+200)}));
      if(chosenSelection!==selection || epoch!==generation)return;
      const target=values.every(value=>value==='all') ? 'all' : 'none';
      for(const id of ids)ontoggle?.(id,target);
    }catch(error){onerror(error);}
  }
  async function search(query: string, active: number, direction: 1 | -1, includeCurrent: boolean) {
    const ticket = searches.next(), epoch = generation, selectedTarget = target;
    try {
      const index = await findPagedMatch(total, async (start, end) => {
        const page = await invoke<Page>(changes ? 'repository_changes' : 'commit_file_tree', { generation: epoch, hash, viewport: { expanded: [...expanded], flat, start, end, positionIds: [] } });
        if (!searches.accepts(ticket) || epoch !== generation || selectedTarget !== target) return null;
        return page.rows.map(row => row.label || t(row.id as 'files' | 'changes' | 'unversioned'));
      }, query, active, direction, includeCurrent);
      if (index >= 0 && searches.accepts(ticket)) { await load({ start: index, end: Math.min(total, index + 40) }); await tick(); }
      return searches.accepts(ticket) ? index : -1;
    } catch (error) { if (searches.accepts(ticket)) onerror(error); return -1; }
  }
</script>
<section>
  {#if !changes}<h3>{t('files')} <span>{files}</span></h3>{/if}
  <div class="tree"><Tree bind:scrollTop bind:activeIndex onviewportchange={viewport => { if (viewport.rowHeight > 0) rowHeight = viewport.rowHeight; }} searchRows={search} onquerychange={query => { if (!query) searches.invalidate(); }} label={changes ? t('changes') : t('files')} visibleRows={{ count: total, getRow: rowAt }} {expanded} onexpandedchange={ids => { expanded = ids; }}
    onactivate={node => { if((node as FileRow).path) { onselect?.(node as FileRow); onopen?.(); } }}
    onspace={ontoggle ? ids=>void toggleSelected(ids) : undefined}
    bind:selectedIds={selected} onselectionchange={(_, row) => {
      onselectedids?.([...selected]);
      if (row && (row as FileRow).path) onselect?.(row as FileRow);
      if (row?.id.startsWith('loading:')) {
        const index = Number(row.id.slice(8));
        void load({ start: index, end: index + 40 }).then(() => { const node = rows.get(index); if (node) { selected = new Set([node.id]); if (node.path) onselect?.(node); } });
      }
    }} onrangechange={next => { if (next.end > next.start && (next.start !== range.start || next.end !== range.end)) void load(next); }}>
    {#snippet content(node, query)}
      {@const file = node as FileRow}
      {#if changes && ontoggle}<CheckBox state={states.get(node.id) ?? 'none'} label={node.label} disabled={file.conflicted || node.id.startsWith('loading:')} onchange={() => ontoggle?.(node.id,states.get(node.id) ?? 'none')} />{/if}
      <span style:color={file.status ? `var(--status-${fileStatus(file)})` : undefined} title={file.path?.display ?? file.label}><Highlight text={node.label} {query} /></span>
      {#if file.note}<span class="note">{file.note} →</span>{/if}
      {#if file.staged || file.unstaged || file.conflicted}<span class="note">{file.conflicted ? t('conflicted') : [file.staged ? t('staged') : '', file.unstaged ? t('unstaged') : ''].filter(Boolean).join(' + ')}</span>{/if}
    {/snippet}
  </Tree></div>
</section>
<style>
  section { height: 100%; display: flex; flex-direction: column; min-height: 0; }
  h3 { font: inherit; font-weight: var(--font-weight-heading); margin: 0; padding: var(--space-panel-y) var(--space-inline); border-bottom: var(--border-width) solid var(--separator); }
  h3 span { color: var(--text-secondary); font-weight: normal; }
  .note { font-size: var(--font-small-size); color: var(--text-secondary); }
  .tree { flex: 1; min-height: 0; }
</style>
