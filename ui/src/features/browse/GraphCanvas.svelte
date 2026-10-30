<script lang="ts">
  import { onMount } from 'svelte';
  import { graphCommands, drawGraph } from './graph';
  import type { HistoryRow } from './types';
  let { rows, scrollTop, rowHeight, height, width, head, scrollLeft = 0 }: { rows: HistoryRow[]; scrollTop: number; rowHeight: number; height: number; width: number; head: string | null; scrollLeft?: number } = $props();
  let canvas: HTMLCanvasElement;
  let themeVersion = $state(0);
  onMount(() => {
    const observer = new MutationObserver(() => { themeVersion++; });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['style'] });
    return () => observer.disconnect();
  });
  $effect(() => {
    themeVersion;
    const commands = graphCommands(rows, scrollTop, rowHeight, height, head);
    if (!canvas || height <= 0) return;
    const scale = window.devicePixelRatio || 1;
    canvas.width = Math.ceil(width * scale); canvas.height = Math.ceil(height * scale);
    const context = canvas.getContext('2d'); if (!context) return;
    context.scale(scale, scale); context.translate(-scrollLeft, 0);
    const style = getComputedStyle(document.documentElement);
    const count = Math.max(1, Number(style.getPropertyValue('--graph-lanes-count')) || 6);
    const colors = Array.from({ length: count }, (_, index) => style.getPropertyValue(`--graph-lanes-${index}`).trim());
    drawGraph(context, commands, colors, style.getPropertyValue('--graph-head'));
  });
</script>
<canvas bind:this={canvas} style:width={`${width}px`} style:height={`${height}px`} aria-hidden="true"></canvas>
<style>canvas { position: absolute; inset: 0; pointer-events: none; }</style>
