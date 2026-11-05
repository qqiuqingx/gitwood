<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import List from '../../components/List.svelte';
  import { Compartment, EditorState } from '@codemirror/state';
  import { Decoration, EditorView, lineNumbers, keymap } from '@codemirror/view';
  import { conflictText as t } from '../../i18n/conflict';
  import { needsCloseConfirmation, replacement, type MergeContent, type MergeBlock, type Choice } from './model';
  let { content, rebase, busy, onsave, onclose }: { content: MergeContent; rebase: boolean; busy: boolean; onsave: (text: string) => void; onclose: () => void } = $props();
  let left: HTMLDivElement, center: HTMLDivElement, right: HTMLDivElement;
  let blocks = $state<MergeBlock[]>(untrack(() => content.blocks.map(b => ({ ...b }))));
  let activeBlock = $state(0);
  const highlights = new Compartment();
  const editing = new Compartment();
  let dirty = $state(false), confirm = $state(false);
  let result = $state.raw<EditorView>();
  const remaining = $derived(blocks.filter(b => !b.resolved).length);
  export function requestClose() { if (busy) return; if (needsCloseConfirmation(blocks, dirty)) confirm = true; else onclose(); }
  function choose(index: number, choice: Choice) {
    const block = blocks[index];
    if (!result || block.resolved || busy) return;
    const insert = replacement(block, choice);
    if (insert !== null) result.dispatch({ changes: { from: block.from, to: block.to, insert } });
    blocks = blocks.map((b, i) => i === index ? { ...b, resolved: true } : b);
    dirty = true;
  }
  function navigate(index: number) {
    if (!result) return;
    result.dispatch({ selection: { anchor: blocks[index].from }, effects: EditorView.scrollIntoView(blocks[index].from, { y: 'center' }) });
    result.focus();
  }
  $effect(() => {
    const current = blocks;
    const disabled = busy;
    if (result) result.dispatch({ effects: [
      editing.reconfigure([EditorState.readOnly.of(disabled), EditorView.editable.of(!disabled)]),
      highlights.reconfigure(EditorView.decorations.of(Decoration.set(current.filter(b => b.to > b.from).map(b => Decoration.mark({ class: b.resolved ? 'merge-resolved' : 'merge-unresolved' }).range(b.from, b.to)), true))),
    ] });
  });
  onMount(() => {
    const nonce = document.querySelector<HTMLMetaElement>('meta[name="gitwood-style-nonce"]')?.content ?? '';
    const create = (parent: HTMLElement, text: string, editable: boolean, label: string) => new EditorView({ parent, state: EditorState.create({
      doc: text, extensions: [EditorView.cspNonce.of(nonce), EditorState.lineSeparator.of('\n'), lineNumbers(),
        ...(editable ? [highlights.of([]), editing.of([])] : [EditorState.readOnly.of(true), EditorView.editable.of(false)]),
        EditorView.contentAttributes.of({ 'aria-label': label }),
        keymap.of([{ key: 'Mod-s', run: view => { if (editable && !busy && remaining === 0) onsave(view.state.doc.toString()); return true; } }]),
        EditorView.updateListener.of(update => {
          if (!editable || !update.docChanged) return;
          blocks = blocks.map(b => ({ ...b, from: update.changes.mapPos(b.from, -1), to: update.changes.mapPos(b.to, 1) }));
          dirty = true;
        }),
        EditorView.theme({ '&': { height: '100%', backgroundColor: 'var(--background-primary)', color: 'var(--text-primary)' }, '.merge-unresolved': { backgroundColor: 'var(--diff-conflict-bg, var(--selection-bg))' }, '.merge-resolved': { backgroundColor: 'var(--diff-insert-bg, var(--selection-bg))' }, '.cm-scroller': { overflow: 'auto', fontFamily: 'var(--font-editor-family)' }, '&.cm-focused': { outline: '1px solid var(--accent)' } }),
      ],
    }) });
    const l = create(left, content.left, false, t('local'));
    result = create(center, content.result, true, t('result'));
    const r = create(right, content.right, false, t('incoming'));
    result.focus();
    return () => { l.destroy(); result?.destroy(); r.destroy(); result = undefined; };
  });
</script>
<p role="status">{t('auto')} · {remaining} {t('unresolved')}</p>
<div class="editors">
  <section><h3>{t(rebase ? 'rebased' : 'local')}</h3><div class="editor" bind:this={left}></div></section>
  <section><h3>{t('result')}</h3><div class="editor" bind:this={center}></div></section>
  <section><h3>{t(rebase ? 'upstream' : 'incoming')}</h3><div class="editor" bind:this={right}></div></section>
</div>
<div class="blocks"><List rowCount={blocks.length} label={t('title')} getRow={index => ({ id: String(index), label: `${t('block')} ${index + 1}${blocks[index].resolved ? ' ✓' : ''}` })}
  onselectionchange={(_, index) => { activeBlock = index; }} onactivate={(_, index) => navigate(index)} /></div>
{#if blocks.length}<div>{#each ['left', 'right', 'both', 'ignore'] as choice}<button disabled={busy || blocks[activeBlock]?.resolved} onclick={() => choose(activeBlock, choice as Choice)}>{t(choice as Choice)}</button>{/each}</div>{/if}
{#if confirm}<div role="alert"><p>{t('confirm')}</p><button onclick={() => { confirm = false; }}>{t('cancel')}</button><button onclick={onclose}>{t('discard')}</button></div>{/if}
<footer><button disabled={busy} onclick={requestClose}>{t('close')}</button><button disabled={busy || remaining > 0} onclick={() => { if (result) onsave(result.state.doc.toString()); }}>{t('save')}</button></footer>
<style>
  .editors { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) minmax(0, 1fr); height: 48vh; }
  section { display: flex; flex-direction: column; min-height: 0; min-width: 0; border: 1px solid var(--separator); }
  h3 { font: inherit; margin: 0; padding: var(--space-inline); background: var(--popup-bg); }
  .editor { flex: 1; min-height: 0; overflow: hidden; }
  .blocks { height: 12vh; overflow: auto; padding-block: var(--space-inline); }
  button { font: inherit; margin: 2px; } footer { display: flex; justify-content: end; gap: var(--space-inline); }
</style>
