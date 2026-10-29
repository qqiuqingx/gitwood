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

These are mandatory implementation rules for future Git access; this skeleton
does not yet open repositories or run Git commands.

- Treat `.git/config` as untrusted: `core.fsmonitor`, `core.sshCommand`,
  `diff.external`, and textconv can cause command execution.
- Background reads must pass `--no-optional-locks`, disable fsmonitor with
  `-c core.fsmonitor=false`, and use `--no-ext-diff --no-textconv` for `diff`,
  `show`, and `log`. Centralize these rules in the future Rust Git runner.
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
- Capabilities explicitly grant only `allow-app-version` to `main`. The application
  command manifest opts custom commands into ACL enforcement. No `core:default`,
  shell plugin, filesystem plugin, remote capability, or global Tauri object is enabled.
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
- Reuse system Git credential helpers and SSH configuration. Do not persist new
  plaintext credentials or expose credentials through logs.

See the [architecture](docs/architecture.md) for deferred Git/version/E2E decisions.
