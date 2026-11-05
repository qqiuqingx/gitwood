<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import List from '../../components/List.svelte';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  import { stashText as t } from '../../i18n/stash';
  import { requestResolution } from '../conflicts/model';
  import { notifications } from '../notifications/center.svelte';

  interface Entry { selector: string; oid: string; subject: string }
  interface Page { entries: Entry[]; start: number; total: number; listToken: string }
  type Action = { action: 'apply' | 'pop'; entry: Entry; reinstateIndex: boolean } | { action: 'drop'; entry: Entry } | { action: 'clear'; listToken: string } | { action: 'branch'; entry: Entry; name: string };
  let { browser, onclose }: { browser: RepositoryBrowser; onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  let rows = $state.raw(new Map<number, Entry>()), selected = $state<Entry | null>(null);
  let total = $state(0), listToken = $state(''), diff = $state(''), busy = $state(false), loading = $state(true), failure = $state('');
  let reinstateIndex = $state(false), confirmation = $state<Action | null>(null), branchEntry = $state<Entry | null>(null), branchName = $state('');
  let confirmationCancel = $state<HTMLButtonElement>();
  let ticket = 0;
  const allowed = $derived(!busy && !browser.commit.busy && !browser.status?.operation && !browser.status?.conflicts && !browser.status?.update_stash);
  onMount(() => { dialog.showModal(); void load(0, 40); });
  async function load(start: number, end: number) {
    const current = ++ticket;
    try {
      const page = await invoke<Page>('stash_page', { generation: browser.generation, start, end: Math.min(end, start + 200) });
      if (current !== ticket) return;
      if (listToken && listToken !== page.listToken) { rows = new Map(); selected = null; diff = ''; }
      listToken = page.listToken; total = page.total;
      const next = new Map(rows); page.entries.forEach((entry, offset) => next.set(page.start + offset, entry)); rows = next; loading = false;
    } catch (error) { if (current === ticket) { failure = details(error); loading = false; } }
  }
  function details(error: unknown) { return String((error as { detail?: string }).detail ?? error); }
  async function askConfirmation(action: Action) {
    confirmation = action;
    await tick();
    confirmationCancel?.focus();
  }
  async function view(entry: Entry | null) {
    selected = entry; diff = '';
    if (!entry) return;
    try { diff = await invoke<string>('stash_diff', { generation: browser.generation, entry }); }
    catch (error) { failure = details(error); }
  }
  async function execute(action: Action) {
    if (!allowed) return;
    busy = true; failure = ''; browser.commit.busy = true; browser.commit.task = t('unstash');
    try {
      const result = await invoke<{ kind: 'completed' | 'conflicts'; files?: { display: string }[] }>('stash_write', {
        generation: browser.generation, expected: browser.status?.fingerprint ?? '', action,
      });
      confirmation = null; branchEntry = null; branchName = '';
      if (result.kind === 'conflicts') {
        requestResolution();
        notifications.notify({ kind: 'warning', title: t('conflict'), detail: result.files?.map(file => file.display).join('\n') ?? '', group: 'git' });
      } else {
        const title = action.action === 'apply' ? t('apply') : action.action === 'pop' ? t('pop') : action.action === 'drop' ? t('confirmDrop') : action.action === 'clear' ? t('confirmClear') : t('branchCreate');
        notifications.notify({ kind: 'success', title, detail: 'entry' in action ? action.entry.subject : '', group: 'git' });
      }
      await browser.refresh({ status: true, refs: true, history: true });
      rows = new Map(); listToken = ''; total = 0; selected = null; diff = ''; loading = true;
      await load(0, 40);
    } catch (error) { failure = details(error); }
    finally { busy = false; browser.commit.busy = false; }
  }
</script>
<dialog bind:this={dialog} class="manager" onclose={onclose} aria-labelledby="stashes-title" oncancel={event => { if (busy) event.preventDefault(); }}>
  <h2 id="stashes-title">{t('list')}</h2>
  <div class="stash-grid">
    <div class="list">{#if !loading && !total}<p class="empty">{t('empty')}</p>{/if}<List label={t('list')} rowCount={total} getRow={index => ({ id: String(index), label: rows.get(index)?.subject ?? t('loading') })} search={false}
      onrangechange={range => { if (range.end > range.start) void load(range.start, range.end); }}
      onselectionchange={(_, index) => { void view(rows.get(index) ?? null); }} onactivate={entry => { void view(rows.get(Number(entry.id)) ?? null); }} /></div>
    <pre class="diff" aria-label={t('viewDiff')}>{diff || (selected ? t('diffEmpty') : '')}</pre>
  </div>
  <label><input type="checkbox" bind:checked={reinstateIndex} disabled={!allowed || !!branchEntry} />{t('reinstate')}</label>
  {#if failure}<pre class="error" role="alert">{failure}</pre>{/if}
  {#if busy}<progress aria-label={t('loading')}></progress>{/if}
  <footer>
    <button disabled={!allowed || !selected} onclick={() => selected && void execute({ action: 'apply', entry: selected, reinstateIndex })}>{t('apply')}</button>
    <button disabled={!allowed || !selected} onclick={() => selected && void execute({ action: 'pop', entry: selected, reinstateIndex })}>{t('pop')}</button>
    <button disabled={!allowed || !selected} onclick={() => selected && void askConfirmation({ action: 'drop', entry: selected })}>{t('drop')}</button>
    <button disabled={!allowed || !selected} onclick={() => { branchEntry = selected; branchName = ''; }}>{t('branch')}</button>
    <button disabled={!allowed || !total || loading} onclick={() => void askConfirmation({ action: 'clear', listToken })}>{t('clear')}</button>
    <span class="spacer"></span><button onclick={() => dialog.close()}>{t('cancel')}</button>
  </footer>
  {#if branchEntry}<section class="confirm"><label>{t('branchName')}<input bind:value={branchName} /></label><footer><button onclick={() => branchEntry = null}>{t('cancel')}</button><button disabled={!allowed || !branchName.trim()} onclick={() => branchEntry && void execute({ action: 'branch', entry: branchEntry, name: branchName })}>{t('branchCreate')}</button></footer></section>{/if}
  {#if confirmation}<section class="confirm" role="alertdialog" aria-modal="true">
    <p>{confirmation.action === 'drop' ? t('dropQuestion').replace('{selector}', confirmation.entry.selector) : t('clearQuestion').replace('{count}', String(total))}</p>
    <footer><button bind:this={confirmationCancel} disabled={busy} onclick={() => confirmation = null}>{t('cancel')}</button><button disabled={!allowed} onclick={() => void execute(confirmation!)}>{confirmation.action === 'drop' ? t('confirmDrop') : t('confirmClear')}</button></footer>
  </section>{/if}
</dialog>
<style>
.manager { color:var(--text-primary); background:var(--popup-bg); border:var(--border-width) solid var(--popup-border); width:min(92vw,calc(var(--font-ui-size)*70)); height:min(82vh,calc(var(--font-ui-size)*46)); }
h2 { margin-block-start:0; } .stash-grid { display:grid; grid-template-columns:minmax(220px, 0.8fr) minmax(0,1.2fr); height:calc(100% - var(--list-row-height)*5); border:var(--border-width) solid var(--separator); min-height:0; }
.list { min-height:0; border-inline-end:var(--border-width) solid var(--separator); } .diff { margin:0; min-width:0; overflow:auto; white-space:pre; font-family:var(--font-editor-family); }
.empty { padding:var(--space-inline); color:var(--text-secondary); }
footer { display:flex; gap:var(--space-inline); justify-content:end; flex-wrap:wrap; padding-block:var(--space-panel-y); } .spacer { flex:1; }
.confirm { position:absolute; inset:25% 20%; padding:var(--space-panel-x); border:var(--border-width) solid var(--popup-border); background:var(--background-primary); box-shadow:0 4px 18px var(--popup-shadow); }
.confirm p { overflow-wrap:anywhere; } .confirm input { font:inherit; color:var(--input-fg); background:var(--input-bg); border:var(--border-width) solid var(--input-border); }
.error { color:var(--status-conflicted); white-space:pre-wrap; max-height:20%; overflow:auto; }
</style>
