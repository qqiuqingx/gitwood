<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke, isTauri } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { commitText as c } from './i18n/commit';
  import RecoveryDialog from './features/commit/RecoveryDialog.svelte';
  import Preview from './features/diff/Preview.svelte';
  import List from './components/List.svelte';
  import CommitPanel from './features/browse/CommitPanel.svelte';
  import Branches from './features/browse/Branches.svelte';
  import HistoryList from './features/browse/HistoryList.svelte';
  import Information from './features/browse/Information.svelte';
  import ErrorDialog from './features/browse/ErrorDialog.svelte';
  import GitWorkflow from './features/git/GitWorkflow.svelte';
  import { RepositoryBrowser } from './features/browse/browser.svelte';
  import { browseText as b } from './i18n/browse';
  const browser = new RepositoryBrowser();
  let workflow: { commitAndPush: () => void; update: () => void; push: () => void };
  let browseReady = $state(false);
  async function chooseRepository() {
    if (!isTauri() || browser.opening) return;
    try {
      const path = await openDialog({ directory: true, multiple: false, title: b('open') });
      if (path) await browser.open(path);
    } catch (error) { browser.report(error); }
  }
  import Splitter from './components/Splitter.svelte';
  import { detectLocale, translate } from './i18n';
  import { defaultLayout, type Layout } from './layout/state';
  import { applyTheme, effectiveTheme, type Theme, type ThemeChoice, type ThemeError } from './theme';

  interface Settings { theme: ThemeChoice; layout: Layout }
  const locale = detectLocale(navigator.language);
  const t = (key: Parameters<typeof translate>[1]) => translate(locale, key);
  function togglePanel(panel: 'commit' | 'log') {
    void invoke('set_tool_window', { panel, visible: panel === 'commit' ? !layout.commitVisible : !layout.logVisible }).catch(error => { settingsError = String(error); });
  }
  let version = $state('');
  let layout = $state<Layout>(structuredClone(defaultLayout));
  let ready = $state(false);
  let settingsError = $state('');
  let closing = $state(false);
  let themeErrors = $state<ThemeError[]>([]);
  let lastRatios = '';
  let pendingSave = Promise.resolve();

  $effect(() => {
    const ratios = JSON.stringify(layout.ratios);
    if (!ready || closing || !isTauri() || ratios === lastRatios) return;
    lastRatios = ratios;
    // Send each UI change immediately; Rust debounces disk writes and flushes on exit.
    pendingSave = pendingSave.then(() => invoke('save_layout', { ratios: JSON.parse(ratios) }))
      .then(() => {}).catch((error) => { settingsError = String(error); });
  });

  onMount(() => {
    document.documentElement.lang = locale;
    if (!isTauri()) return;
    const system = window.matchMedia('(prefers-color-scheme: dark)');
    let choice: ThemeChoice = 'system';
    let active = '';
    let generation = 0;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    let unlistenExit: (() => void) | undefined;
    async function refreshTheme() {
      const name = effectiveTheme(choice, system.matches);
      if (name === active) return;
      active = name;
      const request = ++generation;
      try {
        const theme = await invoke<Theme>('get_theme', { name });
        if (!disposed && request === generation) { applyTheme(theme); themeErrors = theme.errors; }
      } catch (error) {
        if (!disposed && request === generation) { active = ''; settingsError = String(error); }
      }
    }
    const systemChanged = () => { if (choice === 'system') void refreshTheme(); };
    system.addEventListener('change', systemChanged);
    async function initialize() {
      unlisten = await listen<Settings>('preferences-changed', ({ payload }) => {
        // Do not replace current ratios with a backend snapshot while dragging.
        layout.commitVisible = payload.layout.commitVisible;
        layout.logVisible = payload.layout.logVisible;
        choice = payload.theme;
        void refreshTheme();
      });
      if (disposed) { unlisten(); return; }
      unlistenError = await listen<string>('settings-error', ({ payload }) => { settingsError = payload; });
      if (disposed) { unlisten(); unlistenError(); return; }
      unlistenExit = await listen('save-before-exit', () => {
        if (closing) return;
        closing = true;
        const ratios = ready ? JSON.parse(JSON.stringify(layout.ratios)) : null;
        void pendingSave.then(() => invoke('finish_exit', { ratios })).catch((error) => {
          closing = false;
          settingsError = String(error);
        });
      });
      if (disposed) { unlisten(); unlistenError(); unlistenExit(); return; }
      const labels = Object.fromEntries(['file','open','recentProjects','closeProject','view','theme','system','light','dark','walnut','git','update','commitAction','push','commit','log','quit','edit','undo','redo','cut','copy','paste','selectAll'].map((key) => [key, t(key as Parameters<typeof t>[0])]));
      const response = await invoke<{ settings: Settings; error: string | null }>('get_settings', { labels });
      if (disposed) return;
      layout = response.settings.layout;
      choice = response.settings.theme;
      settingsError = response.error ?? '';
      lastRatios = JSON.stringify(layout.ratios);
      ready = true;
      await refreshTheme();
      version = await invoke<string>('app_version');
      browseReady = true;
    }
    initialize().catch((error) => { settingsError = String(error); });
    return () => { disposed = true; generation++; unlisten?.(); unlistenError?.(); unlistenExit?.(); system.removeEventListener('change', systemChanged); };
  });
  $effect(() => {
    if (!browseReady) return;
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    async function initializeBrowser() {
      const refresh = await listen<[number, { status: boolean; refs: boolean; history: boolean }]>('repository-refresh', ({ payload: [generation, areas] }) => { void browser.refresh(areas, generation); });
      if (disposed) { refresh(); return; } unlisteners.push(refresh);
      const menu = await listen<string>('repository-menu', ({ payload }) => {
        if (payload === 'open-project') void chooseRepository();
        else if (payload === 'close-project') void browser.close().catch(error => browser.report(error));
        else if (payload.startsWith('recent:')) void browser.open(payload.slice(7));
        else if (payload === 'update-project') workflow.update();
        else if (payload === 'commit-project') document.querySelector<HTMLElement>('.commit-panel textarea')?.focus();
        else if (payload === 'push-project') workflow.push();
      });
      if (disposed) { menu(); return; } unlisteners.push(menu);
      await browser.recentProjects();
      if (disposed) return;
      const last = browser.recent.find(project => !project.failed);
      if (last) await browser.open(last.path);
    }
    void initializeBrowser().catch(error => browser.report(error));
    return () => { disposed = true; unlisteners.forEach(unlisten => unlisten()); browser.dispose(); };
  });
</script>

<main inert={closing}>
  <header><strong>Gitwood</strong><button class="project" onclick={chooseRepository}>{browser.project?.root.split('/').pop() ?? b('open')} ⌄</button>
    <GitWorkflow bind:this={workflow} {browser} oncommitpush={() => { browser.commit.result = null; }} />
    <span class="grow"></span><button class="project" aria-label={b('refresh')} title={b('refresh')} disabled={!browser.project} onclick={() => { void browser.refresh({ status: true, refs: true, history: true }); }}>⟳</button>{#if version}<span>{version}</span>{/if}
  </header>
  {#if settingsError}<p role="alert">{t('settingsError')}: {settingsError}</p>{/if}
  {#if themeErrors.length}<p role="alert">{t('themeError')}: {themeErrors.map(error => `${error.path}: ${error.message}`).join('; ')}</p>{/if}
  {#if browser.project}
    <div class="workspace">
      <nav class="rail" aria-label={t('view')}><button title={t('commit')} aria-pressed={layout.commitVisible} onclick={() => togglePanel('commit')}>⊖</button><span class="grow"></span><button title={t('log')} aria-pressed={layout.logVisible} onclick={() => togglePanel('log')}>⑂</button></nav>
      <div class="panes"><Splitter axis="y" label={t('workspaceLog')} minFirst={240} minSecond={240} bind:ratio={layout.ratios.workspaceLog} secondVisible={layout.logVisible}>
        {#snippet first()}
          <Splitter label={t('commitDiff')} minFirst={220} minSecond={420} bind:ratio={layout.ratios.commitDiff} firstVisible={layout.commitVisible}>
            {#snippet first()}<CommitPanel {browser} refresh={() => { void browser.refresh({ status: true, refs: false, history: false }); }} commitPush={() => workflow?.commitAndPush()} />{/snippet}
            {#snippet second()}<Preview {browser} />{/snippet}
          </Splitter>
        {/snippet}
        {#snippet second()}
          <section class="log"><h2>Git <span>{b('log')}</span><button class="hide-log" title={t('log')} aria-label={b('close')} onclick={() => togglePanel('log')}>−</button></h2><div class="log-body">
            <Splitter label={t('branchesCommits')} minFirst={160} minSecond={490} bind:ratio={layout.ratios.branchesCommits}>
              {#snippet first()}<Branches refs={browser.refs} branch={browser.filter.branch} select={branch => { void browser.applyFilter({ ...browser.filter, branch }); }} />{/snippet}
              {#snippet second()}
                <Splitter label={t('commitsInformation')} minFirst={260} minSecond={220} bind:ratio={layout.ratios.commitsInformation}>
                  {#snippet first()}<HistoryList {browser} />{/snippet}
                  {#snippet second()}<Information {browser} label={t('filesDetails')} bind:ratio={layout.ratios.filesDetails} />{/snippet}
                </Splitter>
              {/snippet}
            </Splitter>
          </div></section>
        {/snippet}
      </Splitter></div>
    </div>
    <footer><span>{browser.project.root}</span>{#if browser.status}<span>{browser.status.changes} {b('changes')} · {browser.status.untracked} {b('unversioned')}</span>{/if}{#if browser.commit.busy}<span role="status">{browser.commit.task}</span><progress aria-label={c('running')}></progress><button onclick={()=>void invoke('cancel_write',{generation:browser.generation}).catch(error=>browser.report(error))}>{c('cancel')}</button>{/if}{#if browser.loading}<span>{b('loading')}</span>{/if}</footer>
  {:else}
    <section class="welcome"><h1>{b('welcome')}</h1><p>{browser.opening ? b('opening') : b('chooseRepository')}</p>
      <button onclick={chooseRepository} disabled={browser.opening}>{b('open')}</button>
      {#if browser.recent.length}<h2>{b('recentProjects')}</h2><div class="recent"><List label={b('recentProjects')} rowCount={browser.recent.length}
        getRow={index => ({ id: browser.recent[index].path, label: `${browser.recent[index].path}${browser.recent[index].failed ? ` (${b('failed')})` : ''}` })}
        activateOnClick onactivate={row => { void browser.open(row.id); }} /></div>{/if}
    </section>
  {/if}
  {#if browser.commit.result?.kind === 'recovery'}<RecoveryDialog result={browser.commit.result} close={()=>{browser.commit.result=null;}} />{/if}
{#if browser.error}<ErrorDialog error={browser.error} projectOpen={!!browser.project} close={() => { browser.error = null; }} />{/if}
</main>
<style>
  main { display: flex; flex-direction: column; height: 100%; }
  header, footer { display: flex; align-items: center; gap: var(--space-inline); flex: none; padding: var(--space-header-y) var(--space-panel-x); background: var(--background-header); color: var(--text-header); border-bottom: var(--border-width) solid var(--separator); }
  strong { font-weight: var(--font-weight-heading); }
  header span { color: var(--text-header-secondary); font-size: var(--font-small-size); }
  .project { background: none; color: inherit; border: 0; }
  .grow { flex: 1; }
  main > p { margin: 0; padding: var(--space-panel-y) var(--space-panel-x); }
  .workspace { display: flex; flex: 1; min-width: 0; min-height: 0; }
  .panes { flex: 1; min-width: 0; min-height: 0; }
  .rail { display: flex; flex-direction: column; flex: none; padding: var(--space-panel-y) calc(var(--space-inline) / 2); border-inline-end: var(--border-width) solid var(--separator); }
  .rail button, .hide-log { padding: 0; width: var(--toolbar-button-size); height: var(--toolbar-button-size); background: none; color: var(--text-secondary); border: 0; }
  .rail button[aria-pressed='true'] { background: var(--toolbar-active-bg); color: var(--accent); }
  .hide-log { float: right; }
  .log { height: 100%; display: flex; flex-direction: column; }
  h2 { flex: none; margin: 0; font-size: var(--font-ui-size); padding: var(--space-panel-y) var(--space-panel-x); border-bottom: var(--border-width) solid var(--separator); }
  .log h2 span { margin-inline-start: var(--space-panel-x); border-bottom: var(--focus-width) solid var(--tab-active-border); }
  .log-body { flex: 1; min-height: 0; }
  footer { font-size: var(--font-small-size); background: var(--background-secondary); color: var(--text-secondary); padding-block: 0; border-top: var(--border-width) solid var(--separator); }
  footer span:first-child { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .welcome { flex: 1; min-height: 0; width: min(80%, calc(var(--font-ui-size) * 50)); margin: auto; padding: var(--space-panel-x); display: flex; flex-direction: column; align-items: start; justify-content: center; }
  .welcome h1 { font-size: calc(var(--font-ui-size) * 1.5); }
  .welcome h2 { margin-block-start: var(--space-panel-x); width: 100%; }
  .recent { height: min(40vh, calc(var(--list-row-height) * 10)); width: 100%; }
  button { font: inherit; padding: var(--space-panel-y) var(--space-panel-x); background: var(--button-bg); color: var(--button-fg); border: var(--border-width) solid var(--button-border); }
</style>
