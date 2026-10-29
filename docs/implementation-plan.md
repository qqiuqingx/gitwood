# Gitwood：跨平台开源 Git 客户端实施计划

日期：2026-10-29（v3）

## 目标与已确认决策

开发一个开源 Git 客户端，满足三项要求，优先级依次为：

1. **一套界面**，覆盖 macOS、Windows 和 Linux。
2. **交互上 1:1 还原 IntelliJ IDEA 的 Git 工作流**。
3. 在前两项的基础上，**尽量降低资源占用**。

| 决策项 | 结论 |
|---|---|
| 项目名称 | **Gitwood** |
| 本地位置 | `/Users/qqx/job/code/gitwood/`，即仓库根目录 |
| GitHub 仓库 | `qqiuqingx/gitwood`（拟定，首次推送前确认） |
| 应用标识 | `io.github.qqiuqingx.gitwood` |
| 许可证 | Apache-2.0 |
| 签名 | 暂不购买 Apple Developer ID 和 Windows 代码签名证书 |
| 技术栈 | **Rust 核心 + Tauri 2 + TypeScript / Svelte 5 界面**，Diff 视图使用 CodeMirror 6 |
| 平台节奏 | v0.1 只发布 macOS 版本；从第一天起在 CI 中同时构建 Windows 和 Linux；v0.2 起正式发布这两个平台 |
| 最低系统版本 | macOS 13；Windows 10 1809 及以上（使用系统自带的 WebView2）；Linux 为 Ubuntu 22.04 或同等发行版（依赖 webkit2gtk-4.1） |
| 交互参照 | IntelliJ IDEA 2025.3（Build 253.28294.334）、已确认的 `idea-git-interaction-prototype.html`、intellij-community 源码 |
| 外观 | 默认主题还原 IDEA 新界面（Int UI）的 Light 和 Dark；另外内置 *Walnut*，还原本机看到的棕灰暖色外观；支持用户自定义主题 |
| Git 执行 | 调用系统 git 或用户指定路径的 git |
| 隐私 | 不收集任何遥测或使用数据 |

本轮只更新计划。仓库、代码、构建和推送均未开始。

## 技术选型理由

### 为什么选“自绘界面”，而不是原生控件

IDEA 本身不使用系统原生控件，它的树、列表、弹窗、分隔线、快速搜索都是 JetBrains 自己绘制的。要做到 1:1 还原，就必须完全掌控每个控件的行为。原生控件（AppKit、WinUI、GTK）各有自己的默认行为，反而会妨碍还原。因此需要一个“一套代码、完全自己绘制”的方案。

### 候选方案对比

| 方案 | 一套界面 | 还原 IDEA 交互 | 资源占用（估算） | 结论 |
|---|---|---|---|---|
| **Rust + Tauri 2（系统 WebView）** | 可以 | 可以：所有控件自己实现，行为完全可控 | 内存约 120–200 MB（含 WebView 进程）；安装包约 10–20 MB | **采用** |
| Kotlin + Compose Desktop + Jewel | 可以 | 最接近：Jewel 是 JetBrains 官方开源的 IDEA 界面组件库 | JVM 内存约 300 MB 以上；安装包需带 JRE，约 90–120 MB | 不采用：占用约为 Tauri 的 2 倍 |
| Rust + GPUI（Zed 的界面框架） | 可以 | 可以 | 低 | 不采用：没有稳定版本，API 变动频繁，文档少 |
| C# + Avalonia | 可以 | 可以 | 内存约 100–150 MB；安装包约 40–80 MB | 不采用：占用偏大，且已有同类项目 SourceGit |
| Swift + AppKit | 不可以：只能 macOS | 原生控件限制较多 | 最低 | 不采用：不满足“一套界面” |

### 选 Svelte 5 和 CodeMirror 6 的原因

- **Svelte 5**：编译时生成代码，运行时很小，细粒度更新，适合大列表和频繁选择切换；比 React 占用更少内存。
- **CodeMirror 6**：MIT 许可，模块化，可以只渲染可见区域，适合实现 IDEA 风格的双栏 Diff。不使用 Monaco，因为它的体积和内存大得多。

### intellij-community 源码的使用

intellij-community 以 Apache-2.0 开源，其中 `git4idea`、`vcs-log`（提交图布局）、diff 模块的行为可以直接作为 1:1 还原的依据：

- **可以**：阅读源码以确认交互细节；按 Apache-2.0 移植算法（例如提交图泳道布局、按词比较）和图标，并在 NOTICE 中注明来源。每个移植的文件都要逐一核对许可证。
- **不可以**：使用 “IntelliJ”“IDEA”“JetBrains” 名称和 logo 作为产品标识；README 只写“交互参考 IntelliJ IDEA”。
- 字体：界面字体 Inter、代码字体 JetBrains Mono，两者都是 OFL 许可，可以打包。

## 版本范围与优先级

对照清单中每一项都标为 P0、P1 或 P2，按版本交付。

| 版本 | 范围 | 平台 |
|---|---|---|
| **v0.1（首个公开版本）** | P0：打开仓库，查看状态、分支、历史和只读 Diff；文件级与差异块级提交、Amend；分支弹窗、切换和新建分支；Fetch、Pull、Push；三套内置主题 | macOS |
| v0.2 | P1：冲突解决与三方合并、Cherry-pick、Revert、Reset、Stash、标签、单文件历史、Blame；用户自定义主题与热重载 | macOS、Windows、Linux |
| v0.3+ | P2：交互式 Rebase、Shelf/Changelist 对应能力、补丁创建与应用、克隆、远端管理、导入 IntelliJ `.icls` / `.theme.json` 主题 | 三个平台 |

与 Java 代码分析、构建系统相关的 IDE 能力不在本项目范围内。

## 1:1 交互还原

### 还原范围

| 类别 | 需要还原的内容 |
|---|---|
| 布局 | 工具窗口位置、分隔线拖动、最小尺寸、显示与隐藏、尺寸记忆 |
| 控件行为 | 树和列表的单击、双击、多选（Shift / Cmd 或 Ctrl）、方向键展开和折叠、**输入即搜索（speed search）** |
| 弹窗 | 分支弹窗（可搜索、分组、子菜单）、操作弹窗的默认按钮、Esc 取消、焦点返回位置 |
| 快捷键 | 按平台还原 IDEA 默认快捷键：macOS 使用 “macOS” keymap，Windows 和 Linux 使用 “Windows” keymap |
| 右键菜单 | 菜单项、分组、分隔线、启用条件、文字 |
| Diff | 左右双栏、行内按词高亮、中间栏的 `>>` / `<<` 应用箭头、差异跳转（F7 / Shift+F7）、同步滚动、折叠未修改区域 |
| 提交图 | 泳道布局、颜色分配、合并线走法、HEAD 与分支标签位置（移植 vcs-log 的布局算法） |
| 反馈 | 后台任务进度、通知气泡、错误对话框的文字和按钮 |
| 状态恢复 | 操作成功、失败、取消后的选择项、焦点和滚动位置 |

### 对照清单

清单维护在仓库的 `docs/interaction-spec/` 中，按功能分文件。每一项记录：

1. 入口位置、菜单名称、各平台快捷键和启用条件。
2. 单击、双击、右键、键盘导航、多选、焦点切换的行为。
3. 对话框、默认选项、取消行为、成功反馈和失败反馈。
4. 选择项、滚动位置、过滤条件以及布局的恢复方式。
5. 对真实 Git 状态的影响和对应的测试场景。
6. 对应的 intellij-community 源码位置（如有）。
7. 优先级（P0/P1/P2）和目标版本。

状态分为“已实现”“已与 IDEA 对照”“已用真实 Git 验证”。只画出界面或放置无实际功能的入口，不能算该功能完成；未完成的功能不能以成功提示代替实际操作。

### 对照方法

- 在本机 IDEA 和 Gitwood 中对同一个测试仓库执行同一组操作，录屏对比，差异记录在清单中。
- 快捷键和菜单文字以 IDEA 的实际显示为准；源码只用于确认显示中看不出的规则（例如启用条件、默认选项）。
- Windows 和 Linux 的快捷键与行为差异，在 v0.2 前用对应平台的 IDEA 补充对照。

## 可拖动区域

| 分隔位置 | 可调整内容 |
|---|---|
| Commit 与中央 Diff 之间 | Commit 面板宽度 |
| 上方工作区与下方 Git Log 之间 | Git Log 整体高度 |
| 下方分支列表与提交记录之间 | 分支列表宽度 |
| 下方提交记录与右侧信息区之间 | 提交记录、右侧信息区宽度 |
| 右侧变更文件与提交详情之间 | 变更文件、提交详情高度 |
| Diff 左右两个版本之间 | 左右版本宽度比例 |

- 每个区域设置最小尺寸，拖动时不能把相邻区域压到不可操作。
- 窗口缩放时保持可用布局；隐藏后重新显示的面板恢复用户调整的尺寸。
- 窗口大小、位置和分割比例由 Rust 端保存到配置文件，重启后恢复；屏幕尺寸变化时把布局约束在可见范围内。
- 拖动和切换主题只更新界面，不重新读取 Git 历史或计算文件差异。
- 分隔线调整或切换主题后，所选提交、所选文件、提交说明和滚动位置保持不变。
- 首版使用应用级布局记忆，不增加多套布局管理功能。

## 主题系统

### 设计原则

- 界面所有颜色、字体、间距都引用 **CSS 变量**，组件代码中不写死具体值。
- 主题文件是 JSON，由 Rust 端读取和校验后转换成 CSS 变量，注入 WebView。
- 切换主题只更新 CSS 变量，不重新读取 Git 数据，不重建界面。
- 主题文件格式错误或缺少字段时，回退到默认值，并在设置中显示具体错误；应用不能因此崩溃。

### 变量分组（首版）

| 分组 | 示例变量 |
|---|---|
| 基础 | `background.primary`、`background.secondary`、`text.primary`、`text.secondary`、`accent`、`selection.bg`、`selection.inactive.bg`、`separator`、`focus.border` |
| 控件 | `button.*`、`input.*`、`popup.*`、`menu.*`、`tooltip.*`、`scrollbar.*`、`toolbar.*`、`tab.*` |
| Diff | `diff.added.bg`、`diff.added.inline`、`diff.removed.bg`、`diff.removed.inline`、`diff.modified.bg`、`diff.gutter.*` |
| 提交图 | `graph.lanes`（颜色数组）、`graph.head`、`graph.merge` |
| 标签 | `label.branch.local`、`label.branch.remote`、`label.tag`、`label.head` |
| 文件状态 | `status.added`、`status.modified`、`status.deleted`、`status.renamed`、`status.untracked`、`status.conflicted`、`status.ignored` |
| 字体 | `font.ui.family`、`font.ui.size`、`font.editor.family`、`font.editor.size`、`font.editor.lineHeight` |
| 语法高亮 | `syntax.keyword`、`syntax.string`、`syntax.comment`、`syntax.number`、`syntax.type`、`syntax.function` 等 |

### 文件格式与位置

- JSON 格式，带 `schemaVersion`、`name`、`author`、`dark`（是否深色）；可以用 `extends` 继承内置主题，只覆盖需要修改的变量。
- 内置主题打包在应用内：*Light*、*Dark*（还原 IDEA Int UI 的配色）、*Walnut*；另外支持“跟随系统”的浅色 / 深色切换。
- 用户主题的位置由 Tauri 按平台决定：macOS 为 `~/Library/Application Support/Gitwood/Themes/`，Windows 为 `%APPDATA%\Gitwood\Themes\`，Linux 为 `~/.config/gitwood/themes/`。
- JSON Schema 放在 `docs/theme-schema.json`，供编辑器补全和社区贡献主题时使用。
- 因为所有界面都是自绘的，主题能覆盖**全部界面**，包括菜单、弹窗和滚动条。系统级的窗口标题栏和原生菜单栏除外。

### 分版本范围

- **v0.1**：三套内置主题，跟随系统切换。
- **v0.2**：加载用户主题，热重载（只监听主题目录），设置中预览，并提示文字与背景对比度不足。
- **P2**：导入 IntelliJ 的 `.icls` 配色方案和 `.theme.json` 主题，做颜色映射，不保证完全一致。

## 安全与 Git 环境

### 打开不可信仓库

仓库自带的 `.git/config` 可以配置 `core.fsmonitor`、`core.sshCommand`、`diff.external`、textconv 等，打开仓库时的后台读取可能因此执行任意命令。规则如下：

- 后台读取命令统一加 `--no-optional-locks`，禁用 fsmonitor，`diff`、`show`、`log` 加 `--no-ext-diff`、`--no-textconv`。
- 遵守 Git 的 `safe.directory`；被拒绝时向用户说明原因，不自动修改全局配置。
- 用户主动发起的写操作（提交、推送等）保留用户已有的 Git 配置和 hooks 行为。
- 以上规则写入 `SECURITY.md`，并用带恶意配置的仓库编写回归测试。

### WebView 安全

- 界面只加载应用内打包的资源，不加载任何远程页面；配置严格的 CSP。
- 使用 Tauri 2 的权限（capabilities）机制，前端只能调用明确列出的 Rust 命令，不开放通用的 shell 或文件系统接口。
- 提交说明、文件名、分支名等来自仓库的文本一律按纯文本渲染，不插入 HTML。
- 发布版本关闭开发者工具。

### 进程调用

Git 进程用参数数组启动，不把仓库路径、文件名或用户输入拼成 shell 命令。读取与解析在 Rust 的后台任务中执行，结果通过 IPC 发给界面。写操作在应用内串行执行，同时要识别外部 Git 工具并发修改带来的状态变化和锁冲突。

### Git 路径与版本

- 检测顺序：用户指定路径 → 各平台常见路径（macOS 的 Homebrew 路径、Windows 的 Git for Windows 安装路径、`PATH`）→ macOS 的 `/usr/bin/git`。
- macOS 上的 `/usr/bin/git` 在未安装 Command Line Tools 时只是一个占位程序，必须先检测，不能直接调用而意外弹出系统安装窗口。
- Windows 上未安装 Git 时，给出 Git for Windows 的安装指引。
- 最低 Git 版本在阶段 0 根据实际用到的参数确定，版本不足时明确提示。

### 认证

沿用系统 Git 的 credential helper 和 SSH 配置（macOS Keychain、Windows 的 Git Credential Manager、Linux 的 libsecret 等），首版不保存新的明文凭据。

## 关键技术方案

### 部分提交且保留已有暂存内容

1. 以 HEAD 为起点，用 `GIT_INDEX_FILE` 建一个临时 index。
2. 用 `git apply --cached` 把用户选中的文件和差异块写入临时 index。
3. 用临时 index 执行 `git commit`，保证 hooks、签名、`commit.template` 行为不变。
4. 提交成功后，把真实 index 中未参与提交的暂存内容对齐到新 HEAD 上；提交失败时，真实 index 保持不动。

阶段 0 做技术验证；验证通过前，阶段 3 不开始开发。

### 提交图

- 用 `git log --format='%H %P' --topo-order` 一次性读取全部提交关系，在 Rust 中计算全部泳道，算法移植自 intellij-community 的 vcs-log 模块。
- 提交说明、作者、日期按可见范围分页读取。
- 界面只拿到可见行的泳道数据，用 Canvas 绘制；滚动时向 Rust 请求新的可见范围。

### 控制资源占用

Tauri 方案的主要内存开销来自 WebView 中的 JS 堆和 DOM，因此：

- **数据留在 Rust 端**。历史、文件列表、Diff 内容不整体传给界面，只传可见窗口的数据，用 ID 引用。
- **所有长列表使用虚拟滚动**，包括提交记录、变更文件、分支列表和 Diff。
- 提交图用 Canvas 绘制，不为每个节点创建 DOM。
- CodeMirror 只加载当前文件，限制缓存大小；切换仓库后释放旧数据。
- 只有一个窗口和一个 WebView，不为弹窗开新窗口。
- 空闲时不轮询、不运行持续动画；仓库变化通过文件系统事件触发刷新（Rust `notify` crate），过滤 `.git/objects` 等无关变化，并做防抖；应用重新激活时补做一次检查。
- IPC 传输使用紧凑格式，避免一次传大量数据。

### 语法高亮

阶段 2 比较 CodeMirror 自带的 Lezer 语言包与在 Rust 端用 tree-sitter 高亮两种方式，比较内存、包体积和覆盖语言数量。两种方式都要映射到主题的 `syntax.*` 变量。

## 仓库结构与构建

```
gitwood/
├── LICENSE / NOTICE              Apache-2.0；NOTICE 登记移植自 intellij-community 的代码和图标
├── README.md / README.zh-CN.md
├── CONTRIBUTING.md / CODE_OF_CONDUCT.md / SECURITY.md / CHANGELOG.md
├── .github/
│   ├── workflows/ci.yml          macOS、Windows、Linux 三平台：cargo test、前端测试、构建
│   ├── workflows/release.yml     打 tag 后构建三平台安装包，上传 Release（v0.1 只上传 macOS）
│   ├── ISSUE_TEMPLATE/
│   └── pull_request_template.md
├── Cargo.toml                    Rust workspace
├── crates/
│   └── gitwood-core/             不依赖 Tauri 的纯逻辑库
│       ├── src/git/              GitRunner、Git 路径与版本检测、安全参数
│       ├── src/repo/             仓库识别、状态解析、刷新、路径处理、文件监听
│       ├── src/history/          提交关系、泳道布局、分页详情、过滤
│       ├── src/diff/             差异解析、版本内容读取、左右行对应、按词比较
│       ├── src/ops/              提交（临时 index）、分支、同步、高级操作
│       ├── src/theme/            主题解析、继承、校验、回退
│       └── tests/                在临时真实仓库中做 Git 行为回归测试
├── src-tauri/                    Tauri 应用：窗口、原生菜单、IPC 命令、权限配置、布局与设置保存
│   ├── capabilities/
│   ├── icons/                    自有应用图标
│   └── tauri.conf.json
├── ui/                           Svelte 5 + TypeScript
│   ├── src/components/           还原 IDEA 的基础控件：Tree、List、SpeedSearch、Popup、Splitter、Toolbar、Tabs
│   ├── src/features/             commit / diff / log / branches / settings
│   ├── src/keymap/               macOS 与 Windows 两套 keymap
│   ├── src/theme/                CSS 变量和内置主题
│   ├── src/i18n/                 简体中文、英文
│   └── tests/                    组件单测（Vitest）
├── e2e/                          端到端测试
├── docs/
│   ├── interaction-spec/
│   ├── theme-schema.json
│   ├── themes.md
│   ├── architecture.md
│   └── perf/
└── scripts/
    └── make-fixture-repos.*      生成测试与性能基准用仓库
```

- 开发环境：Rust stable（rustup）、Node LTS、pnpm。本机目前已有 Node 25.6.1，**未安装 Rust 和 pnpm**，阶段 0 安装。
- 外部依赖只接受与 Apache-2.0 兼容的许可证，并在 NOTICE 中登记。
- 界面文字全部走 i18n，首版提供简体中文和英文；菜单与按钮的英文文字与 IDEA 一致。
- 无障碍：自绘控件提供 ARIA 角色和键盘操作，让系统读屏软件可以使用；列入 P1。
- 测试：Rust 核心用真实临时 Git 仓库测试；界面组件用 Vitest 测试；关键流程用端到端测试（WebDriver，Tauri 官方支持 Windows 和 Linux，macOS 端在阶段 0 确认可行方案）。
- 测试走生产操作路径，不向生产代码加入仅供测试使用的开关或接口。

## 分发（无签名证书）

| 平台 | 安装包 | 首次打开 |
|---|---|---|
| macOS | Universal `.dmg`，ad-hoc 签名，不公证 | 在“系统设置 → 隐私与安全性”中选择“仍要打开”，或执行 `xattr -dr com.apple.quarantine /Applications/Gitwood.app` |
| Windows（v0.2 起） | `.msi` 和安装版 `.exe` | SmartScreen 会提示“未知发布者”，选择“更多信息 → 仍要运行” |
| Linux（v0.2 起） | `.AppImage`、`.deb` | 无额外限制 |

- 每个安装包附 SHA-256 校验值；README 同时说明从源码构建的方法。
- 暂不启用自动更新，不上架应用商店或包管理器官方仓库。
- 购买签名证书后，再补 macOS 公证和 Windows 签名流程，这两项单独记录状态。
- 只有在用户确认后，才会推送到 GitHub 或创建 Release。

## 实施阶段

阶段表是唯一进度来源。

| 阶段 | 交付结果 | 前置条件 | 验收标准 | 状态 |
|---|---|---|---|---|
| 0. 开源骨架与技术验证 | 工具链安装；仓库骨架、许可证、社区文件；三平台 CI；GitRunner 与安全参数；临时 index 部分提交验证；提交图布局验证；**空壳应用的内存与体积基线** | 本计划确认 | CI 三平台通过；部分提交验证覆盖“暂存 A 只提交 B”；5 万提交的布局耗时与内存有数据；空壳应用占用有实测数据 | 待执行 |
| 1. 界面骨架、基础控件与主题 | 还原 IDEA 的 Tree、List、SpeedSearch、Popup、Splitter；六处分隔线；keymap 框架；三套内置主题；P0 交互清单 | 阶段 0 | 各区域可拖动，重启后恢复；基础控件的键盘和鼠标行为与清单一致；切换主题不丢失状态 | 待执行 |
| 2. 真实仓库浏览闭环 | 打开仓库、状态、分支、历史、文件、只读 Diff | 阶段 1 | 完成“分支 → 提交 → 文件 → Diff”联动；恶意配置仓库不执行命令；大仓库滚动流畅 | 待执行 |
| 3. 变更与提交闭环 | 文件和差异块选择、提交、Amend、撤销 | 阶段 2 | 只提交选中内容，保留无关的工作区内容和已有暂存内容 | 待执行 |
| 4. 分支、远程同步 → **v0.1 发布（macOS）** | 分支弹窗、切换与新建、Fetch、Pull、Push；README 与发布流程 | 阶段 3 | 用本地测试远端完成同步，结果与 Git 一致；性能报告完成；Release 可下载并按说明打开 | 待执行 |
| 5a. P1 能力 + Windows / Linux → v0.2 | 冲突与三方合并、Cherry-pick、Revert、Reset、Stash、标签、文件历史、Blame、用户主题；Windows 和 Linux 的对照与安装包 | v0.1 | 按清单核对三平台的功能、快捷键、菜单状态和失败路径 | 待执行 |
| 5b. P2 能力 → v0.3+ | 交互式 Rebase、Shelf/Changelist、补丁、克隆、远端管理、主题导入 | v0.2 | 同上 | 待执行 |
| 6. 性能与稳定性（每次发布前执行） | 三平台资源测量报告、交互回归 | 当次版本功能完成 | 提供实测数据，记录未通过条目 | 待执行 |

### 阶段 0：开源骨架与技术验证

1. 安装 Rust（rustup）和 pnpm。
2. 在 `/Users/qqx/job/code/gitwood/` 执行 `git init`，按上面的结构创建骨架，写好 LICENSE、NOTICE 和各社区文件。
3. 初始化 Rust workspace、Tauri 2 应用和 Svelte 5 界面，打通 `pnpm tauri dev` 和 `pnpm tauri build`。
4. 配置 GitHub Actions，三个平台都运行 `cargo test`、前端测试和构建；确认含换行、制表符等特殊文件名的测试仓库在三个平台上的行为（Windows 不允许部分字符，测试需按平台区分）。
5. 实现 GitRunner：参数数组调用、取消、退出码、标准错误输出、安全参数、Git 路径与版本检测。
6. 技术验证：
   - 临时 index 部分提交，包括 hook 拒绝和签名失败两种情况；
   - 移植 vcs-log 泳道布局，测量 5 万个提交的耗时与内存；
   - 空壳应用（一个窗口加六处分隔线）在 macOS 上的空闲内存、空闲 CPU 和安装包体积，作为后续优化的基线。
7. 确定最低 Git 版本和端到端测试方案，写入 README 和 `docs/architecture.md`。

如果空壳应用基线明显超出下面的性能目标，先停下来向用户报告，再决定是否调整方案。本阶段只做本地提交；推送到 GitHub 前先确认仓库名和公开时间。

### 阶段 1：界面骨架、基础控件与主题

1. 对照本机 IDEA，在 `docs/interaction-spec/` 中建立 P0 条目，P1 和 P2 只列条目名称；同时阅读 intellij-community 中对应的源码，记录规则。
2. 对照提交面板的文件选择、预览开关、暂存区模式、Changelist/Shelf 配置，明确 IDEA 当前的实际工作方式。
3. 实现还原 IDEA 行为的基础控件：Tree（含展开、折叠、多选）、List、SpeedSearch、Popup、Splitter、Toolbar、Tabs，并编写组件测试。
4. 按已确认的原型搭建窗口和各区域边界，实现六处分隔线、最小尺寸、面板显隐和布局记忆。
5. 实现 keymap 框架，先接入 macOS keymap。
6. 实现主题解析、继承、回退、三套内置主题和跟随系统切换。

验证：手动拖动各处分隔线、缩放窗口、隐藏和恢复面板、切换主题，退出后重开，检查布局、焦点和内容状态；基础控件逐项与 IDEA 录屏对比。

### 阶段 2：连接真实仓库，先完成只读闭环

1. 支持打开本地仓库、记住最近仓库、切换当前仓库；识别普通仓库与 Git worktree；处理 `safe.directory` 拒绝访问的情况。
2. 用 `--porcelain=v2 -z` 解析文件状态，覆盖中文、空格、制表符和换行文件名，以及重命名、删除、新增和未跟踪文件。
3. 显示本地和远程分支、当前 HEAD、标签和跟踪关系。
4. 按“全量提交关系 + 分页详情 + Canvas 绘制”的方案实现提交图，滚动时保持图线连续和选择位置。
5. 实现按说明 / hash、分支、作者、日期、路径过滤，以及提交、文件、详情三者联动。
6. 基于 CodeMirror 6 实现 IDEA 风格的双栏 Diff，覆盖插入、删除、修改、空文件、无末尾换行、二进制和无法按文本解码的文件。
7. 实现差异跳转、同步滚动、行号、按词高亮、折叠未修改区域和左右宽度调整；确定语法高亮方案。
8. 接入文件监听，合并事件并按区域刷新；切换仓库或选择项时取消并丢弃过期结果。

验证：在临时仓库中制造分支、合并、重命名和特殊文件名，逐项与 Git 命令结果比较；用恶意配置仓库验证不执行命令；用用户选择的实际仓库做只读浏览验证；在基准大仓库中测量滚动和切换的响应。

### 阶段 3：完成变更和提交

1. 实现 Changes、Unversioned Files 分组和文件勾选，与本机 IDEA 的单击、双击、键盘和焦点行为对照。
2. 按阶段 0 验证过的临时 index 方案实现差异块选择和部分提交，区分“本次选中的内容”和 Git 中已有的暂存内容。
3. 接入提交说明、历史提交说明、Amend、Commit、Commit and Push；推送部分在阶段 4 完成闭环。
4. 实现撤销变更及其确认流程；确认范围、执行结果和失败反馈要与选中内容一致。
5. 检查提交失败、hook 拒绝、签名失败、仓库锁冲突和外部工具修改仓库时的状态恢复。

验证必须覆盖：

- 已暂存 A、工作区修改 B，只选择 B：提交结果只包含 B，A 仍保持暂存。
- 同一文件同时有暂存和未暂存修改：选择部分差异提交后，提交内容与选择一致，其余内容保留。
- 删除、重命名、未跟踪文件和特殊文件名的提交结果正确。
- hook 拒绝和签名失败后，变更、暂存内容和提交说明都可恢复，界面不显示提交成功。
- 用户原有的 Git 配置和 hooks 行为得到保留。

### 阶段 4：分支、远程同步与 v0.1 发布

1. 实现与 IDEA 一致的分支弹窗：搜索、分组、子菜单和键盘操作，核对各菜单项的启用条件。
2. 接入新建、切换、重命名、删除分支和跟踪远程分支。
3. 接入 Fetch、Pull/Update 和 Push，显示远端、目标分支、待推送提交和执行结果。
4. 对照 IDEA 的合并 / 变基选择和本地修改处理方式；冲突时 v0.1 给出明确提示并引导中止，完整的冲突解决放在 v0.2。
5. 验证认证、网络失败、推送被拒、缺少上游分支和用户取消认证时的反馈。
6. 完成 README（中英文，含截图、安装方法、首次打开说明、从源码构建）、CHANGELOG 和 release workflow；执行阶段 6 的性能测量。

验证：用临时 bare 仓库作远端，两个临时工作副本制造分叉和推送被拒。开发验证期间不对用户真实的远端执行写操作。发布 v0.1 前需用户确认。

### 阶段 5a / 5b：按版本补齐能力和平台

以 `docs/interaction-spec/` 中的 P1、P2 条目为依据，逐项细化后实现。至少核对：

- Cherry-pick、Revert、Reset，以及各自的确认、冲突和取消流程。
- Merge、Rebase、冲突文件列表、三方合并、继续 / 中止，以及 IDEA 的交互式历史操作。
- 标签、从提交新建分支、检出历史版本、历史版本与工作区比较。
- 单文件历史、Blame、补丁创建 / 应用、Stash，以及 Shelf/Changelist 的对应能力。
- 仓库克隆、远端管理。
- 提交图多选、范围选择、右键菜单、组合过滤、搜索和快捷键。
- 用户主题加载、热重载、预览、对比度提示，以及 IntelliJ 主题导入。
- Windows 和 Linux：Windows keymap、路径与换行符（CRLF、`core.autocrlf`）、大小写不敏感的文件系统、长路径、凭据管理器、高 DPI 缩放、WebKitGTK 的渲染差异。

Shelf/Changelist 与 Git Stash 的语义要分别对照验证，不能用相近的名称掩盖行为差异。

### 阶段 6：低占用、稳定性与性能测量

实现期间持续遵循“控制资源占用”一节的规则；性能优化后重新验证交互与 Git 行为，不能为了降低占用而丢失功能。

以下是初始工程目标，尚无实测结果，阶段 0 的空壳基线测出后可能调整：

| 指标 | 初始目标与测量条件 |
|---|---|
| 安装包大小 | macOS Universal 包不超过 20 MiB；Windows / Linux 安装包不超过 15 MiB |
| 空闲 CPU | 仓库无变化、无用户操作时连续观察 60 秒，所有相关进程合计平均低于 1% |
| 常用场景内存 | 打开一个普通仓库，加载一页提交和一个普通文本 Diff：**应用进程加 WebView 相关进程合计**不超过 200 MiB |
| 长时间浏览 | 多次翻页、切换文件和仓库后，内存有上限，数据释放后没有持续上升的趋势 |
| 大仓库响应 | 用至少 5 万提交、1 万文件的基准仓库测量；提交关系一次性加载，详细信息和文件列表不一次性全部加载；滚动保持流畅 |

报告记录各进程的内存统计口径、Git 子进程峰值、冷启动 / 热启动情况、基准仓库规模和测量时长，按平台存放在 `docs/perf/`。实际数据偏离目标时，先记录瓶颈和数据，再决定优化哪一项；不能把设计目标写成测量结果。

签名与公证（暂不执行）、各平台的最低系统版本兼容性、Intel Mac 兼容性分别记录验证状态；本机运行通过不代表这些项通过。

## 交付记录与下一步

2026-10-29：

- 已读取本机 IDEA 版本和界面，已交付交互原型并得到用户认可。
- 用户补充：下方各区域也需要可以拖动调整大小。
- v1：生成 Swift + AppKit 实施计划。
- v2：用户确认项目放在 `/Users/qqx/job/code/` 下、使用 Apache-2.0 许可证、暂不购买 Developer ID；改名为 Gitwood，新增开源、主题、安全、技术方案、仓库结构、分发等章节，新增阶段 0。
- v3：用户确认以后要支持 Windows 和 Linux，优先级依次为“一套界面”“交互 1:1 还原 IDEA”“占用小”。技术栈改为 Rust + Tauri 2 + Svelte 5 + CodeMirror 6；1:1 交互还原取代“不做像素级复刻”；主题覆盖范围扩大到全部界面；性能目标改为按“应用进程加 WebView 进程合计”统计；阶段 0 增加空壳应用的资源基线测量。
- 仍未创建仓库，未编写代码，未运行测试或性能测量。
- 下一步从阶段 0 开始，需要用户给出实施指令。GitHub 仓库名 `qqiuqingx/gitwood` 和公开时间在首次推送前确认。
