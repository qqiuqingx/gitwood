<script lang="ts">
  import Popup from '../../components/Popup.svelte';
  import ProjectAvatar from './ProjectAvatar.svelte';
  import { projectName } from './avatar';
  import { projectPopupItems } from './model';
  import { projectText as t } from '../../i18n/projects';
  import { browseText } from '../../i18n/browse';
  import type { RepositoryBrowser } from '../browse/browser.svelte';
  let { browser, open, select, busy = false }: { browser: RepositoryBrowser; open: () => void; select: (path: string) => void; busy?: boolean } = $props();
  let anchor = $state<HTMLButtonElement>();
  let shown = $state(false);
  const items = $derived(projectPopupItems(browser.project?.root, browser.recent,
    { new: t('new'), open: t('open'), clone: t('clone'), opened: t('opened'), recent: t('recent'), failed: browseText('failed') },
    open, path => {
      // Selecting this window's project just closes the popup; retain its draft and viewport.
      if (path !== browser.project?.root) select(path);
    }));
  const avatars = $derived(new Map(items.flatMap(item => item.avatar ? [[item.id, item.avatar] as const] : [])));
</script>
<button bind:this={anchor} aria-label={t('projects')} aria-haspopup="dialog" aria-expanded={shown}
  disabled={browser.opening || busy} onclick={() => { shown = !shown; }}>
  {#if browser.project}<ProjectAvatar path={browser.project.root} name={projectName(browser.project.root)} />{/if}
  <strong>{browser.project ? projectName(browser.project.root) : t('open')}</strong><span aria-hidden="true">⌄</span>
</button>
{#snippet projectIcon(id: string)}{@const avatar = avatars.get(id)}{#if avatar}<ProjectAvatar {...avatar} />{/if}{/snippet}
{#if shown && anchor}<div class="project-popup"><Popup items={items.map(item => item.avatar ? { ...item, icon: projectIcon } : item)} label={t('projects')} {anchor} filter
  onclose={() => { shown = false; }} /></div>{/if}
<style>
  button { display: inline-flex; align-items: center; gap: var(--space-inline); font: inherit; border: 0;
    background: none; color: inherit; padding: var(--space-panel-y) var(--space-inline); }
  button:hover { background: var(--toolbar-hover-bg); }
  strong { font-weight: var(--font-weight-heading); }
  .project-popup { --popup-height: calc(var(--font-ui-size) * 54); --popup-width: calc(var(--font-ui-size) * 28); }
</style>
