import { describe, expect, it } from 'vitest';
import { activeDiffTabs, closeDiff, DiffTabsModel, emptyDiffTabs, openDiff, pinDiff, previewDiff, touchDiffTab, trimDiffTabs, type DiffTabInput } from '../src/features/diff/tabs.svelte';

const input = (path: string, area: DiffTabInput['area'] = 'commit', hash = ''): DiffTabInput => ({
  source: hash ? 'commit' : 'worktree', path: { display: path, bytes: [...new TextEncoder().encode(path)] }, hash,
  revision: hash || 'head-one', leftRevision: hash ? 'parent-hash' : 'HEAD', rightRevision: hash || 'Working tree',
  staged: true, unstaged: true, area,
});

describe('diff tab model', () => {
  it('replaces the single preview tab', () => {
    const first = previewDiff(emptyDiffTabs(), input('a.ts'));
    const next = previewDiff(first, input('b.ts'));
    expect(next.tabs).toHaveLength(1);
    expect(next.tabs[0].preview).toBe(true);
    expect(next.tabs[0].path.display).toBe('b.ts');
  });

  it('activates an existing fixed tab instead of duplicating it', () => {
    const first = openDiff(emptyDiffTabs(), input('a.ts', 'commit', 'commit-a'));
    const second = openDiff(first, input('b.ts', 'commit', 'commit-b'));
    const duplicate = openDiff(second, input('a.ts', 'commit', 'commit-a'));
    expect(duplicate.tabs).toHaveLength(2);
    expect(duplicate.activeId).toBe(first.activeId);
  });

  it('promotes a matching preview to a fixed tab', () => {
    const preview = previewDiff(emptyDiffTabs(), input('a.ts'));
    const pinned = pinDiff(preview, preview.activeId);
    expect(pinned.tabs).toHaveLength(1);
    expect(pinned.tabs[0].preview).toBe(false);
    expect(pinned.activeId).toBe(preview.activeId);
  });

  it('activates the adjacent tab on close, preferring the previous neighbour', () => {
    let state = openDiff(emptyDiffTabs(), input('a.ts', 'commit', 'a'));
    state = openDiff(state, input('b.ts', 'commit', 'b'));
    state = openDiff(state, input('c.ts', 'commit', 'c'));
    const closed = closeDiff(state, state.activeId);
    expect(closed.tabs.map(tab => tab.path.display)).toEqual(['a.ts', 'b.ts']);
    expect(closed.activeId).toBe(state.tabs[1].id);
    const firstState = { ...state, activeId: state.tabs[0].id };
    const firstClosed = closeDiff(firstState, state.tabs[0].id);
    expect(firstClosed.activeId).toBe(firstClosed.tabs[0].id);
  });

  it('clears all tabs when the project changes', () => {
    const model = new DiffTabsModel();
    model.open(input('a.ts', 'commit', 'a'));
    model.open(input('b.ts', 'commit', 'b'));
    model.clear();
    expect(model.tabs).toEqual([]);
    expect(model.activeId).toBe('');
  });

  it('provides only the active tab to the DiffView renderer', () => {
    let state = openDiff(emptyDiffTabs(), input('a.ts', 'commit', 'a'));
    state = openDiff(state, input('b.ts', 'commit', 'b'));
    expect(activeDiffTabs(state.tabs, state.activeId).map(tab => tab.id)).toEqual([state.activeId]);
    expect(activeDiffTabs(state.tabs, 'missing')).toEqual([]);
  });

  it('closes the least recently used inactive tab beyond the limit', () => {
    const model = new DiffTabsModel();
    model.setLimit(3);
    model.open(input('a.ts', 'commit', 'a'));
    model.open(input('b.ts', 'commit', 'b'));
    model.open(input('c.ts', 'commit', 'c'));
    model.select(model.tabs[0].id);
    model.open(input('d.ts', 'commit', 'd'));
    expect(model.tabs.map(tab => tab.path.display)).toEqual(['a.ts', 'c.ts', 'd.ts']);
    expect(model.tabs.find(tab => tab.id === model.activeId)?.path.display).toBe('d.ts');
  });

  it('trims existing tabs when the limit is lowered but keeps the active tab', () => {
    const model = new DiffTabsModel();
    for (const name of ['a', 'b', 'c', 'd']) model.open(input(`${name}.ts`, 'commit', name));
    model.select(model.tabs[0].id);
    model.setLimit(1);
    expect(model.tabs.map(tab => tab.path.display)).toEqual(['a.ts']);
  });

  it('closes tabs without history before recently used ones', () => {
    let state = openDiff(emptyDiffTabs(), input('a.ts', 'commit', 'a'));
    state = openDiff(state, input('b.ts', 'commit', 'b'));
    state = touchDiffTab(state);
    expect(trimDiffTabs(state, 1).tabs.map(tab => tab.path.display)).toEqual(['b.ts']);
  });
});
