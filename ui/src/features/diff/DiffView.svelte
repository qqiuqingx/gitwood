<script lang="ts">
  import { preservedScroll } from './request';
  import { onMount } from 'svelte';
  import { Compartment, EditorState } from '@codemirror/state';
  import { EditorView } from '@codemirror/view';
  import { diffText as t } from '../../i18n/diff';
  import { createActionRegistry } from '../../keymap/registry';
  import { keymapFor } from '../../keymap/keymaps';
  import { detectPlatform } from '../../keymap/shortcuts';
  import { constrainSplit } from '../../components/splitter';
  import { connectorGeometry, expandFold, mapLine, nextDifference, visibleBlockRange, type Connector } from './model';
  import { diffDecorations, editorExtensions, linePosition, lineBoundary, inclusionGutter, rollbackGutter } from './editor';
  import type { Content, DiffResult, Side } from './types';

  let { left, right, result, fileKey = '', included = [], onblock, onrevert, disabled = false, ratio = $bindable(0.5) }: { included?: readonly number[]; onblock?: (index: number) => void; onrevert?: (index: number) => void; disabled?: boolean; fileKey?: string; left: string | Content; right: string | Content; result: DiffResult; ratio?: number } = $props();
  function content(value: string | Content): Content {
    return typeof value === 'string' ? { kind: 'text', text: value, byteLen: new TextEncoder().encode(value).length,
      empty: !value.length, missingFinalNewline: !!value.length && !value.endsWith('\n') } : value;
  }
  const leftContent = $derived(content(left));
  const rightContent = $derived(content(right));
  let mounted = $state(false);
  let leftHost: HTMLDivElement; let rightHost: HTMLDivElement; let middle: HTMLDivElement; let host: HTMLDivElement;
  let selected = $state(-1);
  let bands = $state<{ index: number; kind: string; geometry: Connector }[]>([]);
  let middleWidth = $state(1); let middleHeight = $state(1);
  let previousFileKey = '';
  let savedScroll = { left: 0, right: 0 };
  let updateMarkers = $state<((included: readonly number[], disabled: boolean, revert?: (index: number) => void, toggle?: (index: number) => void) => void) | undefined>();
  let navigate: ((direction: 1 | -1) => void) | undefined;
  let resizeMeasure: (() => void) | undefined;
  let drag: { id: number; start: number; width: number; first: number } | undefined;
  onMount(() => { mounted = true; });

  $effect(() => {
    if (!mounted) return;
    const data = result;
    const currentFileKey = fileKey;
    // Verified in tauri 2.12.5 manager/mod.rs replace_csp_nonce. Vite dev has
    // no asset nonce and already permits its HMR/runtime styles in devCsp.
    const nonce = document.querySelector<HTMLMetaElement>('meta[name="gitwood-style-nonce"]')?.content ?? '';
    const contents = { left: leftContent, right: rightContent };
    let alive = true; let expanded: ReadonlySet<number> = new Set(); let active: Side = contents.left.kind === 'text' ? 'left' : 'right';
    const views: Partial<Record<Side, EditorView>> = {};
    const markers = { left: new Compartment(), right: new Compartment() };
    const compartments = { left: new Compartment(), right: new Compartment() };
    const expected = new Map<Side, number>();
    let source: Side | undefined;
    selected = -1;
    bands = [];
    function expand(id: number) {
      expanded = expandFold(expanded, id);
      for (const side of ['left', 'right'] as const) {
        const view = views[side];
        if (view) view.dispatch({ effects: compartments[side].reconfigure(EditorView.decorations.of(diffDecorations(view.state, side, data, expanded, expand))) });
      }
      measure();
    }
    function measure() {
      const primary = views.left ?? views.right;
      if (!primary || !alive) return;
      primary.requestMeasure({
        key: host,
        read: () => {
          if (!alive) return null;
          const rect = middle.getBoundingClientRect();
          let target: { side: Side; top: number } | undefined;
          const from = source; source = undefined;
          if (from) {
            const origin = views[from]; const side = from === 'left' ? 'right' : 'left'; const destination = views[side];
            if (origin && destination) {
              const top = Math.max(0, origin.scrollDOM.scrollTop - origin.documentPadding.top);
              const block = origin.lineBlockAtHeight(top);
              const line = origin.state.doc.lineAt(block.from).number - 1;
              const mapped = mapLine(line, from, data.alignment);
              const offset = Math.max(0, top - block.top);
              // Keep pixel position within a visible line/fold widget; no document-wide ratio.
              target = { side, top: lineBoundary(destination, mapped) + Math.min(offset, destination.defaultLineHeight) + destination.documentPadding.top };
            }
          }
          const l = views.left, r = views.right;
          const visible = (view: EditorView) => ({ start: view.state.doc.lineAt(view.viewport.from).number - 1, end: view.state.doc.lineAt(view.viewport.to).number });
          const candidates = l && r ? visibleBlockRange(data.blocks, visible(l), visible(r)) : { start: 0, end: 0 };
          const nextBands = l && r ? data.blocks.slice(candidates.start, candidates.end).flatMap((block, offset) => {
            const index = candidates.start + offset;
            const y = (view: EditorView, line: number) => lineBoundary(view, line) + view.documentPadding.top - view.scrollDOM.scrollTop + view.scrollDOM.getBoundingClientRect().top - rect.top;
            const geometry = connectorGeometry([y(l, block.left.start), y(l, block.left.end)], [y(r, block.right.start), y(r, block.right.end)], rect.width);
            return geometry.bottom >= 0 && geometry.top <= rect.height ? [{ index, kind: block.kind, geometry }] : [];
          }) : [];
          return { width: rect.width, height: rect.height, bands: nextBands, target };
        },
        write: (measurement) => {
          if (!alive || !measurement) return;
          middleWidth = measurement.width; middleHeight = measurement.height; bands = measurement.bands;
          if (measurement.target) {
            const { side, top } = measurement.target; const view = views[side];
            if (view && Math.abs(view.scrollDOM.scrollTop - top) > 0.5) {
              view.scrollDOM.scrollTop = top;
              expected.set(side, view.scrollDOM.scrollTop);
            }
          }
        },
      });
    }
    for (const side of ['left', 'right'] as const) {
      const value = contents[side];
      if (value.kind !== 'text') continue;
      // CM normalizes CRLF/LF in its document; Rust line/UTF-16 ranges exclude EOL.
      const state = EditorState.create({ doc: value.text.replace(/\r\n/g, '\n'), extensions: [editorExtensions(side),
        EditorState.lineSeparator.of('\n'),
        EditorView.cspNonce.of(nonce.startsWith('__TAURI_') ? '' : nonce),
        EditorView.contentAttributes.of({ 'aria-label': t(side) }),
        compartments[side].of([]),
        markers[side].of([]),
        EditorView.updateListener.of(update => {
          if (update.selectionSet) { active = side; selected = -1; }
          if (update.geometryChanged || update.viewportChanged) measure();
        }),
      ] });
      const view = new EditorView({ state, parent: side === 'left' ? leftHost : rightHost });
      views[side] = view;
      view.scrollDOM.scrollTop = preservedScroll(previousFileKey, currentFileKey, savedScroll[side]);
      view.dispatch({ effects: compartments[side].reconfigure(EditorView.decorations.of(diffDecorations(view.state, side, data, expanded, expand))) });
      view.scrollDOM.addEventListener('scroll', () => {
        const value = expected.get(side); expected.delete(side);
        if (value === undefined || Math.abs(value - view.scrollDOM.scrollTop) > 0.5) source = side;
        measure();
      }, { passive: true });
      view.dom.addEventListener('focusin', () => { active = side; });
    }
    updateMarkers = (chosen, busy, revert, toggle) => {
      views.left?.dispatch({ effects: markers.left.reconfigure(revert ? rollbackGutter(data, busy, revert) : []) });
      views.right?.dispatch({ effects: markers.right.reconfigure(toggle ? inclusionGutter(data, chosen, busy, toggle) : []) });
    };
    navigate = direction => {
      const origin = views[active];
      if (!origin) return;
      const line = origin.state.doc.lineAt(origin.state.selection.main.head).number - 1;
      const index = nextDifference(data.blocks, line, active, direction, selected);
      if (index < 0) return;
      const block = data.blocks[index];
      for (const side of ['left', 'right'] as const) {
        const view = views[side]; if (!view) continue;
        const pos = linePosition(view.state, block[side].start);
        view.dispatch({ selection: { anchor: pos } });
      }
      active = origin === views.left ? 'left' : 'right';
      selected = index;
      origin.focus();
      // Measure both targets together; asynchronous editor scroll effects would
      // otherwise look like a fresh user scroll and fight the counterpart jump.
      source = undefined;
      origin.requestMeasure({
        read: () => alive ? (['left', 'right'] as const).flatMap(side => {
          const view = views[side];
          return view ? [{ side, top: lineBoundary(view, block[side].start) + view.documentPadding.top - view.scrollDOM.clientHeight / 2 }] : [];
        }) : [],
        write: targets => {
          if (!alive) return;
          for (const { side, top } of targets) {
            const view = views[side]; if (!view) continue;
            view.scrollDOM.scrollTop = Math.max(0, top);
            expected.set(side, view.scrollDOM.scrollTop);
          }
          measure();
        },
      });
    };
    const actions = createActionRegistry();
    function currentBlock() {
      if(selected>=0)return selected;
      const view=views[active];if(!view)return -1;
      const line=view.state.doc.lineAt(view.state.selection.main.head).number-1;
      return data.blocks.findIndex(block=>line>=block[active].start && line<=Math.max(block[active].start,block[active].end-1));
    }
    actions.register('Vcs.RollbackChangedLines',{run:()=>{const index=currentBlock();if(index>=0)onrevert?.(index);},enabled:()=>!!onrevert && !disabled && currentBlock()>=0,allowInInput:true});
    actions.register('NextDiff', { run: () => navigate?.(1), allowInInput: true });
    actions.register('PreviousDiff', { run: () => navigate?.(-1), allowInInput: true });
    const keydown = (event: KeyboardEvent) => actions.dispatch(event, keymapFor(detectPlatform(navigator.platform)));
    host.addEventListener('keydown', keydown);
    const observer = new ResizeObserver(measure); observer.observe(host); observer.observe(middle);
    resizeMeasure = measure;
    measure();
    return () => {
      alive = false; observer.disconnect(); host.removeEventListener('keydown', keydown);
      for (const side of ['left', 'right'] as const) { const view = views[side]; if (view) { savedScroll[side] = view.scrollDOM.scrollTop; view.destroy(); } }
      previousFileKey = currentFileKey;
      navigate = undefined; resizeMeasure = undefined; updateMarkers=undefined;
    };
  });
  $effect(()=> { updateMarkers?.(included,disabled,onrevert,onblock); });
  function resize(event: PointerEvent) {
    if (!drag || drag.id !== event.pointerId) return;
    ratio = constrainSplit(drag.first + event.clientX - drag.start, drag.width, 80, 80).first / Math.max(1, drag.width);
    resizeMeasure?.();
  }
  function startResize(event: PointerEvent) {
    if (event.button || (event.target instanceof HTMLElement && event.target.closest('button'))) return;
    event.preventDefault(); middle.focus(); middle.setPointerCapture(event.pointerId);
    const width = Math.max(1, host.clientWidth - middle.clientWidth);
    drag = { id: event.pointerId, start: event.clientX, width, first: ratio * width };
  }
  function stopResize(event: PointerEvent) {
    if (drag?.id !== event.pointerId) return;
    drag = undefined;
    if (middle.hasPointerCapture(event.pointerId)) middle.releasePointerCapture(event.pointerId);
  }
  function resizeKey(event: KeyboardEvent) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault(); ratio = Math.max(0.1, Math.min(0.9, ratio + (event.key === 'ArrowLeft' ? -0.02 : 0.02)));
    resizeMeasure?.();
  }
</script>

<div class="diff-view" bind:this={host} role="region" aria-label={t('diff')}>
  <div class="navigation">
    <button type="button" title="Shift+F7" disabled={!result.blocks.length || selected === 0} onclick={() => navigate?.(-1)}>{t('previous')}</button>
    <button type="button" title="F7" disabled={!result.blocks.length || selected === result.blocks.length - 1} onclick={() => navigate?.(1)}>{t('next')}</button>
  </div>
  <div class="panes" style:grid-template-columns={`${ratio}fr var(--diff-divider-width) ${1-ratio}fr`}>
    {#snippet pane(value: Content)}
      {#if value.kind !== 'text'}
        <div class="special">{t(value.kind)} · {value.byteLen} {t('bytes')}{#if value.kind === 'tooLarge'} / {value.limit} {t('bytes')}{/if}</div>
      {:else if value.empty}<div class="empty-state">{t('empty')}</div>{/if}
    {/snippet}
    <h3 class="left">{t('left')}</h3>
    <section class="pane left" aria-label={t('left')}>
      {@render pane(leftContent)}
      <div class="editor" bind:this={leftHost} hidden={leftContent.kind !== 'text'}></div>
    </section>
    <footer class="left">{#if leftContent.kind === 'text' && leftContent.missingFinalNewline}{t('noNewline')}{/if}</footer>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (Keyboard accessible splitter, same pattern as the shared Splitter.) -->
    <div class="middle" bind:this={middle} role="separator" tabindex="0" aria-label={t('resize')} aria-orientation="vertical" aria-valuemin="10" aria-valuemax="90" aria-valuenow={Math.round(ratio * 100)}
      onpointerdown={startResize} onpointermove={resize} onpointerup={stopResize} onpointercancel={stopResize} onlostpointercapture={stopResize} onkeydown={resizeKey}>
      <svg width="100%" height="100%" viewBox={`0 0 ${middleWidth} ${middleHeight}`} preserveAspectRatio="none" aria-hidden="true">
        {#each bands as band (band.index)}
          <g class={`band ${band.kind}`}>
            <path class="fill" d={band.geometry.path} />
            <path class="borders" d={band.geometry.borders} />
          </g>
        {/each}
      </svg>
    </div>
    <h3 class="right">{t('right')}</h3>
    <section class="pane right" aria-label={t('right')}>
      {@render pane(rightContent)}
      <div class="editor" bind:this={rightHost} hidden={rightContent.kind !== 'text'}></div>
    </section>
    <footer class="right">{#if rightContent.kind === 'text' && rightContent.missingFinalNewline}{t('noNewline')}{/if}</footer>
  </div>
</div>
<style>
  .diff-view { height: 100%; display: flex; flex-direction: column; min-height: 0; color: var(--text-primary); background: var(--background-primary); }
  .navigation { display: flex; gap: var(--space-inline); padding: var(--space-panel-y); border-bottom: var(--border-width) solid var(--separator); }
  button { font: inherit; color: var(--text-primary); background: var(--background-secondary); border: var(--border-width) solid var(--separator); }
  button:disabled { color: var(--text-disabled); }
  button:focus-visible, .middle:focus-visible { outline: var(--focus-width) solid var(--focus-border); outline-offset: calc(-1 * var(--focus-width)); }
  .panes { display: grid; grid-template-rows: auto minmax(0, 1fr) auto; flex: 1; min-height: 0; min-width: 0; }
  .pane { position: relative; display: flex; flex-direction: column; grid-row: 2; min-width: 0; min-height: 0; overflow: hidden; }
  .left { grid-column: 1; }
  .right { grid-column: 3; }
  h3 { grid-row: 1; min-width: 0; font: inherit; margin: 0; padding: var(--space-panel-y) var(--space-inline); background: var(--background-secondary); }
  .editor { flex: 1; min-height: 0; overflow: hidden; }
  .special { flex: 1; padding: var(--space-panel-x); color: var(--text-secondary); }
  .empty-state { position: absolute; top: 50%; left: var(--space-panel-x); color: var(--text-secondary); pointer-events: none; z-index: 1; }
  footer { grid-row: 3; min-height: calc(var(--font-small-size) * var(--line-height-ui)); font-size: var(--font-small-size); color: var(--text-secondary); }
  .middle { grid-column: 2; grid-row: 2; min-height: 0; position: relative; overflow: hidden; background: var(--diff-gutter-bg); cursor: col-resize; touch-action: none; }
  svg { position: absolute; inset: 0; pointer-events: none; }
  .band.insert { fill: var(--diff-added-bg); stroke: var(--diff-gutter-added); }
  .band.delete { fill: var(--diff-removed-bg); stroke: var(--diff-gutter-removed); }
  .band.modify { fill: var(--diff-modified-bg); stroke: var(--diff-gutter-modified); }
  .fill { stroke: none; }
  .borders { fill: none; stroke-width: var(--border-width); }
  :global(.diff-view .cm-gutters-before:empty) { display: none; }
  :global(.diff-view .commit-inclusion), :global(.diff-view .diff-rollback) { min-width: var(--diff-marker-width); }
  :global(.diff-view .diff-rollback button) { padding: 0; width: var(--diff-marker-width); border: 0; background: none; color: var(--text-secondary); font: inherit; cursor: pointer; }
  :global(.diff-view .diff-rollback button:disabled) { color: var(--text-disabled); cursor: default; }
  :global(.diff-view .diff-rollback button:focus-visible), :global(.diff-view .commit-inclusion input:focus-visible) { outline: var(--focus-width) solid var(--focus-border); }
  :global(.diff-view .commit-inclusion .cm-gutterElement), :global(.diff-view .diff-rollback .cm-gutterElement) { padding: 0; min-width: var(--diff-marker-width); text-align: center; }
  :global(.diff-view .cm-editor) { height: 100%; color: var(--text-primary); background: var(--background-primary); }
  :global(.diff-view .cm-scroller) { overflow: auto; font-family: var(--font-editor-family); font-size: var(--font-editor-size); line-height: var(--font-editor-line-height); }
  :global(.diff-view .commit-inclusion input) { margin: 0; vertical-align: middle; width: var(--checkbox-size); height: var(--checkbox-size); accent-color:var(--accent); }
  :global(.diff-view .cm-gutters) { color: var(--diff-gutter-fg); background: var(--diff-gutter-bg); border-color: var(--separator); }
  :global(.diff-view .cm-focused) { outline: var(--focus-width) solid var(--focus-border); }
  :global(.diff-view .cm-selectionBackground), :global(.diff-view .cm-content ::selection) { background: var(--selection-bg); }
  :global(.diff-view .diff-insert) { background: var(--diff-added-bg); }
  :global(.diff-view .diff-delete) { background: var(--diff-removed-bg); }
  :global(.diff-view .diff-modify) { background: var(--diff-modified-bg); }
  :global(.diff-view .diff-inline-left) { background: var(--diff-removed-inline); }
  :global(.diff-view .diff-inline-right) { background: var(--diff-added-inline); }
  :global(.diff-view .diff-modify .diff-inline) { background: var(--diff-modified-inline); }
  :global(.diff-view .diff-fold) { display: block; width: 100%; padding: 0; border: 0; font: inherit; line-height: inherit; color: var(--diff-gutter-fg); background: var(--diff-gutter-bg); cursor: pointer; }
</style>
