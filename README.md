# Gitwood

[简体中文](README.zh-CN.md)

Gitwood is an open-source, cross-platform Git client built with Rust, Tauri 2,
Svelte 5, and TypeScript. Its interaction design references IntelliJ IDEA.
Gitwood uses its own name and original icon; it is an independent project.
No telemetry or usage data is collected.

**Status: v0.1 local-use development.** The application can open a local
repository, browse history and diffs, commit selected changes, use the IDEA-style
branch popup, create/checkout/rename/delete branches, fetch/update/push, and run
Commit and Push through the real IPC path. v0.1 targets macOS; Windows and Linux
builds are configured in CI, with releases planned from v0.2.

### Known limitations

- There is no credential input dialog; Git uses the configured credential helper and SSH setup.
- Conflict resolution is deferred to v0.2.
- Windows and Linux have not been runtime-verified.
- macOS artifacts are not notarized; an unsigned/ad-hoc download may need “Open Anyway”.

## Build from source

Install Rust stable through rustup, Node.js 24 LTS, and pnpm 12.16.1.
The local development toolchain also supports Node 25.6.1.
On macOS install Xcode Command Line Tools (`xcode-select --install`).
On Windows install Visual Studio C++ Build Tools and WebView2. On Ubuntu 22.04:

```sh
sudo apt update
sudo apt install build-essential curl wget file libwebkit2gtk-4.1-dev \
  libssl-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev
```

See the [official Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
for platform setup. Target systems: macOS 13+, Windows 10 1809+, Ubuntu 22.04 or
equivalent with webkit2gtk-4.1. Minimum-system compatibility is not yet verified.

Run from the repository root:

```sh
source ~/.cargo/env # macOS / Linux, if Rust is not on PATH
pnpm install --frozen-lockfile
pnpm tauri dev
```

Create a release-mode application bundle for the current macOS architecture:

```sh
pnpm tauri build
# target/release/bundle/macos/Gitwood.app
```

The default bundle target is `app`; Windows/Linux compilation can use
`pnpm tauri build --no-bundle`. To create a macOS universal DMG:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pnpm tauri build --target universal-apple-darwin --bundles dmg
```

macOS bundles are ad-hoc signed, without notarization. Future downloaded builds
may require approval in System Settings → Privacy & Security → Open Anyway.
The tag workflow uploads a universal DMG and SHA-256 as workflow artifacts;
publishing a GitHub Release remains a manual, maintainer-approved action.

## Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm -C ui check
pnpm -C ui test
pnpm tauri build
```

`cargo test` and Clippy also compile the desktop adapter. Production builds need
the frontend assets; `pnpm tauri build` builds those automatically.

See the [implementation plan](docs/implementation-plan.md),
[architecture](docs/architecture.md), [security rules](SECURITY.md), and
[contribution guide](CONTRIBUTING.md). The core runner requires Git 2.35.2 or newer;
the [E2E strategy](docs/architecture.md#e2e-strategy) uses native WebDriver on Windows/Linux and manual native acceptance on macOS. On Windows, install
[Git for Windows](https://gitforwindows.org/). The core includes GitRunner and an alternate-index partial-commit probe;
these operations are not yet exposed by the desktop shell.

## License

[Apache-2.0](LICENSE). See [NOTICE](NOTICE) and the
[dependency license inventory](docs/third-party-licenses.md).
