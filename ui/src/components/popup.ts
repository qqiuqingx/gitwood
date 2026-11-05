import { matchSpeedSearch } from './speed-search';
import type { Snippet } from 'svelte';
export interface Rect { x: number; y: number; width: number; height: number }
export interface PopupItem {
  id: string;
  label: string;
  kind?: 'item' | 'heading' | 'separator';
  disabled?: boolean;
  description?: string;
  /** Tooltip, e.g. why an item is disabled (IDEA action description). */
  hint?: string;
  icon?: Snippet<[id: string]>;
  shortcut?: string;
  groupHeader?: boolean;
  firstGroup?: boolean;
  checked?: boolean;
  children?: readonly PopupItem[];
  /** Tree depth for inline-expanded rows such as branch folders. */
  level?: number;
  /** Run without closing the popup, e.g. toggling a folder in place. */
  keepOpen?: boolean;
  /** Inline decoration right after the label, e.g. incoming/outgoing counts. */
  badge?: Snippet<[id: string]>;
  /** Right-aligned decoration before the submenu chevron. */
  trailing?: Snippet<[id: string]>;
  run?: () => void;
}

/** Returns the quick key's row; disabled items keep their slot and remain non-executable. */
export function quickSelectIndex(items: readonly PopupItem[], key: string): number {
  const normalized = key.toLowerCase();
  if (!/^[a-z0-9]$/.test(normalized)) return -1;
  return items.findIndex(item => (!item.kind || item.kind === 'item') && item.shortcut?.toLowerCase() === normalized);
}

/** Keep a section title only when at least one of its items matches. */
export function filterPopupItems(items: readonly PopupItem[], query: string): readonly PopupItem[] {
  if (!query) return items;
  const result: PopupItem[] = [];
  let heading: PopupItem | undefined;
  let hasGroup = false;
  for (const item of items) {
    if (item.kind === 'heading') { heading = item; continue; }
    if (item.kind === 'separator' || matchSpeedSearch(query, `${item.label} ${item.description ?? ''}`) === null) continue;
    if (heading) {
      result.push(heading.groupHeader ? { ...heading, firstGroup: !hasGroup } : heading);
      hasGroup ||= !!heading.groupHeader; heading = undefined;
    }
    result.push(item);
  }
  return result;
}

/** Prefer below an anchor (beside for submenus), flip then clamp to viewport. */
export function positionPopup(anchor: Rect, size: { width: number; height: number }, viewport: { width: number; height: number }, margin: number, submenu = false) {
  const width = Math.min(size.width, Math.max(0, viewport.width - 2 * margin));
  const height = Math.min(size.height, Math.max(0, viewport.height - 2 * margin));
  let x = submenu ? anchor.x + anchor.width : anchor.x;
  let y = submenu ? anchor.y : anchor.y + anchor.height;
  if (x + width > viewport.width - margin) x = submenu ? anchor.x - width : anchor.x + anchor.width - width;
  if (y + height > viewport.height - margin) y = submenu ? anchor.y + anchor.height - height : anchor.y - height;
  return {
    x: Math.max(margin, Math.min(x, viewport.width - margin - width)),
    y: Math.max(margin, Math.min(y, viewport.height - margin - height)), width, height,
  };
}
