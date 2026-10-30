import { describe, expect, it } from 'vitest';
import { navigationIndex, selectRow, visibleRange, type ListRow, type Selection } from '../src/components/list';
const row = (i: number): ListRow => ({ id: `id-${i}`, label: `File${i}` });
const initial: Selection = { ids: new Set(), anchor: -1, active: -1 };

describe('virtual list ranges', () => {
  it('renders a bounded slice of a hundred thousand rows', () => {
    expect(visibleRange(100000, 24, 24000, 240)).toEqual({ start: 997, end: 1013 });
  });
  it('clamps overscan at both ends and handles zero-sized hosts', () => {
    expect(visibleRange(5, 24, 0, 240)).toEqual({ start: 0, end: 5 });
    expect(visibleRange(0, 24, 0, 240)).toEqual({ start: 0, end: 0 });
    expect(visibleRange(5, 0, 0, 240)).toEqual({ start: 0, end: 0 });
    expect(visibleRange(5, 24, 0, 0)).toEqual({ start: 0, end: 0 });
    expect(visibleRange(5, 24, 999, 240)).toEqual({ start: 5, end: 5 });
  });
  it('includes partial rows and clamps negative scroll bounce', () => {
    expect(visibleRange(100, 20, 21, 21, 0)).toEqual({ start: 1, end: 3 });
    expect(visibleRange(100, 20, -40, 21, 0)).toEqual({ start: 0, end: 2 });
  });
});
describe('selection through the production List helper', () => {
  it('selects one item, toggles noncontiguous items and deselects', () => {
    const first = selectRow(initial, 2, 10, row);
    const second = selectRow(first, 7, 10, row, { multi: true, toggle: true });
    expect([...second.ids]).toEqual(['id-2', 'id-7']);
    expect([...selectRow(second, 2, 10, row, { multi: true, toggle: true }).ids]).toEqual(['id-7']);
    expect([...first.ids]).toEqual(['id-2']); // immutable selection updates
  });
  it('extends and shrinks a range from the original anchor', () => {
    const first = selectRow(initial, 5, 10, row);
    const range = selectRow(first, 2, 10, row, { multi: true, range: true });
    expect([...range.ids]).toEqual(['id-2', 'id-3', 'id-4', 'id-5']);
    expect([...selectRow(range, 4, 10, row, { multi: true, range: true }).ids]).toEqual(['id-4', 'id-5']);
  });
  it('supports additive ranges and ignores modifiers in single selection', () => {
    const first = selectRow(initial, 1, 10, row);
    const toggled = selectRow(first, 5, 10, row, { multi: true, toggle: true });
    expect([...selectRow(toggled, 7, 10, row, { multi: true, range: true, toggle: true }).ids])
      .toEqual(['id-1', 'id-5', 'id-6', 'id-7']);
    expect([...selectRow(first, 4, 10, row, { range: true, toggle: true }).ids]).toEqual(['id-4']);
  });
  it('skips headings, separators and disabled rows in selection ranges', () => {
    const get = (i: number): ListRow => i === 2 ? { ...row(i), kind: 'heading' } : i === 3 ? { ...row(i), disabled: true } : row(i);
    const first = selectRow(initial, 1, 6, get);
    expect(selectRow(first, 2, 6, get)).toBe(first);
    expect(selectRow(first, -1, 6, get)).toBe(first);
    expect([...selectRow(first, 4, 6, get, { multi: true, range: true }).ids]).toEqual(['id-1', 'id-4']);
  });
  it('navigates edges, page size and nonselectable rows', () => {
    const get = (i: number) => ({ ...row(i), disabled: i === 4 });
    expect(navigationIndex('ArrowDown', -1, 10, 3, get)).toBe(0);
    expect(navigationIndex('ArrowUp', 0, 10, 3, get)).toBe(0);
    expect(navigationIndex('PageDown', 1, 10, 3, get)).toBe(5);
    expect(navigationIndex('PageUp', 8, 10, 3, get)).toBe(5);
    expect(navigationIndex('Home', 8, 10, 3, get)).toBe(0);
    expect(navigationIndex('End', 0, 10, 3, get)).toBe(9);
    expect(navigationIndex('End', -1, 0, 3, get)).toBe(-1);
  });
});
