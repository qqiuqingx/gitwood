# Gitwood 项目约定

## 范围与架构

- 先完整阅读 `docs/implementation-plan.md`；严格执行当前用户授权的阶段和步骤。
- `crates/gitwood-core/`：不依赖 Tauri 的纯 Rust 逻辑；`git/` 执行与安全参数，
  `repo/` 状态与刷新，`history/` 历史与图布局，`diff/` 差异，`ops/` 写操作，
  `theme/` 主题解析。尚未实现的模块只保留声明和文档，不填假实现。
- `src-tauri/`：窗口、原生菜单、IPC、capabilities、应用设置和布局持久化适配。
- `ui/`：Svelte 5 + TypeScript + Vite（非 SvelteKit）；基础组件放 `components/`，
  功能放 `features/`，文案走 `i18n/`，主题变量放 `theme/`。
- 大数据留在 Rust，只向 UI 传可见范围与 ID；长列表虚拟滚动，提交图用 Canvas。
  不增加空闲轮询、持续动画、额外 WebView。当前不实现布局持久化。
- 不增加测试专用接口、构造器、重载、开关、分支或假成功提示。优先复用现有实现；
  新依赖先核对实际版本与 Apache-2.0 兼容性并更新 NOTICE、许可证清单和锁文件。

## 构建与测试（仓库根目录）

```sh
source ~/.cargo/env # 必要时
pnpm install --frozen-lockfile
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm -C ui check
pnpm -C ui test
pnpm tauri build
```

- `pnpm tauri dev` 为开发常驻进程，只在用户允许启动时使用。
- 默认 macOS 构建输出 `target/release/bundle/macos/Gitwood.app`；CI 用
  `pnpm tauri build --debug --no-bundle` 检查三平台编译，tag 工作流另做 release DMG。
- Rust stable、edition 2021、pnpm 12.16.1；建议 Node 24 LTS，支持本机 Node 25.6.1。
- 修改前检查相关代码、配置、类型及实际依赖版本；不确定的用法查对应版本官方文档。
- 按影响执行必要验证；Git 行为用真实临时仓库，UI 约束用 Vitest；不将跳过或未执行
  的检查写成通过。仅注释/文档变更可用轻量检查，无需重新运行构建测试。

## 安全边界

- Git 必须以参数数组启动；不得把路径、文件名或用户输入拼成 shell 命令。
- 后台读取统一 `--no-optional-locks -c core.fsmonitor=false`；`diff/show/log` 加
  `--no-ext-diff --no-textconv`；尊重 `safe.directory`，不自动改全局安全设置。
- 主动写操作保留用户 Git 配置与 hooks，应用内串行并识别外部并发修改及锁冲突。
- 仓库文本一律按纯文本渲染；禁止 `{@html}` 或 `innerHTML` 渲染仓库内容。
- 只加载本地资源，保持严格生产 CSP、显式命令 ACL、最小 capabilities，禁用发布版
  devtools；不开放通用 shell/fs API。详见 `SECURITY.md`。
- 不收集遥测，不保存新明文凭据；不使用第三方名称或 logo 作产品标识。

## 工作区与交付

- 保留无关工作区/暂存改动；用户仅要求分析或 Review 时只读，不自动生成文档。
- 实施前简述范围与影响；明确的小改动直接执行，无需重复确认。
- 仅用户明确要求时 commit/push/发布/建远程仓库；提交只包含当前任务内容。
- Commit 使用中文；已提供 Jira 编号时以 `编号 中文描述` 为格式，不自行补编号。
- 汇报列明实际执行的检查、平台范围、未验证事项及产物，不提前宣称阶段整体完成。
