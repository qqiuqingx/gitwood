import { matchesShortcut, parseShortcut, type KeyEventLike } from './shortcuts';
import type { Binding } from './keymaps';
export interface Action {
  run: () => void;
  enabled?: () => boolean;
  /** Explicit opt-in; editing shortcuts are otherwise left to the input/editor. */
  allowInInput?: boolean;
}
export interface DispatchEvent extends KeyEventLike { preventDefault: () => void; stopPropagation: () => void }
export function createActionRegistry() {
  const actions = new Map<string, Action>();
  return {
    register(id: string, action: Action) {
      if (actions.has(id)) throw new Error(`Action already registered: ${id}`);
      actions.set(id, action);
      return () => { if (actions.get(id) === action) actions.delete(id); };
    },
    run(id: string): boolean {
      const action = actions.get(id);
      if (!action || action.enabled?.() === false) return false;
      action.run();
      return true;
    },
    dispatch(event: DispatchEvent, bindings: readonly Binding[], editing = false): boolean {
      if (event.defaultPrevented || event.isComposing) return false;
      for (const binding of bindings) {
        if (binding.handler === 'native-menu') continue;
        const action = actions.get(binding.action);
        if (!action || (editing && !action.allowInInput) || action.enabled?.() === false) continue;
        if (!matchesShortcut(event, parseShortcut(binding.shortcut))) continue;
        // Suppress WebView defaults only after an enabled application action claims it.
        event.preventDefault();
        event.stopPropagation();
        action.run();
        return true;
      }
      return false;
    },
  };
}
export function isEditingTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && !!target.closest('input, textarea, select, [contenteditable="true"], [role="textbox"]');
}
