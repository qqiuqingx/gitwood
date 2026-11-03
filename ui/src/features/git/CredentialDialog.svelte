<script lang="ts">
  import { tick } from 'svelte';
  import { credentialText as t } from '../../i18n/credential';
  import { credentialAnswer, credentialHost, credentialLabel, credentialSecret, type CredentialRequest } from './credential';
  let { request, answer }: { request: CredentialRequest | null; answer: (value: string | null) => void } = $props();
  let input = $state('');
  let dialog = $state<HTMLDialogElement>();
  let field = $state<HTMLInputElement>();
  const host = $derived(request ? credentialHost(request.prompt) : null);
  const label = $derived(request ? credentialLabel(request.kind) : null);
  const secret = $derived(request ? credentialSecret(request.kind) : true);
  // A new prompt always starts empty: one answer must never leak into the next question.
  $effect(() => {
    if (request) {
      input = '';
      void tick().then(() => field?.focus());
    }
  });
  // Modal so the prompt sits above the window, matching the workflow dialogs.
  $effect(() => {
    const element = dialog;
    if (!element) return;
    if (request && !element.open) element.showModal();
    if (!request && element.open) element.close();
  });
  function submit() { if (request) answer(credentialAnswer(request.kind, input)); }
</script>

{#if request}
  <dialog bind:this={dialog} class="credential-dialog" aria-labelledby="credential-title"
    oncancel={(event) => { event.preventDefault(); answer(null); }}>
    <form onsubmit={(event) => { event.preventDefault(); submit(); }}>
      <h2 id="credential-title">{t('title')}{host ? ` — ${host}` : ''}</h2>
      <p class="prompt">{request.prompt}</p>
      {#if label === null}
        <p class="hint">{t('hostKeyHint')}</p>
        <footer>
          <button type="button" onclick={() => answer(null)}>{t('cancel')}</button>
          <button type="submit" class="default">{t('continueConnection')}</button>
        </footer>
      {:else}
        <label>{t(label)}
          <input bind:this={field} bind:value={input} type={secret ? 'password' : 'text'}
            autocomplete="off" autocapitalize="off" spellcheck="false" aria-label={t(label)} />
        </label>
        <p class="hint">{t('stored')}</p>
        <footer>
          <button type="button" onclick={() => answer(null)}>{t('cancel')}</button>
          <button type="submit" class="default" disabled={credentialAnswer(request.kind, input) === null}>{t('confirm')}</button>
        </footer>
      {/if}
    </form>
  </dialog>
{/if}

<style>
  .credential-dialog { width: min(460px, 84vw); color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  .credential-dialog::backdrop { background: #0008; }
  h2 { margin-block-start: 0; font-size: calc(var(--font-ui-size) * 1.15); }
  .prompt { margin-block: 0 var(--space-panel-y); overflow-wrap: anywhere; }
  label { display: block; margin-block: var(--space-panel-y); }
  input { display: block; width: 100%; font: inherit; color: var(--input-fg); background: var(--input-bg); border: var(--border-width) solid var(--input-border); padding: var(--space-panel-y); }
  .hint { color: var(--text-secondary); font-size: var(--font-small-size); }
  footer { display: flex; justify-content: end; gap: var(--space-inline); margin-top: var(--space-panel-x); }
  footer button { font: inherit; padding: var(--space-panel-y) var(--space-panel-x); }
  footer button.default { border-color: var(--focus-border); }
</style>
