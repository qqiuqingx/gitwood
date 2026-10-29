# Gitwood

[English](README.md)

Gitwood 是使用 Rust、Tauri 2、Svelte 5 和 TypeScript 构建的跨平台开源 Git
客户端，交互参考 IntelliJ IDEA。项目使用自有名称和原创图标，独立开发，
不收集遥测或使用数据。

**状态：开发早期。** 当前仅有空壳应用和六处可拖动分隔线，尚未实现仓库浏览或
Git 操作。v0.1 面向 macOS，CI 配置覆盖 Windows 和 Linux，后两者从 v0.2 起
发布。目前尚未在 GitHub 上执行 CI。

## 从源码构建

准备 Rust stable（rustup）、Node.js 24 LTS、pnpm 12.16.1；本地工具链也支持
Node 25.6.1。macOS 安装 Xcode Command Line Tools（`xcode-select --install`）；
Windows 安装 Visual Studio C++ Build Tools 和 WebView2；Ubuntu 22.04 安装：

```sh
sudo apt update
sudo apt install build-essential curl wget file libwebkit2gtk-4.1-dev \
  libssl-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev
```

其他平台安装细节见 [Tauri 官方前置依赖](https://v2.tauri.app/start/prerequisites/)。
目标系统为 macOS 13+、Windows 10 1809+、Ubuntu 22.04 或具备 webkit2gtk-4.1 的
同等发行版；最低系统版本兼容性尚未验证。

在仓库根目录运行：

```sh
source ~/.cargo/env # macOS / Linux 中 Rust 不在 PATH 时执行
pnpm install --frozen-lockfile
pnpm tauri dev
```

构建当前 macOS 架构的 release 应用：

```sh
pnpm tauri build
# target/release/bundle/macos/Gitwood.app
```

默认只生成 `app`。Windows/Linux 可用 `pnpm tauri build --no-bundle` 验证编译。
构建 macOS universal DMG：

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pnpm tauri build --target universal-apple-darwin --bundles dmg
```

macOS 使用 ad-hoc 签名，不做公证。后续下载的安装包可能需要在“系统设置 →
隐私与安全性”中选择“仍要打开”。tag 工作流只上传 universal DMG 和 SHA-256
到工作流产物，公开发布 GitHub Release 仍由维护者确认后执行。

## 验证命令

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm -C ui check
pnpm -C ui test
pnpm tauri build
```

Clippy 和 `cargo test` 同时编译桌面适配层。生产构建需要前端资源，
`pnpm tauri build` 会自动先构建前端。

详见[实施计划](docs/implementation-plan.md)、[架构](docs/architecture.md)、
[安全规则](SECURITY.md)和[贡献指南](CONTRIBUTING.md)。最低 Git 版本与 macOS
E2E 方案留待后续确认，当前不包含 GitRunner。

## 许可证

[Apache-2.0](LICENSE)。第三方信息见 [NOTICE](NOTICE) 和
[依赖许可证清单](docs/third-party-licenses.md)。
