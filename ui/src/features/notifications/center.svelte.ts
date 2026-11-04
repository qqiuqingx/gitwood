import { addNotice, AUTO_HIDE_MS, removeNotice, type Notice, type NoticeInput } from './model';

export class NotificationCenter {
  items = $state.raw<Notice[]>([]);
  private next = 1;
  private timers = new Map<number, ReturnType<typeof setTimeout>>();

  notify(input: NoticeInput): number {
    const id = this.next++;
    const before = new Set(this.items.map(notice => notice.id));
    this.items = addNotice(this.items, input, id);
    for (const old of before) if (!this.items.some(notice => notice.id === old)) this.clearTimer(old);
    // One timer per transient notice; nothing runs while the window is idle.
    if (!this.items.find(notice => notice.id === id)?.sticky) this.timers.set(id, setTimeout(() => this.dismiss(id), AUTO_HIDE_MS));
    return id;
  }
  dismiss(id: number) {
    this.clearTimer(id);
    this.items = removeNotice(this.items, id);
  }
  /** Repository switch: notices and their actions belong to the previous repository. */
  clear() {
    for (const id of this.timers.keys()) this.clearTimer(id);
    this.items = [];
  }
  private clearTimer(id: number) {
    const timer = this.timers.get(id);
    if (timer !== undefined) clearTimeout(timer);
    this.timers.delete(id);
  }
}

/** Each project window has its own WebView, so a module instance is per window. */
export const notifications = new NotificationCenter();
