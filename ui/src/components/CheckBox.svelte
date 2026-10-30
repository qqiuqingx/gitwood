<script lang="ts">
  import type { Inclusion } from '../features/commit/model';
  let { state, label, disabled = false, onchange }: { state: Inclusion; label: string; disabled?: boolean; onchange: () => void } = $props();
  function mixed(node: HTMLInputElement, value: boolean) { node.indeterminate = value; return { update(next: boolean) { node.indeterminate = next; } }; }
</script>
<input type="checkbox" tabindex="-1" aria-label={label} aria-checked={state === 'some' ? 'mixed' : state === 'all'} checked={state === 'all'} use:mixed={state === 'some'} {disabled}
  onclick={event => { event.stopPropagation(); onchange(); (event.currentTarget.closest('[role=tree]') as HTMLElement | null)?.focus(); }} ondblclick={event => event.stopPropagation()} />
<style>input { flex:none; accent-color:var(--accent); }</style>
