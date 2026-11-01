<script lang="ts">
  import CheckBox from '../../components/CheckBox.svelte';
  import Highlight from '../../components/Highlight.svelte';
  import { browseText as t } from '../../i18n/browse';
  import type { Inclusion } from '../commit/model';
  import type { FileRow } from './PagedFileTree.svelte';
  import { fileStatus } from './model';
  import { fileCount, fileGroup, filePathParts, fileTypeIcon } from './fileTree';
  import { ideaIcon } from '../../components/icons/idea';
  import { appearance } from '../../theme/appearance.svelte';
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
{#snippet image(name: string)}<img class="file-icon" src={ideaIcon(name, appearance.dark)} alt="" width="16" height="16" />{/snippet}
{#if group && file.id === 'files' && root}
  {@render image('module')}
  <span class="name"><Highlight text={root} {query} /></span>
  <span class="note"><Highlight text={fileCount(file.count)} {query} /></span>
{:else if group}
  <strong><Highlight text={t(file.id as 'files' | 'changes' | 'unversioned')} {query} /></strong>
  <span class="note"><Highlight text={fileCount(file.count)} {query} /></span>
{:else}
  {#if file.path}{@render image(icon)}
  {:else if directory}{@render image('folder')}{/if}
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
  .file-icon { flex: none; width: var(--file-icon-size); height: var(--file-icon-size); }
  .conflicted { color: var(--status-conflicted); }
</style>
