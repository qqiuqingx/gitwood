export interface ListRow {
  id: string;
  label: string;
  disabled?: boolean;
  kind?: 'item' | 'heading' | 'separator';
  level?: number;
  expanded?: boolean;
  posInSet?: number;
  setSize?: number;
}

export function selectable(row: ListRow): boolean {
  return !row.disabled && (!row.kind || row.kind === 'item');
}

/** End is exclusive. Never read rows outside this bounded range for rendering. */
export function visibleRange(count: number, rowHeight: number, scrollTop: number, height: number, overscan = 3) {
  if (rowHeight <= 0 || count <= 0 || height <= 0) return { start: 0, end: 0 };
  const start = Math.min(count, Math.max(0, Math.floor(scrollTop / rowHeight) - overscan));
  const end = Math.min(count, Math.ceil((Math.max(0, scrollTop) + height) / rowHeight) + overscan);
  return { start, end: Math.max(start, end) };
}

export interface Selection {
  ids: ReadonlySet<string>;
  anchor: number;
  active: number;
}

export function selectRow(
  state: Selection, index: number, count: number, getRow: (index: number) => ListRow,
  modifiers: { multi?: boolean; range?: boolean; toggle?: boolean } = {},
): Selection {
  if (index < 0 || index >= count || !selectable(getRow(index))) return state;
  const anchor = state.anchor >= 0 && state.anchor < count ? state.anchor : index;
  if (modifiers.multi && modifiers.range) {
    const ids = modifiers.toggle ? new Set(state.ids) : new Set<string>();
    for (let i = Math.min(anchor, index); i <= Math.max(anchor, index); i++) {
      if (selectable(getRow(i))) ids.add(getRow(i).id);
    }
    return { ids, anchor, active: index };
  }
  const ids = modifiers.multi && modifiers.toggle ? new Set(state.ids) : new Set<string>();
  const id = getRow(index).id;
  if (ids.has(id)) ids.delete(id); else ids.add(id);
  return { ids, anchor: index, active: index };
}

export function navigationIndex(
  key: string, active: number, count: number, pageSize: number,
  getRow: (index: number) => ListRow,
): number {
  const backward = ['ArrowUp', 'Home', 'PageUp'].includes(key);
  const step = backward ? -1 : 1;
  const last = count - 1;
  let target = key === 'Home' ? 0 : key === 'End' ? last
    : active < 0 ? (backward ? last : 0)
    : Math.max(0, Math.min(last, active + step * (key.startsWith('Page') ? Math.max(1, pageSize) : 1)));
  for (; target >= 0 && target < count; target += step) {
    if (selectable(getRow(target))) return target;
  }
  return active;
}
