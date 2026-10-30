import type { ListRow } from './list';
export interface TreeNode { id: string; label: string; disabled?: boolean; hasChildren: boolean }
export interface TreeSource {
  rootCount: number;
  getRoot: (index: number) => TreeNode;
  childCount: (id: string) => number;
  getChild: (id: string, index: number) => TreeNode;
}
export interface TreeRow extends ListRow { parentIndex: number; hasChildren: boolean }

/** Only visit expanded branches. IDs must be unique; no text/hidden subtree cache. */
export function flattenTree(source: TreeSource, expanded: ReadonlySet<string>): TreeRow[] {
  const rows: TreeRow[] = [];
  const stack: { parent: string | null; parentIndex: number; level: number; next: number; count: number }[] = [
    { parent: null, parentIndex: -1, level: 1, next: 0, count: source.rootCount },
  ];
  while (stack.length) {
    const frame = stack[stack.length - 1];
    if (frame.next >= frame.count) { stack.pop(); continue; }
    const position = frame.next++;
    const node = frame.parent === null ? source.getRoot(position) : source.getChild(frame.parent, position);
    const index = rows.length;
    rows.push({ ...node, parentIndex: frame.parentIndex, level: frame.level,
      expanded: node.hasChildren ? expanded.has(node.id) : undefined, posInSet: position + 1, setSize: frame.count });
    if (node.hasChildren && expanded.has(node.id)) stack.push({
      parent: node.id, parentIndex: index, level: frame.level + 1, next: 0, count: source.childCount(node.id),
    });
  }
  return rows;
}

export function treeArrow(rows: readonly TreeRow[], index: number, key: 'ArrowLeft' | 'ArrowRight') {
  const row = rows[index];
  if (!row) return { index };
  if (key === 'ArrowRight') {
    if (row.hasChildren && !row.expanded) return { index, expansion: { id: row.id, expanded: true } };
    if (row.expanded && rows[index + 1]?.parentIndex === index) return { index: index + 1 };
  } else {
    if (row.expanded) return { index, expansion: { id: row.id, expanded: false } };
    if (row.parentIndex >= 0) return { index: row.parentIndex };
  }
  return { index };
}
