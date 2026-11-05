import type { Platform } from './shortcuts';
export interface Binding { action: string; shortcut: string; source: string; handler?: 'native-menu' }
// Verified at intellij-community 0698a5fe297dfa08069f015cc423a6a85ed7fc60.
// The macOS file declares parent="$default". Entries below are actual XML values;
// Swing's plain arrows/Home/End/Enter/Esc are component behavior, not invented XML actions.
const mac = 'platform/platform-resources/src/keymaps/Mac OS X 10.5+.xml';
const parent = 'platform/platform-resources/src/keymaps/$default.xml';
const diffBindings: readonly Binding[] = [
  { action: 'NextDiff', shortcut: 'F7', source: parent },
  { action: 'PreviousDiff', shortcut: 'shift F7', source: parent },
];
export const macKeymap: readonly Binding[] = [
  ...diffBindings,
  { action: 'Vcs.ShowMessageHistory', shortcut: 'control M', source: mac },
  { action: 'Vcs.ToggleAmendCommitMode', shortcut: 'control alt M', source: mac },
  { action: 'Vcs.RollbackChangedLines', shortcut: 'meta alt Z', source: 'platform/platform-impl/src/com/intellij/openapi/keymap/impl/MacOSDefaultKeymap.kt (mapModifiers)' },
  { action: 'ChangesView.Revert', shortcut: 'meta alt Z', source: 'platform/platform-impl/src/com/intellij/openapi/keymap/impl/MacOSDefaultKeymap.kt (mapModifiers)' },
  { action: 'Diff.ShowDiff', shortcut: 'meta D', source: `${parent} (mapModifiers)` },
  { action: 'ChangesView.AddUnversioned', shortcut: 'meta alt A', source: `${parent} (mapModifiers)` },
  { action: 'List-selectFirstRow', shortcut: 'alt up', source: mac },
  { action: 'List-selectLastRow', shortcut: 'alt down', source: mac },
  { action: 'List-selectFirstRowExtendSelection', shortcut: 'alt shift up', source: mac },
  { action: 'List-selectLastRowExtendSelection', shortcut: 'alt shift down', source: mac },
  { action: 'NextTab', shortcut: 'meta shift CLOSE_BRACKET', source: mac },
  { action: 'NextTab', shortcut: 'control RIGHT', source: mac },
  { action: 'PreviousTab', shortcut: 'meta shift OPEN_BRACKET', source: mac },
  { action: 'PreviousTab', shortcut: 'control LEFT', source: mac },
  { action: 'CloseContent', shortcut: 'meta W', source: mac },
  { action: 'CloseActiveTab', shortcut: 'control shift F4', source: mac },
  { action: 'ActivateVersionControlToolWindow', shortcut: 'meta 9', source: mac, handler: 'native-menu' },
  { action: 'ActivateCommitToolWindow', shortcut: 'meta 0', source: mac, handler: 'native-menu' },
  { action: 'CheckinProject', shortcut: 'meta K', source: 'platform/vcs-impl/resources/META-INF/VcsActions.xml (control K mapped on macOS; d0421b4e8)', handler: 'native-menu' },
  { action: 'Vcs.Push', shortcut: 'meta shift K', source: 'platform/dvcs-impl/resources/intellij.platform.vcs.dvcs.impl.xml (control shift K mapped on macOS; d0421b4e8)', handler: 'native-menu' },
  { action: 'Vcs.UpdateProject', shortcut: 'meta T', source: 'platform/vcs-impl/resources/META-INF/VcsActions.xml (control T mapped on macOS; d0421b4e8)', handler: 'native-menu' },
  { action: 'Vcs.QuickListPopupAction', shortcut: 'control V', source: `${mac} (d0421b4e8)`, handler: 'native-menu' },
  { action: 'HideActiveWindow', shortcut: 'shift ESCAPE', source: parent },
];
// Only inherited Diff bindings are sourced for Windows/Linux; runtime verification awaits v0.2.
export const windowsKeymap: readonly Binding[] = [...diffBindings,
  { action: 'Git.Branches', shortcut: 'control shift BACK_QUOTE', source: 'platform/platform-resources/src/keymaps/$default.xml; Mac OS X 10.5+.xml removes it' },
  { action: 'Vcs.ShowMessageHistory', shortcut: 'control M', source: parent },
  { action: 'Vcs.ToggleAmendCommitMode', shortcut: 'alt M', source: parent },
  { action: 'Vcs.RollbackChangedLines', shortcut: 'control alt Z', source: parent },
  { action: 'ChangesView.Revert', shortcut: 'control alt Z', source: parent },
  { action: 'Diff.ShowDiff', shortcut: 'control D', source: parent },
  { action: 'ChangesView.AddUnversioned', shortcut: 'control alt A', source: parent },
];
export function keymapFor(platform: Platform): readonly Binding[] {
  return platform === 'mac' ? macKeymap : windowsKeymap;
}
