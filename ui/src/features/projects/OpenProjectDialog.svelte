<script lang="ts">
  import { onMount } from 'svelte';
  import { projectText as t } from '../../i18n/projects';
  import { projectName } from './avatar';
  import type { OpenTarget } from './opening.svelte';
  let { path, choose, cancel }: { path: string; choose: (target: OpenTarget) => void; cancel: () => void } = $props();
  let dialog: HTMLDialogElement;
  onMount(() => {
    const previous = document.activeElement;
    dialog.showModal();
    return () => { if (previous instanceof HTMLElement) previous.focus(); };
  });
</script>
<dialog bind:this={dialog} oncancel={cancel} aria-labelledby="open-project-title" aria-describedby="open-project-message">
  <h2 id="open-project-title">{t('openProject')}</h2>
  <p id="open-project-message">{t('openWhere').replace('{project}', projectName(path))}</p>
  <p class="path">{path}</p>
  <div class="actions">
    <button onclick={cancel}>{t('cancel')}</button>
    <button onclick={() => choose('current')}>{t('currentWindow')}</button>
    <button class="primary" onclick={() => choose('new')}>{t('newWindow')}</button>
  </div>
</dialog>
<style>
  dialog { max-width: min(80vw, calc(var(--font-ui-size) * 40)); color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  h2 { font-size: var(--font-ui-size); margin-top: 0; }
  p { overflow-wrap: anywhere; }
  .path { color: var(--text-secondary); }
  .actions { display: flex; justify-content: flex-end; gap: var(--space-inline); }
  button { font: inherit; padding: var(--space-panel-y) var(--space-panel-x); color: var(--button-fg); background: var(--button-bg); border: var(--border-width) solid var(--button-border); }
  .primary { border-color: var(--accent); }
</style>
