<script lang="ts">
  import CheckBox from '../../components/CheckBox.svelte';
  import Highlight from '../../components/Highlight.svelte';
  import { browseText as t } from '../../i18n/browse';
  import type { Inclusion } from '../commit/model';
  import type { FileRow } from './PagedFileTree.svelte';
  import { fileStatus } from './model';
  import { fileCount, fileGroup, filePathParts, fileTypeIcon } from './fileTree';
  let { file, flat, changes, root = '', state = 'none', ontoggle, query }: {
    file: FileRow; flat: boolean; changes: boolean; root?: string; state?: Inclusion;
    ontoggle?: () => void; query: string;
  } = $props();
  const group = $derived(fileGroup(file.id));
  const directory = $derived(!group && !file.path && !file.id.startsWith('loading:'));
  const parts = $derived(filePathParts(file.path?.display ?? file.label));
  const icon = $derived(fileTypeIcon(parts.name));
  const details = $derived([file.path?.display ?? file.label, file.note,
    file.staged ? t('staged') : '', file.unstaged ? t('unstaged') : '', file.conflicted ? t('conflicted') : ''].filter(Boolean).join(' · '));
</script>
{#if changes && ontoggle}<CheckBox {state} label={file.label} disabled={file.conflicted || file.id.startsWith('loading:')} onchange={ontoggle} />{/if}
{#snippet folder(module: boolean)}<svg class="file-icon folder" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path d="M1.5 3.5h4.5l1.5 1.5h7v8.5h-13z" fill="none" stroke="currentColor" />{#if module}<rect class="module" x="9" y="9" width="5" height="5" rx="1" />{/if}</svg>{/snippet}
{#if group && file.id === 'files' && root}
  {@render folder(true)}
  <span class="name"><Highlight text={root} {query} /></span>
  <span class="note"><Highlight text={fileCount(file.count)} {query} /></span>
{:else if group}
  <strong><Highlight text={t(file.id as 'files' | 'changes' | 'unversioned')} {query} /></strong>
  <span class="note"><Highlight text={fileCount(file.count)} {query} /></span>
{:else}
  {#if file.path}<span class="file-icon" style:color={`var(--${icon.color})`} aria-hidden="true">{icon.glyph}</span>
  {:else if directory}{@render folder(false)}{/if}
  <span class="name" style:color={file.status ? `var(--status-${fileStatus(file)})` : undefined} title={details}><Highlight text={file.path ? parts.name : file.label} {query} /></span>
  {#if directory}<span class="note"><Highlight text={fileCount(file.count)} {query} /></span>{/if}
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
  .module { fill: var(--accent); stroke: var(--background-primary); }
  .conflicted { color: var(--status-conflicted); }
</style>
