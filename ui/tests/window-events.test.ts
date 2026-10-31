import { beforeEach, expect, it, vi } from 'vitest';
import type { EventCallback, EventTarget } from '@tauri-apps/api/event';
const events = vi.hoisted(() => ({
  current: 'main',
  handlers: [] as { event: string; target: EventTarget; handler: EventCallback<unknown> }[],
}));
vi.mock('@tauri-apps/api/webviewWindow', () => ({ getCurrentWebviewWindow: () => ({ label: events.current }) }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: async (event: string, handler: EventCallback<unknown>, options?: { target: EventTarget }) => {
    const entry = { event, handler, target: options?.target ?? { kind: 'Any' as const } };
    events.handlers.push(entry);
    return () => { events.handlers = events.handlers.filter(item => item !== entry); };
  },
}));
import { listenWindowEvent } from '../src/windowEvents';

beforeEach(() => { events.current = 'main'; events.handlers = []; });
function emit(label: string, event: string, payload: unknown) {
  // Tauri 2's match_any_or_filter: Any listeners receive even targeted events.
  for (const entry of events.handlers) {
    if (entry.event === event && (entry.target.kind === 'Any' || ('label' in entry.target && entry.target.label === label))) {
      entry.handler({ event, id: 1, payload });
    }
  }
}
it.each(['repository-refresh', 'remote_progress', 'preferences-changed', 'repository-menu', 'save-before-exit'])(
  '%s affects only its window even when repository generations match', async event => {
    const first = vi.fn(), second = vi.fn();
    await listenWindowEvent(event, first);
    events.current = 'project-0';
    const unlisten = await listenWindowEvent(event, second);
    emit('main', event, [1, 'first']);
    expect(first).toHaveBeenCalledTimes(1); expect(second).not.toHaveBeenCalled();
    emit('project-0', event, [1, 'second']);
    expect(first).toHaveBeenCalledTimes(1); expect(second).toHaveBeenCalledTimes(1);
    unlisten(); emit('project-0', event, [1, 'after dispose']);
    expect(second).toHaveBeenCalledTimes(1);
  },
);
