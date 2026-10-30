<script lang="ts">
  import CheckBox from '../../components/CheckBox.svelte';
  import Highlight from '../../components/Highlight.svelte';
  import { browseText as t } from '../../i18n/browse';
  import type { Inclusion } from '../commit/model';
  import type { FileRow } from './PagedFileTree.svelte';
  import { fileStatus } from './model';
  import { fileCount, fileGroup, filePathParts, fileTypeIcon } from './fileTree';
  let { file, flat, changes, state = 'none', ontoggle, query }: {
    file: FileRow; flat: boolean; changes: boolean; state?: Inclusion;
    ontoggle?: () => void; query: string;
  } = $props();
  const group = $derived(fileGroup(file.id));
  const parts = $derived(filePathParts(file.path?.display ?? file.label));
  const icon = $derived(fileTypeIcon(parts.name));
  const details = $derived([file.path?.display ?? file.label, file.note,
    file.staged ? t('staged') : '', file.unstaged ? t('unstaged') : '', file.conflicted ? t('conflicted') : ''].filter(Boolean).join(' · '));
</script>
{#if changes && ontoggle}<CheckBox {state} label={file.label} disabled={file.conflicted || file.id.startsWith('loading:')} onchange={ontoggle} />{/if}
{#if group}
  <strong><Highlight text={t(file.id as 'files' | 'changes' | 'unversioned')} {query} /></strong>
  <span class="note"><Highlight text={fileCount(file.count)} {query} /></span>
{:else}
  {#if file.path}<span class="file-icon" style:color={`var(--${icon.color})`} aria-hidden="true">{icon.glyph}</span>
  {:else if !file.id.startsWith('loading:')}<span class="file-icon folder" aria-hidden="true">▱</span>{/if}
  <span class="name" style:color={file.status ? `var(--status-${fileStatus(file)})` : undefined} title={details}><Highlight text={file.path ? parts.name : file.label} {query} /></span>
  {#if flat && file.path && parts.directory}<span class="note directory" title={details}><Highlight text={parts.directory} {query} /></span>{/if}
  {#if file.note}<span class="note" title={details}>{file.note} →</span>{/if}
  {#if file.conflicted}<span class="conflicted">{t('conflicted')}</span>{/if}
{/if}
<style>
  strong { font-weight: var(--font-weight-heading); }
  .name { flex: none; }
  .note { color: var(--text-secondary); font-weight: normal; }
  .directory { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .file-icon { flex: none; width: var(--file-icon-size); font-size: var(--font-small-size); line-height: 1; text-align: center; font-family: var(--font-editor-family); }
  .folder { color: var(--text-secondary); }
  .conflicted { color: var(--status-conflicted); }
</style>
