import { describe, expect, it, vi } from 'vitest';
import { currentFileItems } from '../src/features/git/current-file-menu';
import { currentFileText } from '../src/i18n/current-file';

describe('Current File actions', () => {
  it('registers the history, annotate, revision and branch actions with locale-independent labels', () => {
    const run = vi.fn();
    const text = (key: Parameters<typeof currentFileText>[0]) => currentFileText(key, 'en');
    const items = currentFileItems(run, text);
    expect(items.map(item => item.id)).toEqual([
      'Git.CurrentFile.ShowHistory', 'Git.CurrentFile.Annotate',
      'Git.CurrentFile.CompareRevision', 'Git.CurrentFile.CompareBranch',
    ]);
    expect(items.map(item => item.label)).toEqual([
      currentFileText('history', 'en'), currentFileText('annotate', 'en'),
      currentFileText('compareRevision', 'en'), currentFileText('compareBranch', 'en'),
    ]);
    items[0].run?.(); items[1].run?.(); items[2].run?.(); items[3].run?.();
    expect(run.mock.calls.map(([action]) => action)).toEqual(['history', 'annotate', 'compareRevision', 'compareBranch']);
  });
});
