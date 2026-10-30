import { describe, expect, it, vi } from 'vitest';
import { detectPlatform, matchesShortcut, parseShortcut } from '../src/keymap/shortcuts';
import { keymapFor, macKeymap } from '../src/keymap/keymaps';
import { createActionRegistry } from '../src/keymap/registry';
const event = (overrides = {}) => ({ key: 'w', code: 'KeyW', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false,
  preventDefault: vi.fn(), stopPropagation: vi.fn(), ...overrides });
describe('platform keymaps and dispatch', () => {
  it('parses IDEA XML modifiers and familiar macOS aliases', () => {
    expect(parseShortcut('Cmd Opt Ctrl Shift W')).toEqual({ key: 'W', meta: true, alt: true, ctrl: true, shift: true });
    expect(parseShortcut('control alt PAGE_UP').key).toBe('PageUp');
    expect(parseShortcut('meta shift CLOSE_BRACKET').key).toBe(']');
    expect(() => parseShortcut('Super W')).toThrow('Unknown modifier');
    expect(() => parseShortcut('')).toThrow('requires a key');
  });
  it('matches exact modifiers and ignores IME composition', () => {
    expect(matchesShortcut(event(), parseShortcut('meta W'))).toBe(true);
    expect(matchesShortcut(event({ ctrlKey: true }), parseShortcut('meta W'))).toBe(false);
    expect(matchesShortcut(event({ isComposing: true }), parseShortcut('meta W'))).toBe(false);
    expect(matchesShortcut(event({ key: '}', code: 'BracketRight', shiftKey: true }), parseShortcut('meta shift CLOSE_BRACKET'))).toBe(true);
    expect(matchesShortcut(event({ key: '∑', altKey: true }), parseShortcut('meta alt W'))).toBe(true);
  });
  it('selects platform bindings including inherited diff shortcuts', () => {
    expect(detectPlatform('MacIntel')).toBe('mac'); expect(detectPlatform('Win32')).toBe('windows');
    expect(detectPlatform('Linux')).toBe('linux'); expect(keymapFor('mac')).toBe(macKeymap);
    for (const platform of ['mac', 'windows', 'linux'] as const) {
      expect(keymapFor(platform).filter(b => ['NextDiff', 'PreviousDiff'].includes(b.action)).map(b => b.shortcut)).toEqual(['F7', 'shift F7']);
    }
    expect(macKeymap.find((b) => b.action === 'HideActiveWindow')?.source).toContain('$default.xml');
  });
  it('claims WebView defaults only for registered, enabled, matching actions', () => {
    const actions = createActionRegistry(); const run = vi.fn(); const e = event();
    expect(actions.dispatch(e, macKeymap)).toBe(false); expect(e.preventDefault).not.toHaveBeenCalled();
    let enabled = false;
    const unregister = actions.register('CloseContent', { run, enabled: () => enabled });
    expect(actions.dispatch(e, macKeymap)).toBe(false); enabled = true;
    expect(actions.dispatch(e, macKeymap)).toBe(true); expect(run).toHaveBeenCalledOnce();
    expect(e.preventDefault).toHaveBeenCalledOnce(); expect(e.stopPropagation).toHaveBeenCalledOnce();
    unregister(); expect(actions.dispatch(event(), macKeymap)).toBe(false);
  });
  it('leaves input editing, consumed keys and composition untouched unless explicitly allowed', () => {
    const actions = createActionRegistry(); const run = vi.fn();
    actions.register('CloseContent', { run });
    expect(actions.dispatch(event(), macKeymap, true)).toBe(false);
    expect(actions.dispatch(event({ defaultPrevented: true }), macKeymap)).toBe(false);
    expect(actions.dispatch(event({ isComposing: true }), macKeymap)).toBe(false);
    const optedIn = createActionRegistry(); optedIn.register('CloseContent', { run, allowInInput: true });
    expect(optedIn.dispatch(event(), macKeymap, true)).toBe(true);
  });
  it('rejects ambiguous duplicate action registration and supports cleanup', () => {
    const actions = createActionRegistry(); const dispose = actions.register('NextTab', { run: () => {} });
    expect(() => actions.register('NextTab', { run: () => {} })).toThrow('already registered');
    dispose(); expect(() => actions.register('NextTab', { run: () => {} })).not.toThrow();
  });
});

it('leaves menu accelerators to the native menu even when an action is registered', () => {
  const registry = createActionRegistry();
  let calls = 0;
  registry.register('ActivateCommitToolWindow', { run: () => calls++ });
  const event = { key: '0', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false,
    preventDefault: () => { throw new Error('native accelerator must not be consumed'); }, stopPropagation: () => {} };
  expect(registry.dispatch(event, macKeymap)).toBe(false);
  expect(calls).toBe(0);
});

it('uses source-backed Control+M history and recognizes macOS Option-modified rollback by physical key',()=>{
 const actions=createActionRegistry();const history=vi.fn(),rollback=vi.fn();actions.register('Vcs.ShowMessageHistory',{run:history,allowInInput:true});actions.register('ChangesView.Revert',{run:rollback});
 expect(actions.dispatch(event({key:'m',code:'KeyM',metaKey:false,ctrlKey:true}),macKeymap,true)).toBe(true);expect(history).toHaveBeenCalledOnce();
 expect(actions.dispatch(event({key:'Ω',code:'KeyZ',altKey:true}),macKeymap)).toBe(true);expect(rollback).toHaveBeenCalledOnce();
 expect(macKeymap.find(b=>b.action==='Vcs.ShowMessageHistory')?.source).toContain('Mac OS X 10.5+.xml');
});
