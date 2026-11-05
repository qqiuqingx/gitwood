import { describe, expect, it } from 'vitest';
import { translate } from '../src/i18n';
import { contrastWarnings, displayedChoice, errorRows, themeAvailable, ThemeSettingsSession } from '../src/theme/theme-settings';
import type { ThemeCatalog } from '../src/theme';

const catalog: ThemeCatalog = {
  directory: '/themes',
  themes: [
    { id: 'light', name: 'Light', dark: false, builtIn: true, contrastIssues: [] },
    { id: 'dark', name: 'Dark', dark: true, builtIn: true, contrastIssues: [{ foreground: 'text-primary', background: 'background-primary', ratio: 3.2, minimum: 4.5 }] },
    { id: 'user:low', name: 'Low Contrast', dark: false, builtIn: false, contrastIssues: [{ foreground: 'button-fg', background: 'button-bg', ratio: 2.1, minimum: 4.5 }] },
  ],
  errors: [{ fileName: 'broken.json', errors: [{ code: 'invalid_json', path: '$', message: 'unexpected end' }] }],
};

describe('theme settings preview model', () => {
  it('previews immediately, restores the original choice on cancel, and saves only on confirm', async () => {
    const previews: string[] = [];
    const saves: string[] = [];
    const session = new ThemeSettingsSession('light', async choice => { previews.push(choice); }, async choice => { saves.push(choice); });
    await session.choose('user:low');
    expect(previews).toEqual(['user:low']);
    await session.cancel();
    expect(previews).toEqual(['user:low', 'light']);
    expect(saves).toEqual([]);

    await session.choose('dark');
    await session.confirm();
    expect(saves).toEqual(['dark']);
    expect(translate('en', 'themeSettingsTitle')).toBe('Theme Settings');
    expect(translate('zh-CN', 'themeApply')).toBe('应用主题');
  });

  it('lists file errors and selected-theme contrast warnings', () => {
    const errors = errorRows(catalog.errors);
    expect(errors).toEqual([{ fileName: 'broken.json', error: { code: 'invalid_json', path: '$', message: 'unexpected end' } }]);
    expect(translate('en', 'themeErrors')).toBe('Theme file errors');
    expect(contrastWarnings('user:low', catalog.themes, false)).toEqual(catalog.themes[2].contrastIssues);
    expect(contrastWarnings('system', catalog.themes, true)).toEqual(catalog.themes[1].contrastIssues);
    expect(translate('zh-CN', 'contrastWarnings')).toBe('对比度提示');
  });

  it('shows system while a saved user theme is unavailable without overwriting its saved ID', async () => {
    const previews: string[] = [];
    const saves: string[] = [];
    const session = new ThemeSettingsSession('user:missing', async choice => { previews.push(choice); }, async choice => { saves.push(choice); });
    expect(themeAvailable(session.original, catalog.themes)).toBe(false);
    expect(displayedChoice(session.original, catalog.themes)).toBe('system');
    expect(await session.confirm()).toBe(false);
    expect(session.original).toBe('user:missing');
    expect(saves).toEqual([]);
    await session.cancel();
    expect(previews).toEqual(['user:missing']);

    const recovered = [...catalog.themes, { id: 'user:missing' as const, name: 'Recovered', dark: false, builtIn: false, contrastIssues: [] }];
    expect(themeAvailable(session.original, recovered)).toBe(true);
    expect(displayedChoice(session.original, recovered)).toBe('user:missing');
  });
});
