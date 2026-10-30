<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { constrainSplit } from './splitter';
  import { panelSplit } from '../layout/state';

  interface Props {
    axis?: 'x' | 'y';
    label: string;
    first: Snippet;
    second: Snippet;
    minFirst: number;
    minSecond: number;
    initialRatio?: number;
    ratio?: number;
    firstVisible?: boolean;
    secondVisible?: boolean;
  }

  let {
    axis = 'x', label, first, second, minFirst, minSecond, initialRatio = 0.5,
    ratio = $bindable(initialRatio), firstVisible = true, secondVisible = true,
  }: Props = $props();

  let container: HTMLDivElement;
  let handle: HTMLDivElement;
  let available = $state(0);
  let dragging = $state(false);
  let dragStart: { pointer: number; position: number; first: number } | undefined;
  const sizes = $derived(panelSplit(ratio, available, minFirst, minSecond, firstVisible, secondVisible));
  const tracks = $derived(`${sizes.first}px ${firstVisible && secondVisible ? 'var(--splitter-size)' : '0px'} minmax(0, 1fr)`);

  onMount(() => {
    const measure = () => {
      available = Math.max(0, axis === 'x'
        ? container.clientWidth - handle.offsetWidth
        : container.clientHeight - handle.offsetHeight);
    };
    const observer = new ResizeObserver(measure);
    observer.observe(container);
    observer.observe(handle);
    measure();
    return () => observer.disconnect();
  });

  function resize(requested: number) {
    if (available > 0 && firstVisible && secondVisible) {
      ratio = constrainSplit(requested, available, minFirst, minSecond).first / available;
    }
  }

  function start(event: PointerEvent) {
    if (event.button !== 0 || dragStart) return;
    event.preventDefault();
    handle.focus();
    handle.setPointerCapture(event.pointerId);
    dragStart = {
      pointer: event.pointerId,
      position: axis === 'x' ? event.clientX : event.clientY,
      first: sizes.first,
    };
    dragging = true;
  }

  function move(event: PointerEvent) {
    if (!dragStart || event.pointerId !== dragStart.pointer) return;
    const position = axis === 'x' ? event.clientX : event.clientY;
    resize(dragStart.first + position - dragStart.position);
  }

  function stop(event: PointerEvent) {
    if (!dragStart || event.pointerId !== dragStart.pointer) return;
    dragStart = undefined;
    dragging = false;
    if (handle.hasPointerCapture(event.pointerId)) handle.releasePointerCapture(event.pointerId);
  }

  function keyboard(event: KeyboardEvent) {
    const previous = axis === 'x' ? 'ArrowLeft' : 'ArrowUp';
    const next = axis === 'x' ? 'ArrowRight' : 'ArrowDown';
    const step = event.shiftKey ? 40 : 10;
    if (event.key === previous) resize(sizes.first - step);
    else if (event.key === next) resize(sizes.first + step);
    else if (event.key === 'Home') resize(sizes.min);
    else if (event.key === 'End') resize(sizes.max);
    else return;
    event.preventDefault();
  }
</script>

<div
  class="splitter"
  class:vertical={axis === 'y'}
  class:dragging
  bind:this={container}
  style:grid-template-columns={axis === 'x' ? tracks : undefined}
  style:grid-template-rows={axis === 'y' ? tracks : undefined}
>
  <div class="pane" hidden={!firstVisible}>{@render first()}</div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (A focusable separator is the WAI-ARIA window splitter pattern, with range values and keyboard resizing.) -->
  <div
    class="handle"
    hidden={!firstVisible || !secondVisible}
    bind:this={handle}
    role="separator"
    tabindex="0"
    aria-label={label}
    aria-orientation={axis === 'x' ? 'vertical' : 'horizontal'}
    aria-valuemin={Math.round(sizes.min)}
    aria-valuemax={Math.round(sizes.max)}
    aria-valuenow={Math.round(sizes.first)}
    onpointerdown={start}
    onpointermove={move}
    onpointerup={stop}
    onpointercancel={stop}
    onlostpointercapture={stop}
    onkeydown={keyboard}
  ></div>
  <div class="pane" hidden={!secondVisible}>{@render second()}</div>
</div>

<style>
  .splitter {
    display: grid;
    grid-template-rows: minmax(0, 1fr);
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .splitter.vertical { grid-template-columns: minmax(0, 1fr); }
  .pane { min-width: 0; min-height: 0; overflow: hidden; }
  /* Keep the surviving pane in its track when hidden siblings leave grid layout. */
  .pane:first-child { grid-column: 1; grid-row: 1; }
  .handle { grid-column: 2; grid-row: 1; }
  .pane:last-child { grid-column: 3; grid-row: 1; }
  .vertical > .handle { grid-column: 1; grid-row: 2; }
  .vertical > .pane:last-child { grid-column: 1; grid-row: 3; }
  .handle {
    background: var(--separator);
    cursor: col-resize;
    touch-action: none;
    outline-offset: calc(-1 * var(--focus-width));
  }
  .vertical > .handle { cursor: row-resize; }
  .handle:hover, .handle:focus-visible, .dragging > .handle {
    background: var(--focus-border);
  }
  .handle:focus-visible { outline: var(--focus-width) solid var(--focus-border); }
  .dragging { user-select: none; }
</style>
