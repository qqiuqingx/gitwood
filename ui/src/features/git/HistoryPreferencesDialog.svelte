<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { historyMenuText as t } from '../../i18n/history-menu';
  type EmptyResolution = 'skip' | 'createEmpty';
  let { suffix, empty, onclose }: { suffix: boolean; empty: EmptyResolution; onclose: () => void } = $props();
  let element: HTMLDialogElement;
  let addSuffix = $state(untrack(() => suffix)), resolution = $state<EmptyResolution>(untrack(() => empty)), error = $state(''), busy = $state(false);
  onMount(() => { element.showModal(); });
  async function save() {
    busy = true; error = '';
    try { await invoke('save_history_preferences', { addSuffixToPublished: addSuffix, emptyCherryPick: resolution }); onclose(); }
    catch (cause) { error = String((cause as { detail?: string }).detail ?? cause); }
    finally { busy = false; }
  }
</script>
<dialog bind:this={element} aria-labelledby="history-preferences-title" oncancel={event => { event.preventDefault(); onclose(); }}>
  <h2 id="history-preferences-title">{t('preferences')}</h2>
  <fieldset><legend>{t('historyPreferences')}</legend>
    <label><input type="checkbox" bind:checked={addSuffix} /> {t('addSuffixToCherryPicks')}</label>
    <p>{t('emptyCherryPick')}</p>
    <label><input type="radio" name="empty-cherry-pick" value="skip" bind:group={resolution} /> {t('skipEmptyCherryPick')}</label>
    <label><input type="radio" name="empty-cherry-pick" value="createEmpty" bind:group={resolution} /> {t('createEmptyCherryPick')}</label>
  </fieldset>
  {#if error}<p role="alert">{t('saveFailed')}: {error}</p>{/if}
  <footer><button onclick={onclose}>{t('cancel')}</button><button class="default" disabled={busy} onclick={() => void save()}>{t('save')}</button></footer>
</dialog>
<style>
  dialog { width: min(500px, 86vw); color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  fieldset { border: var(--border-width) solid var(--separator); padding: var(--space-panel-x); } label { display: block; margin-block: var(--space-panel-y); }
  footer { display: flex; justify-content: end; gap: var(--space-inline); margin-top: var(--space-panel-x); } button { font: inherit; padding: var(--space-panel-y) var(--space-panel-x); }
</style>
