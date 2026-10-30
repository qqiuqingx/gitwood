export type Platform = 'mac' | 'windows' | 'linux';
export interface Keystroke { key: string; meta: boolean; ctrl: boolean; alt: boolean; shift: boolean }
export interface KeyEventLike {
  key: string; code?: string; metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean;
  isComposing?: boolean; defaultPrevented?: boolean;
}
const aliases: Record<string, string> = {
  up: 'ArrowUp', down: 'ArrowDown', left: 'ArrowLeft', right: 'ArrowRight',
  escape: 'Escape', esc: 'Escape', enter: 'Enter', page_up: 'PageUp', page_down: 'PageDown',
  home: 'Home', end: 'End', space: ' ', open_bracket: '[', close_bracket: ']',
  add: '+', subtract: '-',
};
export function parseShortcut(value: string): Keystroke {
  const parts = value.trim().split(/\s+/);
  const keyPart = parts.pop();
  if (!keyPart) throw new Error('Shortcut requires a key');
  const result: Keystroke = { key: aliases[keyPart.toLowerCase()] ?? keyPart, meta: false, ctrl: false, alt: false, shift: false };
  for (const modifier of parts) {
    if (/^(meta|cmd)$/i.test(modifier)) result.meta = true;
    else if (/^(control|ctrl)$/i.test(modifier)) result.ctrl = true;
    else if (/^(alt|opt|option)$/i.test(modifier)) result.alt = true;
    else if (/^shift$/i.test(modifier)) result.shift = true;
    else throw new Error(`Unknown modifier: ${modifier}`);
  }
  return result;
}
export function matchesShortcut(event: KeyEventLike, shortcut: Keystroke): boolean {
  // macOS Option and Shift can change event.key; use physical codes for ASCII shortcuts.
  const codeKey = event.code?.startsWith('Key') ? event.code.slice(3)
    : event.code?.startsWith('Digit') ? event.code.slice(5)
    : event.code === 'BracketLeft' ? '[' : event.code === 'BracketRight' ? ']' : undefined;
  return !event.isComposing && (codeKey ?? event.key).toLowerCase() === shortcut.key.toLowerCase()
    && event.metaKey === shortcut.meta && event.ctrlKey === shortcut.ctrl
    && event.altKey === shortcut.alt && event.shiftKey === shortcut.shift;
}
export function detectPlatform(platform: string): Platform {
  return /mac|iphone|ipad/i.test(platform) ? 'mac' : /win/i.test(platform) ? 'windows' : 'linux';
}
