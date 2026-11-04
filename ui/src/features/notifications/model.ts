/**
 * One notification list per project window, like IDEA VcsNotifier balloons: success and
 * information hide after a while, warnings and errors stay until dismissed or replaced.
 * Repository text is shown as plain text only.
 */
export type NoticeKind = 'info' | 'success' | 'warning' | 'error';
export interface NoticeAction { label: string; run: () => void }
export interface NoticeInput { kind: NoticeKind; title: string; detail?: string; files?: readonly string[]; actions?: readonly NoticeAction[]; group?: string }
export interface Notice extends NoticeInput { id: number; sticky: boolean }

export const AUTO_HIDE_MS = 10_000;
/** Bound memory and screen space; older notices are dropped first. */
export const MAX_NOTICES = 5;

export const isSticky = (kind: NoticeKind) => kind === 'warning' || kind === 'error';

/** A notice of the same group (e.g. one operation) replaces the previous one. */
export function addNotice(list: readonly Notice[], input: NoticeInput, id: number): Notice[] {
  const kept = input.group ? list.filter(notice => notice.group !== input.group) : [...list];
  return [...kept, { ...input, id, sticky: isSticky(input.kind) }].slice(-MAX_NOTICES);
}
export function removeNotice(list: readonly Notice[], id: number): Notice[] {
  return list.filter(notice => notice.id !== id);
}
