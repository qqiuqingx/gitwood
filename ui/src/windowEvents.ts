import { listen, type EventCallback } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

/** Global listeners also receive events aimed at other windows; scope project events explicitly. */
export function listenWindowEvent<T>(event: string, handler: EventCallback<T>) {
  return listen<T>(event, handler, {
    target: { kind: 'WebviewWindow', label: getCurrentWebviewWindow().label },
  });
}
