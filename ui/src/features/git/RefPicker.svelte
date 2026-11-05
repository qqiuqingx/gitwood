<script lang="ts">
  import Popup from '../../components/Popup.svelte';
  import { integrationText as t } from '../../i18n/integration';
  import type { Reference } from '../browse/types';
  let { value = $bindable(''), refs, label, editable = false, disabled = false }: { value?: string; refs: Reference[]; label: string; editable?: boolean; disabled?: boolean } = $props();
  let anchor = $state<HTMLButtonElement>();
  let open = $state(false);
  const items = $derived(refs.map(ref => ({ id: ref.name, label: ref.name.replace(/^refs\/(heads|remotes|tags)\//, ''), run: () => { value = ref.name; } })));
</script>
<label>{label}<span><input bind:value readonly={!editable} {disabled} /><button type="button" bind:this={anchor} {disabled} aria-label={t('browse')} aria-expanded={open} onclick={() => { open = !open; }}>⌄</button></span></label>
{#if open}<Popup {items} label={label} anchor={anchor} onclose={() => { open = false; }} />{/if}
<style>
  label { display: block; margin-block: var(--space-panel-y); } span { display: flex; }
  input { min-width: 0; flex: 1; } input, button { font: inherit; color: var(--input-fg); background: var(--input-bg); border: var(--border-width) solid var(--input-border); padding: var(--space-panel-y); }
</style>
