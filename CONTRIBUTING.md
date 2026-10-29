# Contributing to Gitwood

Gitwood is in early development. Read [AGENTS.md](AGENTS.md), the
[implementation plan](docs/implementation-plan.md), [architecture](docs/architecture.md),
and [Code of Conduct](CODE_OF_CONDUCT.md) before contributing.

Discuss changes that affect public APIs, persistent formats, dependencies, or
workflow scope before implementation. Keep changes focused and preserve unrelated
work. New dependencies must have Apache-2.0-compatible licenses; update both lockfiles
as appropriate and refresh the license inventory in NOTICE.

After installation, regenerate the locked dependency inventory and available
upstream notices with `python3 scripts/update-licenses.py`. Review SPDX expressions
and any missing standalone license files against the linked upstream sources.

Use Rust stable, Node 24 LTS (Node 25.6.1 also supported), pnpm 12.16.1, and the
[Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).
From the repository root, install with `pnpm install --frozen-lockfile`, then run:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm -C ui check
pnpm -C ui test
pnpm tauri build
```

UI development uses `pnpm tauri dev`. The production build extracts local CSS and
must retain the strict CSP. Regenerate icons using
`pnpm tauri icon src-tauri/icons/source.svg --output src-tauri/icons`.

Tests should cover actual production paths, including original bug triggers.
The stage 0 filename baseline requires system Git and uses disposable repositories
under `target/`; it skips tab/newline filenames on Windows. Future domain tests
must exercise the production Git runner with real temporary repositories.
Do not introduce test-only switches, APIs, or fake success feedback. Mark platform
restrictions for filenames explicitly in Git regression tests.

PRs should explain the user-visible change, scope, checks actually run, and remaining
limitations. UI interaction claims need an interaction-spec entry and comparison
evidence. Screenshots must not expose private repository data.

Contributions are made under Apache-2.0. Preserve third-party notices and record
the exact file/license/revision of every port. Do not use upstream trademarks or
logos as Gitwood branding. Do not publish or push without maintainer authorization.
