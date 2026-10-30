<script lang="ts">
  import { onMount } from 'svelte';
  import type { CommitResult } from './model';
  import { commitText as t } from '../../i18n/commit';
  let { result, close }: { result: Extract<CommitResult,{kind:'recovery'}>; close: () => void } = $props();
  let dialog: HTMLDialogElement;
  onMount(()=>{const previous=document.activeElement;dialog.showModal();return()=>{if(previous instanceof HTMLElement)previous.focus();};});
</script>
<dialog bind:this={dialog} onclose={close} aria-labelledby="index-recovery-title"><h2 id="index-recovery-title">{t('recovery')}</h2><p>{result.hash}</p><p>{result.directory}</p><p>{result.reason}</p><pre>{result.instructions}</pre><button onclick={()=>dialog.close()}>{t('close')}</button></dialog>
<style>
dialog {max-width:80vw;color:var(--text-primary);background:var(--popup-bg);border:var(--border-width) solid var(--popup-border);}
h2 {font-size:var(--font-ui-size);} p,pre {white-space:pre-wrap;overflow-wrap:anywhere;} pre {max-height:50vh;overflow:auto;} button {font:inherit;color:var(--button-fg);background:var(--button-bg);border:var(--border-width) solid var(--button-border);}
</style>
