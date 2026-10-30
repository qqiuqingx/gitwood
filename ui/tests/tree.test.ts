import { describe, expect, it } from 'vitest';
import { flattenTree, treeArrow, type TreeSource } from '../src/components/tree';
const source: TreeSource = {
  rootCount: 2,
  getRoot: (i) => ({ id: `root${i}`, label: `Root ${i}`, hasChildren: i === 0 }),
  childCount: () => 2,
  getChild: (_, i) => ({ id: `child${i}`, label: `Child ${i}`, hasChildren: false }),
};
describe('tree visible descriptors and keyboard navigation', () => {
  it('does not read collapsed children', () => {
    const rows = flattenTree({ ...source, childCount: () => { throw new Error('hidden subtree read'); } }, new Set());
    expect(rows.map((r) => r.id)).toEqual(['root0', 'root1']);
  });
  it('flattens depth first with parent index and ARIA metadata', () => {
    const rows = flattenTree(source, new Set(['root0']));
    expect(rows.map((r) => [r.id, r.parentIndex, r.level, r.posInSet, r.setSize])).toEqual([
      ['root0', -1, 1, 1, 2], ['child0', 0, 2, 1, 2], ['child1', 0, 2, 2, 2], ['root1', -1, 1, 2, 2],
    ]);
    expect(rows[1].expanded).toBeUndefined();
  });
  it('right expands first, then enters the first child; left collapses first, then selects parent', () => {
    const closed = flattenTree(source, new Set());
    const open = flattenTree(source, new Set(['root0']));
    expect(treeArrow(closed, 0, 'ArrowRight')).toEqual({ index: 0, expansion: { id: 'root0', expanded: true } });
    expect(treeArrow(open, 0, 'ArrowRight')).toEqual({ index: 1 });
    expect(treeArrow(open, 0, 'ArrowLeft')).toEqual({ index: 0, expansion: { id: 'root0', expanded: false } });
    expect(treeArrow(open, 2, 'ArrowLeft')).toEqual({ index: 0 });
  });
  it('leaves leaves/root/empty selection stable when no transition applies', () => {
    const rows = flattenTree(source, new Set());
    expect(treeArrow(rows, 1, 'ArrowRight')).toEqual({ index: 1 });
    expect(treeArrow(rows, 0, 'ArrowLeft')).toEqual({ index: 0 });
    expect(treeArrow([], -1, 'ArrowRight')).toEqual({ index: -1 });
  });
  it('uses an iterative traversal for deeply nested trees', () => {
    const deep: TreeSource = { rootCount: 1, getRoot: () => ({ id: '0', label: '', hasChildren: true }),
      childCount: () => 1, getChild: (id) => ({ id: String(+id + 1), label: '', hasChildren: +id < 4998 }) };
    expect(flattenTree(deep, new Set(Array.from({ length: 5000 }, (_, i) => String(i))))).toHaveLength(5000);
  });
});
