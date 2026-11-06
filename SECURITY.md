# Security policy

Gitwood is in early development and has no supported public release yet. There is
no telemetry, automatic updater, or new plaintext credential store.

## Reporting

Do not disclose exploitable details, private repository content, or credentials in
public issues. A dedicated private reporting channel has not been configured yet.
Contact the maintainer through the contact methods on the
[maintainer profile](https://github.com/qqiuqingx) to request a private channel,
without including sensitive details in the initial request.

## Opening untrusted repositories

The Rust core enforces these rules in GitRunner; the desktop shell exposes typed,
read-only repository operations through the same runner.

- Treat `.git/config`, worktree config and their includes as untrusted:
  fsmonitor, filters, external diff/textconv, signature verification and some
  revision/merge display options can cause command execution.
- Background reads must pass `--no-optional-locks`, disable fsmonitor with
  `-c core.fsmonitor=false`, and use `--no-ext-diff --no-textconv` for `diff`,
  `show`, and `log`. The typed Rust runner centralizes these rules, disables the untracked cache and
  pager, overrides `core.sshCommand`, and bans transports for reads (including lazy
  promisor-object fetching). It rejects options that re-enable unsafe helpers.
  Verbose status (`-v`/`-vv`/`--verbose` and short-option clusters) is rejected:
  its internal diff can execute textconv and has no no-textconv switch.
- Before each passive read, query effective filter names with NUL-delimited
  `config --show-scope --name-only --get-regexp` (no helper execution). Disable
  clean/smudge/process and required for every driver with any local/worktree or
  unknown-scope property, including conditional includes. No cache is used: this
  adds one Git/config lookup per read and observes changes on the next read.
- **Global/system filters (e.g. user-installed Git LFS) and trusted backend
  command-scope filters remain enabled.** They can execute on repository content
  and belong to the trusted user environment. A local overlay disables that
  entire driver, even if it also has global settings. Local filters do not run
  on passive reads, so content/diffs can appear unfiltered and differ from native
  writes. This policy assumes config is not actively rewritten between the
  preflight and read; it does not sandbox installed Git or trusted helpers.
- Disable `log.showSignature`, override `format.pretty`, and reject
  `--show-signature`, signature `%G*` formatting and configured pretty aliases.
  Passive reads never request GPG/SSH/X.509 signature verification. Reject
  alternate-ref helper enumeration and remerge-diff/custom merge-driver paths.
- Ignore submodules entirely in passive status/diff/log/show/diff-tree, and
  reject overrides/recursion. A parent's filter-name list cannot protect a
  child-local driver. Dirty child state and changed gitlinks are hidden until a
  future feature opens each child with its own hardened runner.
- Honor Git `safe.directory`. Explain rejection and never silently modify global
  configuration or mark a repository safe.
- Explicit user-initiated writes retain existing Git configuration and hooks.
  Distinguish these operations from passive reads.
- Cover malicious configuration, special filenames, hook rejection, signing
  failure, and lock conflicts with real temporary repository regression tests.

## WebView security

- Load only bundled application resources in one window/WebView. Never navigate
  to remote pages. Development is limited to the local Vite server.
- Production CSP uses `default-src 'self'`, local scripts, styles, images and fonts,
  no frames, no objects, no forms, and no `unsafe-inline` or `unsafe-eval`.
  `ipc:` and `http://ipc.localhost` are Tauri's internal IPC endpoints, not remote
  resource hosts. Tauri adds hashes/nonces for its own bootstrap code.
- Svelte/Vite extracts production CSS. Splitter sizes are assigned through CSSOM
  style properties. The development-only CSP permits inline styles for Vite CSS
  hot replacement and `ws://localhost:1420` for HMR; it is not the release CSP.
- Capabilities explicitly grant `allow-app-version`, `allow-get-theme`,
  `allow-get-settings`, `allow-save-layout`, `allow-finish-exit`, and core event listen/unlisten to `main`.
  Theme resolution is restricted to bundled JSON; settings writes always target
  `app_config_dir/settings.json` and accept only six typed split ratios, not paths.
  No frontend theme loader or generic event emit permission is exposed. The application
  command manifest opts custom commands into ACL enforcement. No `core:default`,
  shell plugin, filesystem plugin, remote capability, or global Tauri object is enabled.
  `tauri-plugin-opener` is called only from typed Rust commands; no `opener:*`
  permission is granted to the WebView. Hosting commands accept a remote name and
  structured target, re-read the repository's configured remote URL, discard URL
  credentials, resolve the host from built-in or user-configured hostname entries,
  verify the target against local remote-tracking refs, and generate only `https://`
  links. They never accept a URL from the frontend and never make HTTP/API requests.
  Repository files are only revealed in the system file manager after Rust resolves
  a repository-relative path, canonicalizes it inside the worktree, and verifies it
  is an existing regular file. The default application is never used to open
  repository files. The theme directory command has no path parameter and opens
  only the configured user theme directory after creating it.
- Render repository messages, filenames, branch names, and other repository text
  as text. Never use `{@html}`, `innerHTML`, or repository-driven executable markup.
- Devtools are disabled in the window configuration, including release builds;
  the Rust `devtools` feature is not enabled.

## Process execution

- Launch Git with argument arrays (`std::process::Command` / equivalent), never
  concatenate user input, filenames, or paths into a shell string. Use `--` where
  appropriate to separate path arguments from options.
- Run process I/O and parsing in Rust background tasks; send only bounded, visible
  data through typed IPC. Validate frontend inputs and repository ownership in Rust.
- Serialize application writes and detect external changes, locks, cancellation,
  and stale results. Do not overwrite unrelated staged or working-tree content.
- Reads default to 30 seconds per subprocess; writes default to no timeout and
  remain cancellable. Callers can set an explicit limit. Unix cancellation/
  timeout terminates Git's private process group with SIGTERM, then SIGKILL after
  200 ms, and reaps Git. Escaped sessions and arbitrary hook effects cannot be
  rolled back. Windows currently terminates only Git; Job Objects are not yet
  implemented and descendants can survive. Signal cancellation and await the
  result; dropping a future only guarantees direct-child kill-on-drop.
- Reuse system Git credential helpers and SSH configuration. Do not persist new
  plaintext credentials or expose credentials through logs.

Repository IPC additionally grants only open/recent/close, status, refs, history
layout/rows and commit details/files commands. History and file pages are capped
at 200 entries; repository switching/refresh cancels and rejects stale generations.
Ref enumeration uses fixed formats and rejects signature-verification atoms.

See the [architecture](docs/architecture.md) for Git 2.35.2 requirements, alternate
index preservation/conflict rules, post-commit recovery limits and deferred E2E
decisions and the nine-operation external-helper audit. Completed commits with
failed index reconciliation are a distinct success-with-recovery result, carrying
the hash, snapshot directory and manual recovery instructions; never retry them
as failed commits. Recovery files normally live inside the per-worktree Git
directory with a timestamp and commit hash in the name. An I/O failure there
retains the system-temp snapshot and reports the actual path/error.

## Stage 3 user-initiated writes

The explicit commit/rollback IPC commands accept typed inclusion rules, raw-byte
repository path identities and snapshot tokens, never shell text, arbitrary patches,
Git arguments or environment overrides. Rust resolves them against the current
repository, builds patches from its comparison ranges and rejects stale selections.
Session writes reject overlap; all core commit/rollback operations share one serial
write gate. Cancellation remains live across read refreshes. The real index is
locked and snapshotted; rollback also uses an alternate index while restoring the
confirmed working files. External locks are never removed. Rollback deletes only
confirmed single files, never directories recursively.

Fingerprint checks include status/HEAD, index bytes and changed file contents/modes;
checks before and after writes detect observed external modifications. Git locks
cannot prevent arbitrary direct filesystem/ref writes in the final check-to-write
window. Rollback interrupted after restoring some working files requires inspection.
Completed commits needing index reconciliation show a distinct recovery dialog with
the actual snapshot directory and instructions; the UI never retries them as ordinary
failures. Native WebView/IDEA and non-macOS runtime acceptance remain separate from
core/session and frontend controller tests.

## Stage 4 remote operations

Remote fetch/update/push explicitly disable terminal prompting, retain configured
credential helpers and SSH commands, and default to `ssh -o BatchMode=yes` only
when no user SSH command/helper is configured. Unknown/changed host keys are
never trusted automatically. Use a terminal fetch to establish credentials and
verify host keys. There is no ASKPASS credential dialog in v0.1.
Remote error and progress output redacts URL userinfo. Push parses porcelain ref
flags even on nonzero exit. All new IPC commands have explicit ACL entries and
share session cancellation and the core write gate. See architecture stage 4 for
remaining stash concurrency, large-preview and validation limitations.
