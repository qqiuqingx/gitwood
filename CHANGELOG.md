# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Nothing yet.

## [0.2.0] - 2026-11-06

macOS only (universal DMG, ad-hoc signed, not notarized). Windows and Linux are
not yet runtime-verified. See `docs/perf/v0.2-macos.md` for measurements.

### Added

- Detection of in-progress merge, rebase, cherry-pick and revert, with
  Continue / Abort / Skip; multi-selection in the Git log; a per-window
  notification center.
- Conflict resolution: a conflicts dialog (Accept Yours / Accept Theirs / Merge)
  and an IDEA-style three-way merge editor, including deleted and binary sides
  and recovery after an interrupted update.
- Merge, Rebase and Pull dialogs with a branch picker, upstream tracking setup,
  and branch / commit comparison.
- History operations: Cherry-pick, Revert, Reset (soft / mixed / hard / keep),
  checkout of a revision, tags (create, delete, push), and Go to Child Commit.
- Stash changes, a stash manager (apply, pop, drop, clear, view), and a Changes
  context menu.
- Current File submenu: file history, Annotate (Blame) with previous-revision
  navigation, and comparison with a revision or branch.
- A native Git menu in IDEA order, enabled per project window from repository
  state and selection; macOS shortcuts ⌘K (Commit), ⇧⌘K (Push), ⌘T (Update
  Project), ⌃V (VCS Operations popup).
- User themes: JSON themes in the user theme directory, `extends` inheritance
  from built-in themes, hot reload, preview in a theme settings dialog, and
  WCAG contrast hints.
- GitHub / GitLab submenus without an API token: open files, commits and
  branches in the browser, copy links, and open the Pull / Merge Request
  comparison page; self-hosted hosts are configurable.
- Reveal in Finder for changed files; open the theme directory.
- IDEA-style diff editor tabs (preview and pinned tabs) with a configurable tab
  limit.

### Changed

- `tauri-plugin-opener` 2.7.2 is added and used only from Rust: the WebView has
  no opener permission, links must be `https://`, and repository files are only
  revealed in the file manager, never opened with their default application.

### Fixed

- The Amend checkbox in the commit panel could not be unchecked.
- The preview kept showing a file that no longer had changes after a commit, and
  an "unable to read repository" error could appear.
- Opening a project could show a `repository.stale` error.
- History-operation preferences and the tab limit now sync to all project
  windows.
- Annotate runs a single streaming `git blame` per file instead of one per
  200-line page; on a 20,000-line file with 10,000 revisions, viewing the whole
  file now takes about 6 seconds instead of minutes.

### Known issues

- The universal macOS app is 20.8 MiB, above the 20 MiB size target.

## [0.1.0] - 2026-11-03

macOS only. This milestone was not published as a GitHub Release; v0.2.0 is
the first published release.

### Added

- Open local repositories with a recent list, `safe.directory` handling, and
  worktree identification.
- Status, local/remote branches, tags and tracking information.
- Commit graph rendered on Canvas, with swimlane layout ported from IntelliJ
  IDEA's vcs-log; history filtering and commit/file/detail linkage.
- IDEA-style double-pane diff (CodeMirror 6): word-level highlight, difference
  navigation (F7 / Shift+F7), synchronized scrolling, unchanged-region folding,
  and an adjustable split ratio.
- File and hunk selection in Changes / Unversioned files, partial commit through
  a temporary index, Amend, Undo Commit, and rollback.
- Branch popup (search, grouping, submenus) and new/checkout/rename/delete
  branches, plus upstream tracking.
- Fetch, Update (merge or rebase) and Push with progress, rejection handling and
  conflict detection.
- Credential input dialog (askpass) for Git and SSH prompts; Gitwood never stores
  the credentials it asks for.
- Three built-in themes (Light, Dark, Walnut) with a follow-system switch.
- Six resizable dividers and tool-window show/hide, persisted across restarts
  (split ratios, diff ratio, and the Commit grouping/preview toggles).
- A Rust core (GitRunner with security flags, partial-commit probe, commit-graph
  layout) plus the desktop shell, with no telemetry.
- Apache-2.0 repository skeleton, community and security documents, and a
  three-platform CI configuration (workflow only; see Status in the README).
