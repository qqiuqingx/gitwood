export interface Rect { x: number; y: number; width: number; height: number }
export interface PopupItem {
  id: string;
  label: string;
  kind?: 'item' | 'heading' | 'separator';
  disabled?: boolean;
  children?: readonly PopupItem[];
  run?: () => void;
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
