# Architecture

Gitwood is in early development. This document describes the boundaries for the
stage 0 skeleton; the implementation plan remains the source of phase progress.

## Layers and dependency direction

```text
ui (Svelte 5 / TypeScript / Vite)
       | explicit, capability-scoped IPC
src-tauri (desktop adapter)
       | Rust calls
gitwood-core (domain library, no Tauri dependency)
```

- `gitwood-core`: Git execution/security, repository identity/status/refresh,
  history/graph/paging, diff interpretation, write operations, theme validation.
  The six modules currently contain only documentation, with no fake services.
- `src-tauri`: one native window and WebView, native menus, IPC input validation,
  task coordination and persistence adapters. The only current command is
  `app_version`, explicitly registered in the command manifest and capability.
- `ui`: rendering, interaction, keymap and i18n. It cannot launch processes or
  access the filesystem. Six instances of the reusable Splitter form the shell.
  Colors, fonts and spacing come from `src/theme/light.css`; system fonts are
  used and no fonts are bundled. Additional themes are deferred.

## Data and rendering

Keep repository data in Rust. Send bounded visible ranges and stable IDs through
IPC; never copy whole histories, file trees or diffs into the JS heap. Long lists
(commits, branches, changed files, diff) use virtual scrolling. Render graph lanes
on Canvas; only the current file is loaded in the future CodeMirror adapter.
Bound caches and release old repository data on switching.

Use filesystem events, filtering and debounce rather than idle polling. Recheck
on activation; cancel stale reads and discard outdated responses. Serialize writes
and account for external tools changing a repository. Resizing and theme changes
must not trigger Git reads, reconstruct data or reset selection/focus/scroll.

## Shell layout

The upper workspace splits Commit / Diff; Diff splits left / right. The lower Git
Log splits Branches / (Commits / Information), with Information splitting Changed
Files / Commit Details. The sixth divider separates the workspace / entire log.
There are no repository entries, fake actions, persistence or background Git reads.

Splitter uses pointer capture, cancellation and keyboard arrows (Shift for larger
steps, Home/End for bounds). ResizeObserver updates available space. Both panes
have minimum sizes; if an externally constrained host cannot fit both, space is
shared proportionally without negative tracks or overflow. Ratios live in UI
memory only. Rust-backed layout persistence belongs to stage 1.

## Security and build

Follow [SECURITY.md](../SECURITY.md). The production CSP has no inline/eval exception;
Svelte CSS is extracted. Trusted resize code changes individual CSSOM properties.
The dev CSP alone allows Vite-injected styles and the loopback HMR connection.
Internal IPC uses `ipc:` / `http://ipc.localhost`, as required by
[Tauri CSP documentation](https://v2.tauri.app/security/csp/). Custom commands opt
into [Tauri ACL enforcement](https://v2.tauri.app/security/capabilities/) rather
than relying on their default unrestricted application-command behavior.

The Cargo workspace uses resolver 2 and edition 2021. pnpm owns the root Tauri CLI
and `ui` workspace; build hooks run Vite from the repository root. Default macOS
packaging creates a native-architecture `.app`, ad-hoc signed with identity `-`.
CI compiles debug executables without bundling to reuse test artifacts. Tagged
release builds separately generate a universal DMG plus SHA-256; Windows/Linux
release jobs stay disabled until v0.2. No workflow automatically publishes a Release.

## Minimum Git version — TODO

Determine the version from the actual GitRunner parameters and detection behavior
in the later stage 0 step. The application does not invoke Git; only isolated
filename fixtures do. Do not trigger the macOS placeholder Git's Command Line
Tools installation dialog.

## E2E strategy — TODO

Confirm a viable macOS approach and document its limits. Tauri WebDriver support
on Windows/Linux alone is not evidence for macOS. No E2E framework or test-only
production switch is selected in this task.

## Verification boundaries

Local build results, CI platform runs, minimum-OS compatibility, Intel execution,
and memory/CPU baselines are separate evidence. Only the macOS native-architecture
build and local checks are required for this task. Stage 0 filename fixtures call
real Git directly with argument arrays: Unicode, spaces and leading dashes on all
platforms, plus tabs/newlines on non-Windows systems. They live under `target/`
and clean up after themselves. Domain/security regression tests await GitRunner;
the filename baseline alone does not validate a production repository reader.
