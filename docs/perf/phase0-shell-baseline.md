# Phase 0 — empty shell resource baseline (macOS)

Measured 2026-10-29 on the commit that introduced the Tauri 2 + Svelte 5 shell
(`aad024f`). The shell contains one window, seven placeholder panels and the six
splitters; it opens no repository and runs no Git process.

## Environment

| Item | Value |
|---|---|
| Machine | Apple M2 Max, 32 GiB RAM |
| OS | macOS 27.0 |
| Build | `pnpm tauri build` (release, arm64 only, ad-hoc signed) |
| Artifact | `target/release/bundle/macos/Gitwood.app` |

## Method

1. Launch the bundle with `open -n Gitwood.app`, wait 15 s for startup to settle.
2. Identify the app process and the WebKit XPC processes it spawned
   (`com.apple.WebKit.WebContent`, `.GPU`, `.Networking`) by diffing the WebKit
   process list before and after launch.
3. CPU: sum `ps -o %cpu` over all four processes every 5 s for 60 s, no user input.
4. Memory: `footprint <pid>` (phys_footprint, the metric Activity Monitor shows as
   "Memory") per process, summed. RSS is listed for reference only; it double-counts
   shared system framework pages across processes.

## Results

| Metric | Result | Initial target |
|---|---|---|
| Idle CPU, 4 processes summed, 60 s average | 0.00 % (all 12 samples 0) | < 1 % |
| phys_footprint, app process | 31 MB | — |
| phys_footprint, WebKit.WebContent | 36 MB | — |
| phys_footprint, WebKit.GPU | 11 MB | — |
| phys_footprint, WebKit.Networking | 6 MB | — |
| **phys_footprint total** | **≈ 84 MB** | ≤ 200 MiB (typical-use scenario) |
| RSS sum (reference) | 194.9 MiB | — |
| `.app` size, arm64 only | 5.8 MiB (main binary 4.8 MiB) | Universal ≤ 20 MiB |

## Universal build size

Built 2026-10-29 with `pnpm tauri build --target universal-apple-darwin` after
`rustup target add x86_64-apple-darwin`, same commit as above.

| Artifact | Size | Initial target |
|---|---|---|
| `Gitwood.app` (x86_64 + arm64, ad-hoc signed) | 11.07 MiB (11,336 KiB on disk; main binary 10,491,504 bytes) | — |
| Compressed DMG (UDZO) | **4.69 MiB (4,913,404 bytes)** | ≤ 20 MiB |

The Tauri DMG step (`bundle_dmg.sh`) failed locally: its AppleScript call that lays
out the Finder window timed out (`AppleEvent timed out, -1712`) in a
non-interactive session. The DMG above was therefore created directly with
`hdiutil create -format UDZO -srcfolder Gitwood.app`; it has no custom Finder
layout, so the release workflow's DMG may differ slightly in size. The release
workflow DMG has not been run yet.

## Not yet measured

- DMG produced by the release workflow on GitHub Actions.
- Cold vs warm start time.
- Intel Mac, macOS 13 minimum-version run.
- Windows and Linux (from v0.2).
