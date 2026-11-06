<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listenWindowEvent } from '../../windowEvents';
  import { translate, type Locale } from '../../i18n';
  import { type ThemeCatalog, type ThemeChoice, type ThemeDescriptor } from '../../theme';
  import { contrastWarnings, displayedChoice, errorRows, themeAvailable, ThemeSettingsSession } from '../../theme/theme-settings';

  let { locale, currentTheme, previewTheme, onclose, editorTabLimit }: {
    locale: Locale;
    currentTheme: ThemeChoice;
    editorTabLimit: number;
    previewTheme: (choice: ThemeChoice) => Promise<void>;
    onclose: () => void;
  } = $props();
  let element: HTMLDialogElement;
  let catalog = $state<ThemeCatalog>();
  let selected = $state<ThemeChoice>('system');
  let session = $state<ThemeSettingsSession>();
  let error = $state('');
  let busy = $state(false);
  let copied = $state(false);
  let copyFailed = $state(false);
  let systemDark = $state(false);
  let hasPreview = $state(false);
  let explicitSelection = $state(false);
  let githubHosts = $state('');
  let gitlabHosts = $state('');
  let hostsSaved = $state(false);
  // Initial value only; the saved setting comes back through preferences-changed.
  let tabLimit = $state(untrack(() => editorTabLimit));
  let tabLimitSaved = $state(false);
  let warningList = $derived(catalog ? contrastWarnings(selected, catalog.themes, systemDark) : []);
  let errorList = $derived(catalog ? errorRows(catalog.errors) : []);
  let savedChoiceUnavailable = $derived(catalog ? currentTheme.startsWith('user:') && !themeAvailable(currentTheme, catalog.themes) : false);
  const t = (key: Parameters<typeof translate>[1]) => translate(locale, key);

  onMount(() => {
    element.showModal();
    session = new ThemeSettingsSession(currentTheme, previewTheme, async choice => {
      await invoke('set_theme', { name: choice });
    });
    selected = currentTheme;
    const system = window.matchMedia('(prefers-color-scheme: dark)');
    systemDark = system.matches;
    const systemChanged = () => {
      systemDark = system.matches;
      if (selected === 'system') void previewTheme('system');
    };
    system.addEventListener('change', systemChanged);
    let disposed = false;
    let unlistenCatalog: (() => void) | undefined;
    let unlistenPreferences: (() => void) | undefined;
    void listenWindowEvent('theme-catalog-changed', () => { void loadCatalog(true); }).then(stop => {
      if (disposed) stop(); else unlistenCatalog = stop;
    });
    void listenWindowEvent<{ theme: ThemeChoice }>('preferences-changed', ({ payload }) => {
      if (payload.theme === session?.original) return;
      selected = payload.theme;
      hasPreview = false;
      explicitSelection = false;
      session = new ThemeSettingsSession(payload.theme, previewTheme, async choice => {
        await invoke('set_theme', { name: choice });
      });
    }).then(stop => { if (disposed) stop(); else unlistenPreferences = stop; });
    void loadCatalog();
    void invoke<{ github: string[]; gitlab: string[] }>('get_hosting_hosts').then(value => {
      githubHosts = value.github.join('\n'); gitlabHosts = value.gitlab.join('\n');
    }).catch(cause => { error = String(cause); });
    return () => { disposed = true; unlistenCatalog?.(); unlistenPreferences?.(); system.removeEventListener('change', systemChanged); };
  });

  async function loadCatalog(reload = false) {
    try {
      catalog = await invoke<ThemeCatalog>('theme_catalog');
      error = '';
      const savedUnavailable = !themeAvailable(currentTheme, catalog.themes);
      if (!explicitSelection) {
        selected = displayedChoice(currentTheme, catalog.themes);
      } else if (!themeAvailable(selected, catalog.themes)) {
        selected = 'system';
        hasPreview = false;
      }
      if (session) session.selected = selected;
      if (reload && (hasPreview || explicitSelection || savedUnavailable || selected === currentTheme)) {
        await previewTheme(selected);
      }
    } catch (cause) {
      error = String(cause);
    }
  }

  async function choose(choice: ThemeChoice) {
    if (!session) return;
    selected = choice;
    hasPreview = true;
    explicitSelection = true;
    error = '';
    try { await session.choose(choice); }
    catch (cause) { error = String(cause); }
  }

  async function cancel() {
    error = '';
    try { await session?.cancel(); hasPreview = false; onclose(); }
    catch (cause) { error = String(cause); }
  }

  async function apply() {
    if (!session) return;
    busy = true;
    error = '';
    try {
      if (explicitSelection) await session.confirm();
      onclose();
    }
    catch (cause) { error = String(cause); }
    finally { busy = false; }
  }

  async function copyDirectory() {
    if (!catalog) return;
    copied = false;
    copyFailed = false;
    try { await navigator.clipboard.writeText(catalog.directory); copied = true; }
    catch { copyFailed = true; }
  }

  async function openDirectory() {
    try { await invoke('open_theme_directory'); }
    catch { error = t('themeDirectoryOpenFailed'); }
  }

  async function saveHosts() {
    hostsSaved = false;
    const values = (source: string) => source.split(/[\r\n]+/).map(host => host.trim()).filter(Boolean);
    try {
      await invoke('update_hosting_hosts', { hosts: { github: values(githubHosts), gitlab: values(gitlabHosts) } });
      hostsSaved = true;
    } catch (cause) { error = String(cause); }
  }

  async function saveTabLimit() {
    tabLimitSaved = false;
    try { await invoke('save_editor_tab_limit', { limit: Math.trunc(tabLimit) }); tabLimitSaved = true; }
    catch (cause) { error = String(cause); }
  }

  function label(theme: ThemeDescriptor): string {
    if (!theme.builtIn) return theme.name;
    if (theme.id === 'light' || theme.id === 'dark' || theme.id === 'walnut') return t(theme.id);
    return theme.name;
  }
</script>

<dialog bind:this={element} aria-labelledby="theme-settings-title" oncancel={event => { event.preventDefault(); void cancel(); }}>
  <h2 id="theme-settings-title">{t('themeSettingsTitle')}</h2>
  <p class="hint">{t('themePreviewHint')}</p>
  <fieldset>
    <legend>{t('theme')}</legend>
    <div class="options" role="radiogroup" aria-label={t('theme')}>
      <label><input type="radio" name="theme" value="system" checked={selected === 'system'} onchange={() => void choose('system')} />{t('system')}</label>
      {#each catalog?.themes ?? [] as theme (theme.id)}
        <label><input type="radio" name="theme" value={theme.id} checked={selected === theme.id} onchange={() => void choose(theme.id)} />
          <span>{label(theme)}</span><small>{theme.builtIn ? t('themeBuiltin') : t('themeUser')}</small>
        </label>
      {/each}
    </div>
  </fieldset>
  {#if savedChoiceUnavailable}<p role="alert">{t('themeUnavailable')}: {currentTheme}</p>{/if}
  {#if catalog}
    <section class="directory">
      <h3>{t('themeDirectory')}</h3>
      <code>{catalog.directory}</code>
      <button type="button" onclick={() => void copyDirectory()}>{t('copyPath')}</button>
      <button type="button" onclick={() => void openDirectory()}>{t('openThemeDirectory')}</button>
      {#if copied}<span role="status">{t('pathCopied')}</span>{/if}
      {#if copyFailed}<span role="alert">{t('themeCopyFailed')}</span>{/if}
    </section>
    <section class="hosting-hosts">
      <h3>{t('hostingSettings')}</h3><p class="hint">{t('hostingHostsHelp')}</p>
      <label>{t('githubHosts')}<textarea bind:value={githubHosts} rows="3"></textarea></label>
      <label>{t('gitlabHosts')}<textarea bind:value={gitlabHosts} rows="3"></textarea></label>
      <button type="button" onclick={() => void saveHosts()}>{t('saveHosts')}</button>
      {#if hostsSaved}<span role="status">{t('hostsSaved')}</span>{/if}
    </section>
    <section class="editor-tabs">
      <h3>{t('editorTabs')}</h3>
      <label>{t('editorTabLimit')}<input type="number" min="1" max="50" step="1" bind:value={tabLimit} /></label>
      <button type="button" disabled={!Number.isInteger(tabLimit) || tabLimit < 1 || tabLimit > 50} onclick={() => void saveTabLimit()}>{t('saveTabLimit')}</button>
      {#if tabLimitSaved}<span role="status">{t('tabLimitSaved')}</span>{/if}
    </section>
    <section>
      <h3>{t('contrastWarnings')}</h3>
      <p class="hint">{t('contrastHelp')}</p>
      {#if warningList.length}
        <ul class="warnings" role="status">
          {#each warningList as warning (`${warning.foreground}-${warning.background}`)}
            <li><code>{warning.foreground} / {warning.background}</code><span>{warning.ratio}:1 &lt; {warning.minimum}:1</span></li>
          {/each}
        </ul>
      {:else}<p>{t('noContrastWarnings')}</p>{/if}
    </section>
    <section>
      <h3>{t('themeErrors')}</h3>
      {#if errorList.length}
        <ul class="errors" role="alert">
          {#each errorList as row (`${row.fileName}-${row.error.code}-${row.error.path}`)}
            <li><strong>{row.fileName}</strong><code>{row.error.code} · {row.error.path}</code><span>{row.error.message}</span></li>
          {/each}
        </ul>
      {:else}<p>{t('noThemeErrors')}</p>{/if}
    </section>
  {:else}<p role="status">{t('themeLoading')}</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
  <footer><button type="button" onclick={() => void cancel()}>{t('themeCancel')}</button><button type="button" class="default" disabled={busy || !catalog} onclick={() => void apply()}>{t('themeApply')}</button></footer>
</dialog>

<style>
  dialog { width: min(620px, 88vw); max-height: 86vh; overflow: auto; color: var(--text-primary); background: var(--popup-bg); border: var(--border-width) solid var(--popup-border); padding: var(--space-panel-x); }
  fieldset { border: var(--border-width) solid var(--separator); padding: var(--space-panel-x); }
  .options { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-panel-y) var(--space-inline); }
  .hosting-hosts label { display: grid; gap: var(--space-panel-y); margin-block: var(--space-panel-y); }
  .hosting-hosts textarea { color: var(--text-primary); background: var(--input-bg); border: var(--border-width) solid var(--input-border); padding: var(--space-panel-y); font: inherit; resize: vertical; }
  label { display: flex; align-items: center; gap: var(--space-panel-y); min-width: 0; padding: var(--space-panel-y); border-radius: 4px; }
  label:has(input:checked) { background: var(--selection-bg); color: var(--selection-fg); }
  label small { margin-left: auto; color: var(--text-secondary); }
  .hint { color: var(--text-secondary); }
  section { margin-top: var(--space-panel-x); }
  h3 { margin: 0 0 var(--space-panel-y); font-size: var(--font-small-size); }
  .directory { display: grid; grid-template-columns: 1fr auto; gap: var(--space-panel-y) var(--space-inline); align-items: center; }
  .directory h3 { grid-column: 1 / -1; }
  code { overflow-wrap: anywhere; font-family: var(--font-editor-family); }
  .warnings, .errors { margin: 0; padding-left: var(--space-panel-x); }
  .warnings li, .errors li { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--space-panel-y) var(--space-inline); margin-block: var(--space-panel-y); }
  .errors li { grid-template-columns: auto minmax(0, auto) minmax(0, 1fr); }
  footer { display: flex; justify-content: end; gap: var(--space-inline); margin-top: var(--space-panel-x); }
  button { color: var(--button-fg); background: var(--button-bg); font: inherit; padding: var(--space-panel-y) var(--space-panel-x); border: var(--border-width) solid var(--button-border); }
  button:disabled { color: var(--button-disabled-fg); background: var(--button-disabled-bg); }
  @media (max-width: 520px) { .options { grid-template-columns: 1fr; } .errors li { grid-template-columns: 1fr; } }
</style>
