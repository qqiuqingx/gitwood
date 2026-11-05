<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import type { RepoPath } from '../browse/types';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import { browseText as b } from '../../i18n/browse';
  import { commitText as c } from '../../i18n/commit';
  let { browser, paths, expected, close }: { browser: RepositoryBrowser; paths: RepoPath[]; expected: string; close: () => void } = $props();
  let dialog: HTMLDialogElement;
  let failure = $state('');
  onMount(() => { dialog.showModal(); });
  async function remove() {
    browser.commit.busy = true; failure = '';
    try {
      await invoke('changes_delete', { generation: browser.generation, paths, expected });
      dialog.close();
    } catch (error) { failure = String((error as { detail?: string }).detail ?? error); }
    finally { browser.commit.busy = false; await browser.refresh({ status: true, refs: false, history: false }); }
  }
</script>
<dialog bind:this={dialog} onclose={close} aria-labelledby="delete-title" oncancel={event => { if (browser.commit.busy) event.preventDefault(); }}>
  <h2 id="delete-title">{b('delete')}</h2>
  <p>{b('deleteChangesQuestion')}</p>
  <ul>{#each paths as path}<li>{path.display}</li>{/each}</ul>
  {#if failure}<pre role="alert">{failure}</pre>{/if}
  <footer><button disabled={browser.commit.busy} onclick={() => dialog.close()}>{c('cancel')}</button><button disabled={browser.commit.busy} onclick={() => void remove()}>{b('delete')}</button></footer>
</dialog>
<style>
dialog { color:var(--text-primary); background:var(--popup-bg); border:var(--border-width) solid var(--popup-border); width:min(80vw,calc(var(--font-ui-size)*42)); }
li { overflow-wrap:anywhere; } footer { display:flex; justify-content:end; gap:var(--space-inline); padding-top:var(--space-panel-x); } pre { white-space:pre-wrap; } button {font:inherit;color:var(--button-fg);background:var(--button-bg);border:var(--border-width) solid var(--button-border);}
</style>
