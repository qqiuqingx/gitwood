<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Compartment, EditorState, type Extension } from '@codemirror/state';
  import { EditorView, gutter, GutterMarker, lineNumbers } from '@codemirror/view';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import type { RepoPath } from '../browse/types';

  interface Line { line: number; hash: string; shortHash: string; author: string; authorTime: number; text: string; previous: string | null; previousPath: RepoPath | null }
  class AnnotateMarker extends GutterMarker {
    readonly line: Line; readonly ageRatio: number; readonly oncommit: (hash: string) => void; readonly onprevious: (line: Line) => void;
    constructor(line: Line, ageRatio: number, oncommit: (hash: string) => void, onprevious: (line: Line) => void) { super(); this.line = line; this.ageRatio = ageRatio; this.oncommit = oncommit; this.onprevious = onprevious; }
    eq(other: AnnotateMarker) { return this.line.hash === other.line.hash && this.line.author === other.line.author && this.ageRatio === other.ageRatio; }
    toDOM() {
      const button = document.createElement('button'); button.type = 'button'; button.className = 'annotate-marker';
      const date = new Date(this.line.authorTime * 1000);
      button.textContent = `${this.line.shortHash}  ${this.line.author} · ${date.toLocaleDateString()}`; button.title = `${this.line.shortHash} ${this.line.author} · ${date.toLocaleString()}`;
      button.style.setProperty('--annotate-age', String(this.ageRatio));
      button.onclick = event => { event.stopPropagation(); this.oncommit(this.line.hash); };
      button.oncontextmenu = event => { event.preventDefault(); event.stopPropagation(); this.onprevious(this.line); };
      return button;
    }
  }
  let { browser, id, total, oldest, newest, oncommit, onprevious, onerror }: { browser: RepositoryBrowser; id: string; total: number; oldest: number; newest: number; oncommit: (hash: string) => void; onprevious: (line: Line) => void; onerror: (error: unknown) => void } = $props();
  let host: HTMLDivElement;
  let view: EditorView | undefined;
  const blameGutterSlot = new Compartment();
  let lines = new Map<number, Line>();
  let pages = new Set<number>();
  let pending = new Set<number>();
  let alive = true;
  let current = { start: -1, end: -1 };
  function age(time: number) { return newest <= oldest ? 0 : Math.max(0, Math.min(1, (time - oldest) / (newest - oldest))); }
  function blameGutter(): Extension {
    return gutter({ class: 'annotate-info', side: 'before', lineMarker: (editor, block) => {
      const number = editor.state.doc.lineAt(block.from).number, line = lines.get(number - 1);
      if (!line) return null;
      return new AnnotateMarker(line, age(line.authorTime), oncommit, onprevious);
    } });
  }
  async function loadVisible() {
    if (!view || !total) return;
    const viewport = view.viewport;
    const start = Math.max(0, view.state.doc.lineAt(viewport.from).number - 1);
    const end = Math.min(total, view.state.doc.lineAt(viewport.to).number + 1);
    current = { start, end };
    const firstPage = Math.floor(start / 200), lastPage = Math.floor(Math.max(start, end - 1) / 200);
    for (let page = firstPage; page <= lastPage; page++) {
      if (pages.has(page) || pending.has(page)) continue;
      pending.add(page);
      const pageStart = page * 200, pageEnd = Math.min(total, pageStart + 200);
      void invoke<{ lines: Line[] }>('annotate_page', { generation: browser.generation, id, start: pageStart, end: pageEnd }).then(result => {
        if (!view) return;
        for (const line of result.lines) lines.set(line.line, line);
        const changes = result.lines.map(line => {
          const at = view!.state.doc.line(line.line + 1);
          return { from: at.from, to: at.to, insert: line.text };
        });
        view.dispatch({ changes, effects: blameGutterSlot.reconfigure(blameGutter()) });
        pages.add(page); pending.delete(page);
        prune();
      }).catch(cause => { pending.delete(page); if (alive) onerror(cause); });
    }
  }
  function prune() {
    const min = Math.max(0, Math.floor(current.start / 200) - 1), max = Math.floor(Math.max(0, current.end - 1) / 200) + 1;
    for (const page of [...pages]) {
      if (page >= min && page <= max) continue;
      const from = page * 200, to = Math.min(total, from + 200);
      const changes = [];
      for (let line = from; line < to; line++) {
        const value = view!.state.doc.line(line + 1);
        if (value.length) changes.push({ from: value.from, to: value.to, insert: '' });
        lines.delete(line);
      }
      if (changes.length) view!.dispatch({ changes });
      pages.delete(page);
    }
    view?.dispatch({ effects: blameGutterSlot.reconfigure(blameGutter()) });
  }
  onMount(() => {
    const doc = '\n'.repeat(Math.max(0, total - 1));
    view = new EditorView({ state: EditorState.create({ doc, extensions: [EditorState.readOnly.of(true), EditorView.editable.of(false), lineNumbers(), blameGutterSlot.of(blameGutter()), EditorView.lineWrapping, EditorView.updateListener.of(update => { if (update.viewportChanged || update.geometryChanged) void loadVisible(); }), EditorView.theme({ '&': { height: '100%', fontFamily: 'var(--font-editor-family)', fontSize: 'var(--font-editor-size)' }, '.cm-scroller': { overflow: 'auto' }, '.annotate-info': { width: '280px', overflow: 'hidden' }, '.annotate-marker': { width: '100%', overflow: 'hidden', whiteSpace: 'nowrap', textOverflow: 'ellipsis', textAlign: 'start', border: 0, color: 'var(--text-secondary)', background: 'color-mix(in srgb, var(--accent) calc(var(--annotate-age) * 38%), transparent)', font: 'inherit', cursor: 'pointer' } })] }), parent: host });
    void loadVisible();
  });
  onDestroy(() => { alive = false; view?.destroy(); view = undefined; });
</script>
<div class="editor" bind:this={host}></div>
<style>.editor { height: 100%; min-height: 0; overflow: hidden; border: var(--border-width) solid var(--separator); }</style>
