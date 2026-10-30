<script lang="ts">
  import { onMount } from 'svelte';
  import { browseText as t } from '../../i18n/browse';
  import type { RepositoryError } from './types';
  let { error, close, projectOpen }: { error: RepositoryError; close: () => void; projectOpen: boolean } = $props();
  let dialog: HTMLDialogElement;
  const message = $derived(error.key === 'repository.unsafeDirectory' ? t('unsafeDirectory') : error.key === 'repository.bareUnsupported' ? t('bareUnsupported') : projectOpen ? t('error') : t('notRepository'));
  onMount(() => { const previous = document.activeElement; dialog.showModal(); return () => { if (previous instanceof HTMLElement) previous.focus({ preventScroll: true }); }; });
</script>
<dialog bind:this={dialog} onclose={close} aria-labelledby="repository-error-title">
  <h2 id="repository-error-title">{projectOpen ? t('error') : t('errorTitle')}</h2>
  <p>{message}</p>{#if error.path}<p>{error.path}</p>{/if}
  {#if error.detail}<pre>{error.detail}</pre>{/if}
  <footer><button onclick={() => dialog.close()}>{t('ok')}</button></footer>
</dialog>
<style>
  dialog { max-width: min(80vw, calc(var(--font-ui-size) * 48)); color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  h2 { font-size: var(--font-ui-size); margin: 0; }
  p, pre { white-space: pre-wrap; overflow-wrap: anywhere; }
  pre { max-height: 30vh; overflow: auto; font-size: var(--font-small-size); }
  footer { display: flex; justify-content: end; }
  button { font: inherit; background: var(--button-bg); color: var(--button-fg); border: var(--border-width) solid var(--button-border); padding: var(--space-panel-y) var(--space-panel-x); }
</style>
