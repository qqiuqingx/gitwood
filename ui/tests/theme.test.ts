import { afterEach, describe, expect, it, vi } from 'vitest';
import { applyTheme, effectiveTheme } from '../src/theme';

afterEach(() => vi.unstubAllGlobals());
describe('theme CSSOM injection', () => {
  it('updates documentElement variables and removes obsolete lane entries', () => {
    // The Node test environment has no DOM; only the browser CSSOM is substituted.
    const properties = new Map<string, string>();
    const documentElement = { style: {
      setProperty: (key: string, value: string) => properties.set(key, value),
      removeProperty: (key: string) => properties.delete(key),
    } };
    vi.stubGlobal('document', { documentElement });
    applyTheme({ name: 'First', dark: false, variables: { '--background-primary': '#fff', '--graph-lanes-0': '#abc', '--graph-lanes-1': '#def' }, errors: [] });
    expect(properties.get('--background-primary')).toBe('#fff');
    expect(properties.get('color-scheme')).toBe('light');
    applyTheme({ name: 'Second', dark: true, variables: { '--background-primary': '#32302f', '--graph-lanes-0': '#123' }, errors: [] });
    expect(document.documentElement).toBe(documentElement);
    expect(properties.get('--background-primary')).toBe('#32302f');
    expect(properties.get('color-scheme')).toBe('dark');
    expect(properties.has('--graph-lanes-1')).toBe(false);
  });
  it('follows system colors only for the system choice', () => {
    expect(effectiveTheme('system', true)).toBe('dark');
    expect(effectiveTheme('system', false)).toBe('light');
    expect(effectiveTheme('walnut', false)).toBe('walnut');
    expect(effectiveTheme('light', true)).toBe('light');
  });
});

// Vite expands these globs on every platform; no absolute machine paths.
const components = import.meta.glob(['../src/components/*.svelte', '../src/features/diff/*.svelte'], { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const themes = import.meta.glob('../../crates/gitwood-core/src/theme/builtin/*.json', { eager: true, import: 'default' }) as Record<string, { colors: Record<string, unknown> }>;
it('defines every component CSS variable in every bundled theme', () => {
  for (const theme of Object.values(themes)) {
    const keys = new Set(Object.keys(theme.colors).map(key => '--' + key.replaceAll('.', '-').replace(/[A-Z]/g, c => '-' + c.toLowerCase())));
    for (const [file, source] of Object.entries(components)) {
      for (const match of source.matchAll(/var\((--[\w-]+)/g)) expect(keys.has(match[1]), `${file}: ${match[1]}`).toBe(true);
    }
  }
});
