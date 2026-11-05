<script lang="ts">
  import { requestResolution } from './features/conflicts/model';
  import { conflictText } from './i18n/conflict';
  import { onMount, tick, untrack } from 'svelte';
  import { invoke, isTauri } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { listenWindowEvent } from './windowEvents';
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
  import CredentialDialog from './features/git/CredentialDialog.svelte';
  import Notifications from './features/notifications/Notifications.svelte';
  import { notifications } from './features/notifications/center.svelte';
  import { operationLabel } from './features/git/operation';
  import { operationText } from './i18n/operation';
  import { withoutCredential, type CredentialKey, type CredentialRequest } from './features/git/credential';
  import ProjectWidget from './features/projects/ProjectWidget.svelte';
  import OpenProjectDialog from './features/projects/OpenProjectDialog.svelte';
  import { ProjectOpening } from './features/projects/opening.svelte';
  import { RepositoryBrowser } from './features/browse/browser.svelte';
  import type { NativeMenuActions } from './features/browse/types';
  import { browseText as b } from './i18n/browse';
  const browser = new RepositoryBrowser();
  const projects = new ProjectOpening(browser);
  // Notices and their actions belong to the repository they were raised for.
  $effect(() => { void browser.project; untrack(() => notifications.clear()); });
  let workflow = $state<GitWorkflow>();
  let commitPanel = $state<CommitPanel>();
  let browseReady = $state(false);
  // Waiting prompts of this window, oldest first. Rust closes each one when it is answered
  // or its operation ends, so a dialog never outlives the prompt it belongs to.
  let credentials = $state<CredentialRequest[]>([]);
  async function answerCredential(answer: string | null) {
    const request = credentials[0];
    if (!request) return;
    credentials = withoutCredential(credentials, request);
    const { generation, operation, requestId } = request;
    try { await invoke('answer_credential', { generation, operation, requestId, answer }); }
    catch {
      // The operation already ended (cancelled or timed out); its own error path reports it.
    }
  }
  async function chooseRepository() {
    if (!isTauri() || browser.opening || projects.busy || projects.pending) return;
    try {
      const path = await openDialog({ directory: true, multiple: false, title: b('open') });
      if (path) await projects.request(path);
    } catch (error) { browser.report(error); }
  }
  import Splitter from './components/Splitter.svelte';
  import { detectLocale, translate } from './i18n';
  import { defaultLayout, layoutRequest, type Layout } from './layout/state';
  import { applyTheme, effectiveTheme, type Theme, type ThemeChoice, type ThemeError } from './theme';
  import { appearance } from './theme/appearance.svelte';
  import HistoryPreferencesDialog from './features/git/HistoryPreferencesDialog.svelte';
  import ThemeSettingsDialog from './features/settings/ThemeSettingsDialog.svelte';
  import { historyMenuText } from './i18n/history-menu';
  import { nativeMenuLabels } from './i18n/native-menu';

  type EmptyCherryPick = 'skip' | 'createEmpty';
  interface Settings { theme: ThemeChoice; layout: Layout; cherryPickAddSuffixToPublishedCommits: boolean; emptyCherryPick: EmptyCherryPick }
  const locale = detectLocale(navigator.language);
  const t = (key: Parameters<typeof translate>[1]) => translate(locale, key);
  function togglePanel(panel: 'commit' | 'log') {
    void invoke('set_tool_window', { panel, visible: panel === 'commit' ? !layout.commitVisible : !layout.logVisible }).catch(error => { settingsError = String(error); });
  }
  function showLog() {
    if (!layout.logVisible) void invoke('set_tool_window', { panel: 'log', visible: true }).catch(error => { settingsError = String(error); });
  }
  async function showCommit() {
    if (!layout.commitVisible) {
      await invoke('set_tool_window', { panel: 'commit', visible: true });
      layout.commitVisible = true;
      await tick();
    }
  }
  function dispatchMenuAction(id: string) {
    if (id === 'ChangesView.ToggleCommitUi') workflow?.runAction(id);
    else if (id === 'CheckinProject' || id.startsWith('ChangesView.') || id === 'CheckinFiles') commitPanel?.runAction(id);
    else workflow?.runAction(id);
  }
  let version = $state('');
  let layout = $state<Layout>(structuredClone(defaultLayout));
  let ready = $state(false);
  let settingsError = $state('');
  let closing = $state(false);
  let themeErrors = $state<ThemeError[]>([]);
  let themeUnavailable = $state(false);
  let themeChoice = $state<ThemeChoice>('system');
  let historyPreferencesOpen = $state(false);
  let themeDialogOpen = $state(false);
  let refreshThemeAfterDialog: () => void = () => {};
  let cherryPickAddSuffixToPublished = $state(true);
  let emptyCherryPick = $state<EmptyCherryPick>('skip');
  let lastLayout = '';
  let pendingSave = Promise.resolve();

  async function previewTheme(name: ThemeChoice) {
    const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const selected = effectiveTheme(name, systemDark);
    const theme = await invoke<Theme>('get_theme', { name: selected, systemDark });
    applyTheme(theme);
  }

  function closeThemeDialog() {
    themeDialogOpen = false;
    refreshThemeAfterDialog();
  }

  $effect(() => {
    const request = JSON.stringify(layoutRequest(layout));
    if (!ready || closing || !isTauri() || request === lastLayout) return;
    lastLayout = request;
    // Send each UI change immediately; Rust debounces disk writes and flushes on exit.
    pendingSave = pendingSave.then(() => invoke('save_layout', { layout: JSON.parse(request) }))
      .then(() => {}).catch((error) => { settingsError = String(error); });
  });

  let lastNativeMenuContext = '';
  let nativeMenuRequest = 0;
  $effect(() => {
    if (!browseReady || !browser.project || !browser.generation || !isTauri()) return;
    const file = browser.currentFile;
    const selection = {
      currentFile: file ? { source: file.source, versioned: file.versioned, untracked: file.untracked, conflicted: file.conflicted } : null,
      selectedChangeCount: browser.selectedChangeCount,
      selectedCommitCount: browser.selection.size || (browser.selected ? 1 : 0),
      busy: browser.workflowBusy || browser.commit.busy || browser.opening || projects.busy,
    };
    const signature = JSON.stringify({
      generation: browser.generation,
      status: browser.status && {
        branch: browser.status.branch, changes: browser.status.changes, untracked: browser.status.untracked,
        conflicts: browser.status.conflicts, operation: browser.status.operation, updateStash: browser.status.update_stash,
      },
      refs: browser.refs && {
        head: browser.refs.head,
        local: browser.refs.local.map(ref => [ref.name, ref.hash]),
        remotes: Object.entries(browser.refs.remotes).map(([name, refs]) => [name, refs.map(ref => [ref.name, ref.hash])]),
      },
      selectedFile: file && [file.path.bytes, file.revision, file.source],
      selected: browser.selected,
      selection: [...browser.selection].sort(),
      selectedChangeCount: browser.selectedChangeCount,
      busy: selection.busy,
    });
    if (signature === lastNativeMenuContext) return;
    lastNativeMenuContext = signature;
    const request = ++nativeMenuRequest;
    void invoke<NativeMenuActions>('update_native_menu_context', { generation: browser.generation, selection })
      .then(value => { if (request === nativeMenuRequest && signature === lastNativeMenuContext) browser.nativeMenuActions = value; })
      .catch(error => { if (request === nativeMenuRequest) browser.report(error); });
  });

  onMount(() => {
    document.documentElement.lang = locale;
    if (!isTauri()) return;
    const system = window.matchMedia('(prefers-color-scheme: dark)');
    let active = '';
    let generation = 0;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    let unlistenExit: (() => void) | undefined;
    let unlistenThemeCatalog: (() => void) | undefined;
    let unlistenOpenThemes: (() => void) | undefined;
    async function refreshTheme(force = false, apply = true) {
      const name = effectiveTheme(themeChoice, system.matches);
      if (name === active && !force) return;
      active = name;
      const request = ++generation;
      try {
        const theme = await invoke<Theme>('get_theme', { name, systemDark: system.matches });
        if (!disposed && request === generation) {
          themeUnavailable = theme.errors.some(error => error.code === 'theme_unavailable');
          themeErrors = theme.errors.filter(error => error.code !== 'theme_unavailable');
          if (apply) {
            applyTheme(theme);
            appearance.dark = theme.dark;
          }
        }
      } catch (error) {
        if (!disposed && request === generation) { active = ''; settingsError = String(error); }
      }
    }
    refreshThemeAfterDialog = () => { void refreshTheme(true, true); };
    const systemChanged = () => { if ((themeChoice === 'system' || themeUnavailable) && !themeDialogOpen) void refreshTheme(true); };
    system.addEventListener('change', systemChanged);
    async function initialize() {
      unlisten = await listenWindowEvent<Settings>('preferences-changed', ({ payload }) => {
        // Do not replace current ratios with a backend snapshot while dragging, and keep the
        // Commit view toggles: they are written by this window and need no echo.
        layout.commitVisible = payload.layout.commitVisible;
        layout.logVisible = payload.layout.logVisible;
        themeChoice = payload.theme;
        cherryPickAddSuffixToPublished = payload.cherryPickAddSuffixToPublishedCommits;
        emptyCherryPick = payload.emptyCherryPick;
        void refreshTheme(true);
      });
      if (disposed) { unlisten(); return; }
      unlistenThemeCatalog = await listenWindowEvent('theme-catalog-changed', () => {
        void refreshTheme(true, !themeDialogOpen);
      });
      if (disposed) { unlisten(); unlistenThemeCatalog(); return; }
      unlistenOpenThemes = await listenWindowEvent('open-theme-settings', () => { themeDialogOpen = true; });
      if (disposed) { unlisten(); unlistenThemeCatalog(); unlistenOpenThemes(); return; }
      unlistenError = await listen<string>('settings-error', ({ payload }) => { settingsError = payload; });
      if (disposed) { unlisten(); unlistenError(); return; }
      unlistenExit = await listenWindowEvent('save-before-exit', () => {
        if (closing) return;
        closing = true;
        const snapshot = ready ? JSON.parse(JSON.stringify(layoutRequest(layout))) : null;
        void pendingSave.then(() => invoke('finish_exit', { layout: snapshot })).catch((error) => {
          closing = false;
          settingsError = String(error);
        });
      });
      if (disposed) { unlisten(); unlistenError(); unlistenExit(); return; }
      const labels = { ...Object.fromEntries(['file','open','recentProjects','closeProject','view','theme','system','light','dark','walnut','themeSettings','git','update','commitAction','push','commit','log','quit','edit','undo','redo','cut','copy','paste','selectAll'].map((key) => [key, t(key as Parameters<typeof t>[0])])), ...nativeMenuLabels(locale) };
      const response = await invoke<{ settings: Settings; error: string | null }>('get_settings', { labels });
      if (disposed) return;
      layout = response.settings.layout;
      themeChoice = response.settings.theme;
      cherryPickAddSuffixToPublished = response.settings.cherryPickAddSuffixToPublishedCommits;
      emptyCherryPick = response.settings.emptyCherryPick;
      settingsError = response.error ?? '';
      lastLayout = JSON.stringify(layoutRequest(layout));
      ready = true;
      await refreshTheme(true);
      version = await invoke<string>('app_version');
      browseReady = true;
    }
    initialize().catch((error) => { settingsError = String(error); });
    return () => { disposed = true; generation++; refreshThemeAfterDialog = () => {}; unlisten?.(); unlistenError?.(); unlistenExit?.(); unlistenThemeCatalog?.(); unlistenOpenThemes?.(); system.removeEventListener('change', systemChanged); };
  });
  $effect(() => {
    if (!browseReady) return;
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    async function initializeBrowser() {
      const refresh = await listenWindowEvent<[number, { status: boolean; refs: boolean; history: boolean }]>('repository-refresh', ({ payload: [generation, areas] }) => { void browser.refresh(areas, generation); });
      if (disposed) { refresh(); return; } unlisteners.push(refresh);
      const menu = await listenWindowEvent<string>('repository-menu', ({ payload }) => {
        if (payload === 'open-project') void chooseRepository();
        else if (payload === 'close-project' && !projects.pending && !projects.busy) void browser.close().catch(error => browser.report(error));
        else if (payload.startsWith('recent:')) void projects.request(payload.slice(7));
        else dispatchMenuAction(payload);
      });
      if (disposed) { menu(); return; } unlisteners.push(menu);
      const credential = await listenWindowEvent<CredentialRequest>('credential_request', ({ payload }) => {
        if (disposed || payload.generation !== browser.generation) return;
        credentials = [...credentials, payload];
      });
      if (disposed) { credential(); return; } unlisteners.push(credential);
      const closed = await listenWindowEvent<CredentialKey>('credential_closed', ({ payload }) => {
        credentials = withoutCredential(credentials, payload);
      });
      if (disposed) { closed(); return; } unlisteners.push(closed);
      await browser.recentProjects();
      if (disposed) return;
      const initial = await invoke<string | null>('initial_repository');
      if (disposed) return;
      const path = initial ?? browser.recent.find(project => !project.failed)?.path;
      if (path) await browser.open(path);
    }
    void initializeBrowser().catch(error => browser.report(error));
    return () => { disposed = true; unlisteners.forEach(unlisten => unlisten()); browser.dispose(); };
  });
</script>

<main inert={closing}>
  <header><ProjectWidget {browser} busy={projects.busy} open={() => { void chooseRepository(); }} select={path => { void projects.request(path); }} />
    <GitWorkflow bind:this={workflow} {browser} oncommitpush={() => { browser.commit.result = null; }} onmenuaction={dispatchMenuAction} onshowlog={showLog} ontogglecommit={() => togglePanel('commit')} />
    <span class="grow"></span><button class="project" aria-label={b('refresh')} title={b('refresh')} disabled={!browser.project} onclick={() => { void browser.refresh({ status: true, refs: true, history: true }); }}>⟳</button><button class="project" aria-label={historyMenuText('preferences')} title={historyMenuText('preferences')} onclick={() => historyPreferencesOpen = true}>⚙</button><button class="project" aria-label={t('themeSettings')} title={t('themeSettings')} onclick={() => themeDialogOpen = true}>◉</button>{#if version}<span>{version}</span>{/if}
  </header>
  {#if settingsError}<p role="alert">{t('settingsError')}: {settingsError}</p>{/if}
  {#if themeUnavailable}<p role="alert">{t('themeUnavailable')}</p>{/if}
  {#if themeErrors.length}<p role="alert">{t('themeError')}: {themeErrors.map(error => `${error.path}: ${error.message}`).join('; ')}</p>{/if}
  {#if browser.project}
    <div class="workspace">
      <nav class="rail" aria-label={t('view')}><button title={t('commit')} aria-pressed={layout.commitVisible} onclick={() => togglePanel('commit')}>⊖</button><span class="grow"></span><button title={t('log')} aria-pressed={layout.logVisible} onclick={() => togglePanel('log')}>⑂</button></nav>
      <div class="panes"><Splitter axis="y" label={t('workspaceLog')} minFirst={240} minSecond={240} bind:ratio={layout.ratios.workspaceLog} secondVisible={layout.logVisible}>
        {#snippet first()}
          <Splitter label={t('commitDiff')} minFirst={220} minSecond={420} bind:ratio={layout.ratios.commitDiff} firstVisible={layout.commitVisible}>
            {#snippet first()}<CommitPanel bind:this={commitPanel} {browser} onfocuscommit={showCommit} oncurrentfile={(action,path,revision) => workflow?.showCurrentFileAction(action,path,revision)} bind:flat={layout.commitFlat} bind:preview={layout.commitPreview} refresh={() => { void browser.refresh({ status: true, refs: false, history: false }); }} commitPush={() => workflow?.commitAndPush()} />{/snippet}
            {#snippet second()}<Preview {browser} bind:ratio={layout.ratios.diffVersions} />{/snippet}
          </Splitter>
        {/snippet}
        {#snippet second()}
          <section class="log"><h2>Git <span>{b('log')}</span><button class="hide-log" title={t('log')} aria-label={b('close')} onclick={() => togglePanel('log')}>−</button></h2><div class="log-body">
            <Splitter label={t('branchesCommits')} minFirst={160} minSecond={490} bind:ratio={layout.ratios.branchesCommits}>
              {#snippet first()}<Branches refs={browser.refs} branch={browser.filter.branch} select={branch => { void browser.applyFilter({ ...browser.filter, branch }); }} />{/snippet}
              {#snippet second()}
                <Splitter label={t('commitsInformation')} minFirst={260} minSecond={220} bind:ratio={layout.ratios.commitsInformation}>
                  {#snippet first()}<HistoryList {browser} handlers={workflow?.branchHandlers()} />{/snippet}
                  {#snippet second()}<Information {browser} label={t('filesDetails')} oncurrentfile={(action,path,revision) => workflow?.showCurrentFileAction(action,path,revision)} bind:ratio={layout.ratios.filesDetails} />{/snippet}
                </Splitter>
              {/snippet}
            </Splitter>
          </div></section>
        {/snippet}
      </Splitter></div>
    </div>
    <footer><span>{browser.project.root}</span>{#if browser.status?.conflicts || browser.status?.update_stash}<button onclick={requestResolution}>{conflictText('resolve')}</button>{/if}{#if browser.status?.operation}<span class="operation" role="status">{operationLabel(browser.status.operation, browser.status.branch.head)}{#if browser.status.conflicts} · {operationText('conflicts', { count: browser.status.conflicts })}{/if}</span>{/if}{#if browser.status}<span>{browser.status.changes} {b('changes')} · {browser.status.untracked} {b('unversioned')}</span>{/if}{#if browser.commit.busy}<span role="status">{browser.commit.task}</span><progress aria-label={c('running')}></progress><button onclick={()=>void invoke('cancel_write',{generation:browser.generation}).catch(error=>browser.report(error))}>{c('cancel')}</button>{/if}{#if browser.loading}<span>{b('loading')}</span>{/if}</footer>
  {:else}
    <section class="welcome"><h1>{b('welcome')}</h1><p>{browser.opening ? b('opening') : b('chooseRepository')}</p>
      <button onclick={chooseRepository} disabled={browser.opening}>{b('open')}</button>
      {#if browser.recent.length}<h2>{b('recentProjects')}</h2><div class="recent"><List label={b('recentProjects')} rowCount={browser.recent.length}
        getRow={index => ({ id: browser.recent[index].path, label: `${browser.recent[index].path}${browser.recent[index].failed ? ` (${b('failed')})` : ''}` })}
        activateOnClick onactivate={row => { void projects.request(row.id); }} /></div>{/if}
    </section>
  {/if}
  {#if browser.commit.result?.kind === 'recovery'}<RecoveryDialog result={browser.commit.result} close={()=>{browser.commit.result=null;}} />{/if}
{#if browser.error}<ErrorDialog error={browser.error} projectOpen={!!browser.project} close={() => { browser.error = null; }} />{/if}
<Notifications center={notifications} />
<CredentialDialog request={credentials[0] ?? null} answer={value => { void answerCredential(value); }} />
{#if projects.pending}<OpenProjectDialog path={projects.pending} choose={target => { void projects.choose(target); }} cancel={() => projects.cancel()} />{/if}
</main>
{#if historyPreferencesOpen}<HistoryPreferencesDialog suffix={cherryPickAddSuffixToPublished} empty={emptyCherryPick} onclose={() => { historyPreferencesOpen = false; }} />{/if}
{#if themeDialogOpen}<ThemeSettingsDialog {locale} currentTheme={themeChoice} {previewTheme} onclose={closeThemeDialog} />{/if}
<style>
  main { display: flex; flex-direction: column; height: 100%; }
  header, footer { display: flex; align-items: center; gap: var(--space-inline); flex: none; padding: var(--space-header-y) var(--space-panel-x); background: var(--background-header); color: var(--text-header); border-bottom: var(--border-width) solid var(--separator); }
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
  footer .operation { color: var(--status-modified); }
  footer span:first-child { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .welcome { flex: 1; min-height: 0; width: min(80%, calc(var(--font-ui-size) * 50)); margin: auto; padding: var(--space-panel-x); display: flex; flex-direction: column; align-items: start; justify-content: center; }
  .welcome h1 { font-size: calc(var(--font-ui-size) * 1.5); }
  .welcome h2 { margin-block-start: var(--space-panel-x); width: 100%; }
  .recent { height: min(40vh, calc(var(--list-row-height) * 10)); width: 100%; }
  button { font: inherit; padding: var(--space-panel-y) var(--space-panel-x); background: var(--button-bg); color: var(--button-fg); border: var(--border-width) solid var(--button-border); }
</style>
