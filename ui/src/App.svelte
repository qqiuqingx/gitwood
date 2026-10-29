<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke, isTauri } from '@tauri-apps/api/core';
  import Panel from './components/Panel.svelte';
  import Splitter from './components/Splitter.svelte';
  import { detectLocale, translate } from './i18n';

  const locale = detectLocale(navigator.language);
  const t = (key: Parameters<typeof translate>[1]) => translate(locale, key);
  let version = $state('');

  onMount(() => {
    document.documentElement.lang = locale;
    if (isTauri()) {
      invoke<string>('app_version').then((value) => { version = value; }).catch(console.error);
    }
  });
</script>

<main>
  <header><strong>Gitwood</strong>{#if version}<span>{version}</span>{/if}</header>
  <div class="workspace">
    <Splitter axis="y" label={t('workspaceLog')} minFirst={240} minSecond={240} initialRatio={0.59}>
      {#snippet first()}
        <Splitter label={t('commitDiff')} minFirst={220} minSecond={420} initialRatio={0.22}>
          {#snippet first()}<Panel title={t('commit')} />{/snippet}
          {#snippet second()}
            <Panel title={t('diff')}>
              <Splitter label={t('diffVersions')} minFirst={200} minSecond={200}>
                {#snippet first()}<Panel title={t('diffLeft')} />{/snippet}
                {#snippet second()}<Panel title={t('diffRight')} />{/snippet}
              </Splitter>
            </Panel>
          {/snippet}
        </Splitter>
      {/snippet}
      {#snippet second()}
        <Panel title={t('log')}>
          <Splitter label={t('branchesCommits')} minFirst={160} minSecond={490} initialRatio={0.18}>
            {#snippet first()}<Panel title={t('branches')} />{/snippet}
            {#snippet second()}
              <Splitter label={t('commitsInformation')} minFirst={260} minSecond={220} initialRatio={0.7}>
                {#snippet first()}<Panel title={t('commits')} />{/snippet}
                {#snippet second()}
                  <Splitter axis="y" label={t('filesDetails')} minFirst={90} minSecond={90} initialRatio={0.6}>
                    {#snippet first()}<Panel title={t('files')} />{/snippet}
                    {#snippet second()}<Panel title={t('details')} />{/snippet}
                  </Splitter>
                {/snippet}
              </Splitter>
            {/snippet}
          </Splitter>
        </Panel>
      {/snippet}
    </Splitter>
  </div>
</main>

<style>
  main { display: flex; flex-direction: column; height: 100%; }
  header {
    display: flex;
    align-items: center;
    gap: var(--space-inline);
    flex: none;
    padding: var(--space-header-y) var(--space-panel-x);
    background: var(--background-header);
    border-bottom: var(--border-width) solid var(--separator);
  }
  strong { font-weight: var(--font-weight-heading); }
  span { color: var(--text-secondary); font-size: var(--font-small-size); }
  .workspace { flex: 1; min-width: 0; min-height: 0; }
</style>
