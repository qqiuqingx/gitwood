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
  `git`, partial-commit `ops`, and the stage 0 `history` relationship graph and
  viewport geometry are implemented; the other modules remain documentation-only,
  with no fake services.
- `src-tauri`: one native window and WebView, native menus, IPC input validation,
  task coordination and persistence adapters. Commands `app_version`, `get_theme`, `get_settings`, `save_layout`, and `finish_exit`
  are explicitly registered in the command manifest and capability. Native View
  menu events change the theme and panel visibility, then emit preferences.
- `ui`: rendering, interaction, keymap and i18n. It cannot launch processes or
  access the filesystem. Six instances of the reusable Splitter form the shell.
  Colors, fonts and spacing come from the validated bundled theme map;
  `src/theme/light.css` supplies the same Light tokens before IPC initialization.
  System fonts are used and no fonts are bundled. See [themes.md](themes.md).

## Data and rendering

Keep repository data in Rust. Send bounded visible ranges and stable IDs through
IPC; never copy whole histories, file trees or diffs into the JS heap. Long lists
(commits, branches, changed files, diff) use virtual scrolling. Render graph lanes
on Canvas; only the current file is loaded in the future CodeMirror adapter.
Bound caches and release old repository data on switching.

## 提交图

阶段 0 第 6 步只完成 Rust 关系图、布局、可见行绘制元素和基准，不接入界面或 IPC。
实现位于 `crates/gitwood-core/src/history/`；实测见
[phase0-graph-layout.md](perf/phase0-graph-layout.md)。

### 源码依据与许可证

2026-10-29 按以下命令在仓库外阅读源码，sparse 列表只有 `platform/vcs-log`：

```sh
git clone --depth 1 --filter=blob:none --sparse \
  https://github.com/JetBrains/intellij-community.git /tmp/intellij-community
git -C /tmp/intellij-community sparse-checkout set platform/vcs-log
git -C /tmp/intellij-community rev-parse HEAD
```

固定参考 revision：[`0698a5fe297dfa08069f015cc423a6a85ed7fc60`](https://github.com/JetBrains/intellij-community/tree/0698a5fe297dfa08069f015cc423a6a85ed7fc60/platform/vcs-log)
（该次浅克隆的 master，2026-10-29），不是计划中的 IDEA 2025.3 tag。
下表每个路径均相对于 `platform/vcs-log/`，全部指向这个 revision：

| 参考文件 | 用途 |
| --- | --- |
| `graph/src/com/intellij/vcs/log/graph/impl/permanent/PermanentLinearGraphBuilder.java` | 拓扑顺序、紧凑节点 ID、父子邻接、未加载父节点 |
| `graph/src/com/intellij/vcs/log/graph/impl/permanent/PermanentLinearGraphImpl.java` | 普通边及相邻单父节点的压缩表示 |
| `graph/src/com/intellij/vcs/log/graph/impl/permanent/DuplicateParentFixer.java` | 保序删除重复父节点 |
| `graph/src/com/intellij/vcs/log/graph/impl/permanent/GraphLayoutBuilder.kt` | head 顺序、迭代 DFS、layout index 分支链 |
| `graph/src/com/intellij/vcs/log/graph/impl/permanent/GraphLayoutImpl.kt` | 根据 layout index 找链归属的 head |
| `graph/src/com/intellij/vcs/log/graph/utils/DfsUtil.kt` | 显式栈遍历，避免递归深度限制 |
| `graph/src/com/intellij/vcs/log/graph/impl/facade/PermanentGraphImpl.kt` | permanent graph、分支 head 与布局的组装边界 |
| `graph/src/com/intellij/vcs/log/graph/impl/print/GraphElementComparatorByLayoutIndex.java` | 行内节点与穿行边的左右排序、同链平局规则 |
| `graph/src/com/intellij/vcs/log/graph/impl/print/PrintElementGeneratorImpl.kt` | 本行到邻行位置的上下半边、长边隐藏阈值 |
| `graph/src/com/intellij/vcs/log/graph/impl/print/EdgesInRowGenerator.java` | 行内穿行边查询、分块邻域缓存 |
| `graph/src/com/intellij/vcs/log/graph/impl/print/GraphColorGetterByHead.kt` | head 与分支片段共同决定颜色 |
| `graph/src/com/intellij/vcs/log/graph/impl/facade/PrintElementPresentationManagerImpl.kt` | 边沿用两端较大 layout index 所属链的颜色 |
| `graph-api/src/com/intellij/vcs/log/graph/GraphColorManager.java` | head/fragment 颜色契约 |
| `impl/src/com/intellij/vcs/log/graph/GraphColorManagerImpl.kt` | 主链用代表 ref 的 Java String.hashCode，无 ref 为 0；侧链用 fragment index |
| `impl/src/com/intellij/vcs/log/graph/HeadCommitsComparator.java` | ref manager 提供 head 重要性顺序 |
| `impl/src/com/intellij/vcs/log/graph/DefaultColorGenerator.kt` | IDEA 根据 color ID 生成 RGB；本项目不移植这一层 |
| `graph/test/com/intellij/vcs/log/graph/impl/permanent/GraphLayoutBuilderHeadOrderTest.kt` | 非首行 head、内部 head 的优先级断言 |
| `graph/test/com/intellij/vcs/log/graph/impl/permanent/GraphLayoutBuilderTest.java` | layout index 测试方法 |
| `graph/test/com/intellij/vcs/log/graph/impl/print/PrintElementGeneratorTest.kt` | 列位置、边段与长边阈值测试方法 |
| `graph/testData/elementGenerator/manyNodes_in.txt`、`manyNodes_out.txt` | 已转为 Rust 测试的行列几何夹具 |
| `graph/testData/layoutBuilder/manyNodes_in.txt`、`manyNodes_out.txt`、`headsOrder_in.txt`、`headsOrder_out.txt` | DFS 与 head 顺序的人工交叉核对 |

上述 Java/Kotlin 文件头逐一确认是 Apache-2.0，保留 JetBrains 的版权归属；文本夹具
没有独立文件头，随对应 Apache-2.0 测试使用。移植文件头及根目录 `NOTICE` 登记了
来源、revision、原版权和修改点。源码根 `NOTICE.txt` 的 IntelliJ IDEA 归属声明也已保留；
根 `LICENSE.txt` 当前是 Open-Source Build 产品条款，本次移植依据各源码文件的明确
Apache-2.0 声明，不打包或使用 JetBrains 产品、标识、图标。未引入依赖。

### 数据与算法

1. `CommitGraph::parse_log` 解析 `git log --format=%H %P --topo-order`。SHA-1/SHA-256
   二进制 hash 各占一个 32 字节槽，位于唯一数组中；节点和边 ID、CSR 邻接偏移均为
   `u32`。解析期 hash map 随解析结束释放，不在每个节点保存 String。
   父节点保持 Git 的顺序，重复父节点保序去重。重复提交、混合 hash 宽度、非拓扑顺序、
   缺失父节点均报错，不以假根掩盖坏输入。Git 浅仓库的真实边界按 Git 输出作为根。
2. `GraphLayout::new(graph, heads)` 接收按重要性排序的显式 `Head`（可含内部节点），
   然后补上未列出的 DAG tips，按日志行号排序。HEAD 必须由实际 hash 解析成 ID，
   无任何“HEAD 就是第 0 行”的假设。迭代 DFS 按父节点顺序走未分配节点；同一条链
   使用同一 layout index，只有首次访问且没有未分配父节点时才递增，已分配节点跳过。
   `layout index` 是永久分支链编号，不是屏幕泳道列。
3. 每行只由本行节点及满足 `child < row < parent` 的穿行边组成，按上游 comparator
   排序：比较边的两端最大 layout index 与节点 index；平局比较 child 行号；两条边
   按上游的共同起点/较近起点规则比较。排序后的位置才是零起始屏幕列。
   普通、合并和八爪鱼边都通过同一规则连接到邻行的节点或穿行边位置。
4. node color ID：head 的主链使用代表 ref 的 Java UTF-16 `String.hashCode()`，无 ref
   为 0；其余 DFS fragment 使用一开始的 layout index。edge color ID 使用两端较大
   layout index 所属节点的颜色。`ColorId::palette_index(NonZeroU32)` 输出
   `unsigned_abs(color_id) % graph.lanes.len()`，颜色长度变化无需重新布局。
5. `visible_rows(start, end)` 返回半开区间，越界 end 截断、反向/空范围返回空；结果为
   扁平 `rows + segments`，无 hash、字符串或隐藏行。每段含 edge ID、本行列、邻行列、
   Up/Down 和 color ID。Canvas 从本行中心画到两列均值及行边界（上下半行），两行的
   半边在边界准确相接；节点最后覆盖在线上。只借邻行计算端点，邻行不进入返回值。

### 与上游的差异和当前边界

- 永久图采用完整 CSR 边数组，未移植上游相邻单父节点隐式边压缩、负数 not-loaded
  节点及图筛选/折叠控制器。要求完整闭合 DAG；分页只用于详情，不用于关系图。
- 上游 `EdgesInRowGenerator` 使用有限范围 walk 和 SLRU 行缓存。本项目用按 child
  排序的边数组加 max-parent 区间树，跳转任意行都能查全长边，常驻空间为 `O(V+E)`，
  不随长边跨越行数膨胀。只保存全图永久 index/颜色/邻接/区间索引；屏幕列按需计算。
  每次查询仅保留当前及相邻共三行临时位置；返回量仍取决于可见行中实际边数。
- 上游默认会隐藏 >=30 行的长边中段，并画箭头；showLongEdges 模式也有 >=1000 行
  的折叠阈值。本阶段完整保留所有普通长边，无箭头、虚线、特殊/选择/悬停元素。
  短普通边的几何与所移植源码一致；长边图形和宽度因此可能与 IDEA 默认显示不同。
- 无 ref manager：调用方提供 `Head` 顺序及代表 ref；未提供时使用 DAG tips 行顺序，
  无 ref 的主链为默认 color ID 0。尚未实现 IDEA 对分支类型/仓库/引用的完整排序策略。
- 移植 head/fragment color ID，不移植 IDEA 的 HSB/RGB 生成或固定调色板；本项目
  根据主题 `graph.lanes` 取模，同色碰撞是有限主题色数组的预期结果。
- `read.rs` 改为 async，接收共享 `GitRunner` 和 `RunOptions`，通过
  `ReadCommand + ReadOperation::Log` 读取，复用统一安全参数、filter 封堵、超时、取消、
  safe.directory 错误与退出码/stderr；不另建同步进程路径。
  保留 `--all HEAD`（含分离 HEAD）、无 notes/颜色/装饰的关系输出和空仓库返回 Git 错误
  的语义；Git 发现及最低版本由调用方创建 GitRunner 时检查。
  快照并发刷新与空仓库 UX 仍待仓库层接入。
- 测试包含上游夹具列位置、head 顺序、随机 DAG 的全边连续性与跨 viewport 一致性，
  以及真实仓库的分离 HEAD、浅边界和恶意读取配置。尚未和运行中的 IDEA Canvas
  做截图/交互对照，不能据此声称 IDEA 2025.3 的完整像素或跨平台行为已验证。

Use filesystem events, filtering and debounce rather than idle polling. Recheck
on activation; cancel stale reads and discard outdated responses. Serialize writes
and account for external tools changing a repository. Resizing and theme changes
must not trigger Git reads, reconstruct data or reset selection/focus/scroll.

## Shell layout

The upper workspace splits Commit / Diff; Diff splits left / right. The lower Git
Log splits Branches / (Commits / Information), with Information splitting Changed
Files / Commit Details. The sixth divider separates the workspace / entire log.
There are no repository entries, fake actions or background Git reads.

Splitter uses pointer capture, cancellation and keyboard arrows (Shift for larger
steps, Home/End for bounds). ResizeObserver updates available space. Both panes
have minimum sizes; if an externally constrained host cannot fit both, space is
shared proportionally without negative tracks or overflow. Ratios are bound to application layout settings. Hidden panes stay mounted with
HTML `hidden`; their ratios and descendants remain intact. Restoring or shrinking
uses the same `constrainSplit` minimum-size rules. Native menu shortcuts are
Cmd+0 (Commit) and Cmd+9 (Git) on macOS, confirmed in
`platform/platform-resources/src/keymaps/Mac OS X 10.5+.xml`, lines 227–232
at revision `0698a5fe297dfa08069f015cc423a6a85ed7fc60`. Other platforms use
Alt+0 / Alt+9, confirmed in `$default.xml`, lines 947–952 at the same revision;
native runtime verification there remains outstanding.

## Application settings (stage 1 steps 4 and 6)

Rust owns `app.path().app_config_dir()/settings.json` (Tauri 2 path API). For the
current bundle identifier, macOS resolves this to
`~/Library/Application Support/io.github.qqiuqingx.gitwood/settings.json`;
Windows uses `%APPDATA%\io.github.qqiuqingx.gitwood\settings.json`, Linux uses
`~/.config/io.github.qqiuqingx.gitwood/settings.json` (XDG_CONFIG_HOME respected).
This settings path is distinct from the future user-theme directories.

```json
{
  "schemaVersion": 1,
  "theme": "system",
  "layout": {
    "ratios": {
      "workspaceLog": 0.59, "commitDiff": 0.22, "diffVersions": 0.5,
      "branchesCommits": 0.18, "commitsInformation": 0.7, "filesDetails": 0.6
    },
    "commitVisible": true,
    "logVisible": true
  },
  "window": { "x": 100, "y": 100, "width": 1440, "height": 900 }
}
```

`window` is nullable and stores physical **outer** bounds. The adapter subtracts
native decorations when restoring the inner size, and adapts the logical 900×600
minimum to the chosen monitor scale. Work-area constraints account for taskbars,
docks, negative monitor origins, removed displays and work areas smaller than the
usual minimum. Native resize/move events detect changed work areas; focus and
scale changes also recheck. No display polling is introduced. Maximized,
fullscreen and minimized bounds do not overwrite the last normal geometry.

UI ratio updates enter Rust immediately. A sleeping worker writes only after
250 ms without changes, synchronizes a sibling temporary file, then renames it
over the JSON. Native close and application exit first request a final UI ratio snapshot and
drain pending ratio IPC, then synchronously flush dirty state before exiting.
A write failure keeps the app open with an error. Before UI initialization,
close/exit directly flushes the backend state.
Malformed/missing/unsupported config falls back to defaults; load diagnostics and
background write failures appear in the shell. Six ratios are independently
validated; per-pane minimums are applied when rendering, preserving desired ratios
for later expansion. Visibility and theme changes originate in the native menu;
menu text comes from the frontend i18n catalog. Theme `system` uses frontend
`prefers-color-scheme` change events, fetching only the effective Light/Dark map.

Local unit checks and native compilation do not establish window, menu, focus,
CSSOM rendering or restart behavior in a running app. No real application was
launched for this change; manual IDEA comparison and multi-monitor acceptance
remain to be performed. These steps do not complete stage 1 as a whole.

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

## GitRunner and minimum Git version

`gitwood-core::git` uses **Tokio process APIs** with argument arrays, never a shell.
The desktop adapter must call it from a Tokio runtime with I/O and time enabled.
Stdin, stdout and stderr are drained concurrently, including large binary input;
stdout stays `Vec<u8>`, stderr uses lossy text decoding. `RunOptions` carries cwd,
extra environment, stdin, an optional explicit timeout, and a `CancellationToken`.
`timeout: None` selects the policy default: **reads have 30 seconds per Git
subprocess; writes have no deadline**, so hooks, signing/pinentry and future
fetch/push can finish. `Some(duration)` overrides either policy. Discovery and
post-commit reconciliation remain bounded to 30 seconds. The filter query and
actual read are separate subprocesses, each with that limit.

On Unix, every Git child starts in its own process group (`process_group(0)`).
Cancellation, timeout or I/O failure sends the group SIGTERM, allows 200 ms for
cleanup, then sends SIGKILL to the group and kills/reaps the Git child. This also
reaches hook/signer descendants that ignore SIGTERM. A helper that deliberately
creates a new session/group can escape; arbitrary helper side effects are not
rolled back. **Windows currently kills/reaps only Git: no Job Object is
implemented, so descendants may survive cancellation.** That platform limitation
must be resolved/accepted before Windows runtime release. Callers must signal the
token and await completion; dropping a future only has Tokio's direct-child
`kill_on_drop` guarantee. No idle polling is added.

`GitRunner::discover` uses an explicit path authoritatively, otherwise platform
installation paths, then PATH, with macOS `/usr/bin/git` last. macOS candidates
resolving to `/usr/bin/git` are gated by a successful `xcode-select -p` (bounded to
5 seconds), including explicit overrides/symlinks, before any invocation. Windows
checks Program Files, Program Files (x86), LocalAppData Git installations and PATH;
missing Git returns installation/selection guidance. Versions parse Apple Git
and `.windows.N` suffixes. Discovery validates executability and version.

**Minimum: Git 2.35.2.** Checked against `MIN_GIT_VERSION` in
`crates/gitwood-core/src/git/version.rs` (`GitVersion(2, 35, 2)`). Parameter requirements are lower: porcelain v2 / `-z`
(2.11), `--no-optional-locks` (2.15), and combined `git apply --cached --3way`
(2.32). The floor also requires Git's repository ownership / `safe.directory`
boundary delivered in 2.35.2. This is a feature floor, not a claim that an old Git
has all subsequent security fixes. Prefer a maintained Git release.
See the official [2.15 release notes](https://github.com/git/git/blob/v2.15.0/Documentation/RelNotes/2.15.0.txt),
[2.32 release notes](https://github.com/git/git/blob/v2.32.0/Documentation/RelNotes/2.32.0.txt),
and [2.35.2 release notes](https://github.com/git/git/blob/v2.35.2/Documentation/RelNotes/2.35.2.txt).
Versions below the floor return `VersionTooOld` before repository access.
The minimum-version binary itself has not been exercised locally.

`ReadCommand` accepts only the typed local `ReadOperation` set: status, diff,
log, show, rev-parse, ls-files, ls-tree, diff-tree, for-each-ref. It cannot represent Git aliases,
network operations or configuration writes. Background policy is centralized:

- `--no-pager --no-optional-locks`, `core.fsmonitor=false`,
  `core.untrackedCache=false`, `core.sshCommand=`, `submodule.recurse=false`,
  `log.showSignature=false`, `format.pretty=medium`, `log.diffMerges=separate`.
- Diff/show/log/diff-tree additionally use `--no-ext-diff --no-textconv`.
- Status verbose options (`-v`, short-option clusters such as `-bvv`, `--verbose`
  and abbreviations) are rejected: status has no no-textconv option, and its
  internal verbose diff can invoke textconv. Use the hardened diff operation for
  patch output; normal/porcelain status remains available.
- Status/diff/log/show/diff-tree use `--ignore-submodules=all`. Dirty submodule
  detection and inline submodule diffs can start Git inside a child repository;
  the parent's `-c` parameters propagate through `GIT_CONFIG_PARAMETERS`, but its
  filter inventory does not include child-local driver names. Ignore all prevents
  this entry, including `diff.submodule=diff` and `status.submoduleSummary`.
  Re-enabling inspection (including `--no-ignore-submodules`) is rejected. This
  also hides changed gitlinks; future submodule UI must open the child through its
  own hardened runner. Partial-commit submodule selection is not yet supported.
- Options that re-enable external helpers, output files, pager execution or
  submodule recursion (including abbreviated long options) are rejected before
  the path separator `--`. These arguments are backend-owned, never raw UI input.
- Signature output is unavailable for passive reads: reject `--show-signature`
  and signature `%G` placeholders (also `%+G`, `%-G`, `% G`) in `--format`/`--pretty`,
  including split and abbreviated option forms. Accept only exact built-in pretty
  names or inline formats; repository `pretty.<name>` aliases could hide `%G`.
  Override `format.pretty` so its configured default cannot hide verification.
  This avoids invoking GPG/SSH/X.509 verifiers at all, without relying on a dummy
  executable path behaving identically on three platforms.
- All calls force `LC_ALL=C` and `GIT_OPTIONAL_LOCKS=0`; reads also force
  `GIT_TERMINAL_PROMPT=0` and an empty `GIT_ALLOW_PROTOCOL` transport allowlist.
  The local command set does not require SSH; the transport ban also blocks
  lazy fetching missing promisor objects, so repository `core.sshCommand` cannot
  execute. It is independently overridden as defense in depth.
- The untracked cache is disabled to give reads deterministic behavior independent
  of repository cache configuration; optional index writes are already disabled.
  Measure large-repository performance before revisiting this choice.

Before **every read**, run the private, fixed query
`git --no-pager <read-config-overrides> config --show-scope --null --name-only --get-regexp '^filter\.'`.
Config lookup does not execute filter commands. NUL scope/key pairs avoid parsing
values or losing driver names containing dots/newlines; exit 1 means no matches,
other errors abort the read. For every driver with any property from local,
worktree or an unknown scope, append command-scope overrides
`filter.<name>.clean=`, `.smudge=`, `.process=`, `.required=false`.
Includes (`include.path`, `includeIf`) retain their originating scope and are
covered, including `config.worktree`. The entire driver is disabled even if only
one local property overlays a global definition. Empty values disable the helper
without making required filters fail the diff.

**Trust boundary:** system/global filters and trusted backend command-scope
configuration remain enabled (e.g. globally installed Git LFS). These helpers can
execute on repository content; they and the selected Git executable are part of
the user's trusted environment, not a sandbox. A global include is also trusted.
Repository-local filters do not run on passive reads: displayed content/diffs may
be unfiltered and differ from an explicit native Git write. Writes keep all filter
behavior. There is **no filter cache** in stage 0: every read costs one extra Git
process/config traversal, so config/worktree/include changes take effect on the
next read with no invalidation bookkeeping. This is not an atomic config snapshot;
an actor actively rewriting configuration between query and read is outside this
static untrusted-repository boundary.

### External-program reachability audit (nine background operations)

Checked against the locally installed upstream Git **2.53.0 documentation**
(offline); the security fixtures use the Git executable selected by discovery.
“Reachable” below describes native Git before Gitwood's policy. The set is exactly
status/diff/log/show/rev-parse/ls-files/ls-tree/diff-tree/for-each-ref; adding an operation or
accepting raw frontend options requires another audit. Documentation references
are upstream files/sections, not claims of a full Git source security proof.

| Configuration / entry | Reachable? + basis | Handling |
|---|---|---|
| `cat-file -s <oid>` / `cat-file blob <oid>`（Diff 追加操作） | 固定对象大小和原始 blob；不使用 `--filters` / `--textconv` / revision walk，不触发 checkout 转换或签名。缺失 promisor 对象仍可能走 lazy fetch。 | GitRunner 统一 fsmonitor/filter/pager/transport 约束；两项 opt-in 由既有验证拒绝。先固定 OID 再检查大小、读取。`diff_blob_reads_cannot_execute_repository_helpers` sentinel 回归。 |
| `for-each-ref` formats / ref enumeration | Local ref/object metadata only; no checkout, diff, signature verification or alternate-ref command. `git-for-each-ref` format atoms describe stored fields, not shell execution. | Fixed backend format; reject signature-verification format atoms; centralized fsmonitor/filter/pager/transport policy. `for_each_ref_cannot_execute_repository_helpers` sentinel regression. |
| `core.fsmonitor` | Reachable while reading/refreshing indexes (status, diff, ls-files). `Documentation/config/core.adoc`, fsmonitor; `githooks.adoc`, fsmonitor-watchman. | Override false for query and read; disable untracked cache and optional locks. |
| `filter.<driver>.clean/process` (and smudge) | Reachable during worktree-to-index conversion (status/diff); `gitattributes.adoc`, filter / long-running filter process. `--no-textconv` does not disable these. | Scoped discovery + all four overrides above. Trusted global/system/command filters intentionally remain reachable. |
| `diff.external`, `diff.<driver>.command`, `diff.<driver>.textconv` | Reachable through diff/log/show/diff-tree; `diff-options.adoc`, ext-diff / textconv; `gitattributes.adoc`, diff. **Status verbose diff can also execute textconv** (`git-status.adoc`, verbose; positive-control sentinel test). | `--no-ext-diff --no-textconv` for diff-capable operations; reject opt-ins/abbreviations and all verbose status forms because status has no no-textconv switch. |
| `log.showSignature`, `gpg.program`, `gpg.openpgp.program`, `gpg.x509.program`, `gpg.ssh.program` | Reachable through signed log/show and revision pretty output; `config/log.adoc`, showSignature; `config/gpg.adoc`, program; `pretty-formats.adoc`, `%G*`. | Disable signature display, reject signature formats and configured pretty aliases, override default format. No verification program is started. |
| `format.pretty`, `pretty.<name>` | Reachable indirect verifier selection; `config/format.adoc`, pretty; `pretty-formats.adoc`, configured formats. | Safe default medium, exact builtin or inline format only; reject signature placeholders and named aliases. |
| `diff.submodule`, `status.submoduleSummary`, `submodule.*.ignore` | Reachable child Git reads, including inline worktree diff; `diff-options.adoc`, submodule / ignore-submodules; `git-status.adoc`, ignore-submodules. Child-local filters are not in parent's list. | Ignore all submodules and reject reversal/recursion. Sentinel fixture first proves native inline diff invokes a child-only filter. |
| `core.alternateRefsCommand` | **Reachable** via revision walk `--alternate-refs`, not ordinary alternate-object lookup. `rev-list-options.adoc`, alternate-refs; `config/core.adoc`, alternateRefsCommand. | Reject `--alternate-refs`/abbreviations; native positive-control sentinel regression. |
| `merge.<driver>.driver`, `merge.renormalize`, `log.diffMerges` | **Reachable** via log/show remerge-diff (in-memory merge can use custom drivers and conversion). `diff-options.adoc`, remerge; `gitattributes.adoc`, custom merge drivers / renormalize. | Reject remerge and unsafe diff-merges modes; pin configured default to separate so `-m` cannot select remerge. Native merge-driver sentinel regression. |
| `core.pager`, `pager.*`, `GIT_PAGER` | Reachable through paging, and ls-files can explicitly open files in a pager; `git.adoc`, no-pager; `git-ls-files.adoc`, open-files-in-pager. | Global `--no-pager`; reject explicit open-files-in-pager (including abbreviations). |
| `core.hooksPath` (ordinary write hooks) | **Unreachable**: the nine reads do not commit/checkout/update refs; `githooks.adoc` hook trigger list. Index-writing post-index-change is avoided by no optional locks; fsmonitor hook is handled separately above. | Keep native write hooks; no read hook trigger. |
| `core.sshCommand`, `core.gitProxy`, `remote.*.vcs`, remote helpers | Conditionally reachable if missing promisor objects cause lazy fetch; `partial-clone.adoc`, fetching missing objects; `gitremote-helpers.adoc`. Ordinary complete-object reads do not contact remotes. | Empty `GIT_ALLOW_PROTOCOL` forbids every transport; additionally clear sshCommand. Missing-promisor-object sentinel regression. |
| `uploadpack.*` (including `packObjectsHook`) | **Unreachable**: server-side upload-pack/pack generation, not local reads; `config/uploadpack.adoc`. A lazy fetch could reach a server only through transport. | No upload-pack operation in read enum; transport ban also stops lazy-fetch entry. |
| `protocol.*` | **Unreachable after transport ban**: these are transport allow rules, not standalone commands; `config/protocol.adoc`; `git.adoc`, GIT_ALLOW_PROTOCOL (overrides protocol configuration). | Empty allowlist takes precedence over repository `protocol.*.allow=always`. |
| `credential.helper` | **Unreachable after transport ban**: credential acquisition belongs to remote authentication; `gitcredentials.adoc`, requesting credentials / helpers. The nine local reads never request credentials on their own. | Ban transports, including lazy fetch; keep credentials available for authorized writes. |
| `core.askPass`, `GIT_ASKPASS`, `SSH_ASKPASS` | **Unreachable after transport ban**: password prompts require an authentication path; `config/core.adoc`, askPass; `gitcredentials.adoc`. `GIT_TERMINAL_PROMPT=0` alone would not disable askpass helpers. | Ban transports; disable terminal prompting as an additional read constraint. |
| `core.editor`, `sequence.editor` | **Unreachable**: message/sequence editing belongs to commit/tag/rebase; `config/core.adoc`, editor; `config/sequence.adoc`, editor. None of the nine reads edits messages. | No editor operation in read enum; preserve native behavior for writes. |
| `include.path`, `includeIf` | **No direct execution**; they can import executable helper definitions. `git-config.adoc`, includes / conditional includes / scopes. | Read effective config with includes and scope metadata; do not flatten local includes into trusted global scope. |

Upstream primary references: [git-config](https://git-scm.com/docs/git-config/2.53.0),
[attributes](https://git-scm.com/docs/gitattributes/2.53.0),
[pretty formats](https://git-scm.com/docs/pretty-formats/2.53.0),
[revision options](https://git-scm.com/docs/git-rev-list/2.53.0),
[hooks](https://git-scm.com/docs/githooks/2.53.0),
[credentials](https://git-scm.com/docs/gitcredentials/2.53.0),
[Git environment](https://git-scm.com/docs/git/2.53.0).

`WriteCommand` is for explicitly authorized writes and retains user configuration,
credential helpers, filters, hooks and signing. Only common locale/optional-lock
settings and no-pager apply. Neither mode erases global/system config or changes
`safe.directory`. Ownership rejection is a dedicated `UnsafeRepository` error
carrying the diagnostic path. Other typed errors distinguish discovery, version,
spawn, exit status/stderr, timeout, cancellation and I/O. Extra environment is a
trusted backend facility, including `GIT_INDEX_FILE`; do not accept environment
or repository-routing variables directly from frontend/repository content.

## Partial commits with an alternate index

`ops::partial_commit(runner, options, selection, message, amend)` is the reusable
stage 0 operation. `options.cwd` is checked to be the repository/worktree root; a caller or inherited
`GIT_INDEX_FILE` override is rejected.
`CommitSelection.files` contains literal root-relative file paths (rename: both
old and new names). Directories/absolute paths/traversal are rejected. Whole-file
selection uses **working-tree** contents with native `git add -A -- <literal path>`
semantics, including deletion, new files, filters and attributes.
`CommitSelection.patches` contains unified diff bytes **relative to current HEAD**,
with context, not index-to-worktree diffs. Apply patches in order using
`git apply --cached`; Git enforces index-only path safety. Whole files are added
first; patches are then applied. Nothing is checked out into the working tree.

The operation serializes partial commits across this process. Low-level
`GitRunner::run(WriteCommand)` callers must participate in adapter write
coordination when more write operations are added. The transaction:

1. Resolve the per-worktree index using `rev-parse --git-path index`; reserve its
   `.lock` with exclusive creation **before writing**. Existing locks return
   `IndexLocked` and are never removed. Snapshot the real index bytes and HEAD;
   alternate indexes live in a unique private system temporary directory.
2. `read-tree` seeds an empty tree for unborn HEAD or HEAD for a normal commit.
   Amend starts from HEAD's first parent (empty tree for a root commit), then
   restores the old HEAD tree so changes already in that commit remain present.
   The selected delta is applied on top; native `commit --amend` preserves parents.
3. Build selected tree **C**, with old HEAD/empty tree **H** and original real
   index **I**. Precompute the aligned real index in another temporary file:
   copy I; reset all whole-file selections to H in this copy; generate a canonical
   binary/full-object-ID patch H → C (explicit prefixes, context, no color or
   renames); apply it to the copy with `apply --cached --3way`.
4. For unrelated paths the aligned index is exactly I. Whole-file selections
   intentionally replace earlier staging for those paths and align with C.
   For hunk selections, the three-way application coalesces already-staged chosen
   changes and preserves non-overlapping original staging on top of C. Any other
   unselected working-tree changes remain on disk and appear unstaged. Thus a
   hunk-selected path may intentionally retain a staged delta against new HEAD.
   Conflicting same-path changes are rejected as `StagedConflict` **before commit**;
   no conflict markers or unmerged entries are written to the real index.
5. Recheck HEAD and raw index bytes, then call native `git commit [--amend] -F -`
   with the selected `GIT_INDEX_FILE`. Hooks and GPG signing remain enabled.
   Supplying `-F` bypasses `commit.template`, as native Git does; hooks still
   receive Git's normal message file. A hook rejection/signing failure leaves
   Gitwood's real index and working files untouched and removes temporary files.
6. After success, recompute alignment using the actual committed tree because
   hooks may add changes to the alternate index. Recheck HEAD and original index;
   write/sync the prepared bytes to the held real `index.lock`, close it, then
   atomically rename it over the real index. Clean up alternate indexes and locks.

Example for the same file: HEAD has lines `one` / `five` / `nine`; real index
changes `one` to `STAGED`; working tree additionally changes `five` to `SELECTED`
and `nine` to `WORKTREE`. Selecting only the middle hunk commits `SELECTED`, keeps
`STAGED` staged, and leaves `WORKTREE` unstaged. Selecting the whole file instead
commits all its working-tree contents and consumes that path's earlier staging.

The index lock blocks cooperating Git tools, not manual file/ref rewrites. Byte
and HEAD checks detect observed external changes, but HEAD and index are not one
atomic filesystem transaction. A crash after ref update and before index rename
requires reconciliation; no crash journal exists in this stage 0 probe. If an
index alignment/install error occurs after commit, `partial_commit` returns
`Result<CommitOutcome, CommitError>` with distinct cases:

- `Ok(Committed { hash })`: commit and index reconciliation completed.
- `Ok(CommittedWithIndexRecovery { hash, recovery })`: commit completed, real
  index reconciliation failed. `IndexRecovery` carries directory, target index,
  reason and recovery instructions. UI must present this as completed with a
  recovery action, never a failed commit or automatic retry.
- Ordinary `Err` (validation, locks, hook rejection/signing failure with unchanged
  HEAD): Gitwood has not changed HEAD or installed a real index. Arbitrary hook
  effects and external writers cannot be guaranteed unchanged.
- `Err(OutcomeUncertain { head, reason })` is explicitly **not** “failed without
  changes”: inspect HEAD before any retry (e.g. cancellation after ref update).

Recovery snapshots (`original`, `selected`, potentially unmerged `aligned`) and
`RECOVERY.md` are preserved inside the actual per-worktree Git directory as
`gitwood-recovery-<Unix epoch milliseconds>-<full commit hash>-<random suffix>`.
Linked worktrees use `.git/worktrees/<name>/`. If disk/permission failure prevents
copying there, retain the sole system-temp snapshot and return its actual path
plus that error; never delete it just to satisfy the preferred location.

For manual recovery, close other Git writers, inspect HEAD and back up any newer
real index. From the worktree root, set `RECOVERY` to the returned directory and
`BASE` to the base tree recorded in `RECOVERY.md`, then export the original staging
before resetting (POSIX shell):

```sh
GIT_INDEX_FILE="$RECOVERY/original" git --no-optional-locks -c core.fsmonitor=false diff --cached --binary --full-index --no-ext-diff --no-textconv --src-prefix=a/ --dst-prefix=b/ --no-color --no-relative --output="$RECOVERY/staged.patch" "$BASE" --
git reset --mixed <completed-commit-hash>
git apply --cached --3way "$RECOVERY/staged.patch"
git status --short
```

Use `--output`, not shell redirection, for binary patches (also on Windows).
PowerShell sets `$env:GIT_INDEX_FILE` for the export and removes it before
reset/apply; `RECOVERY.md` records the same steps. If `original` was absent or
the patch is empty, skip the corresponding export/apply. Reset changes only
the index; applying may leave index conflicts for manual resolution. Review the
intended staging and retain recovery files until verified; neither `original`
nor a conflicted `aligned` snapshot should be blindly installed over newer work.

After timeout/cancellation, a changed
or unreadable HEAD yields `OutcomeUncertain`; inspect HEAD before retrying.
Recovery checks run with a fresh token and bounded timeout even after cancellation.
Callers must signal cancellation and await the result, rather than drop the whole
partial-commit future during its critical section.

Gitwood cannot roll back arbitrary filesystem/ref/network actions performed by
user hooks, filters, signers or external tools. Those actions are trusted because
this is a user-initiated write. Hook/signing failure regressions exercise rejecting
helpers with no unrelated side effects. Sparse-checkout/intent-to-add/unmerged
indexes, submodule changes and third-party hooks using hard-coded real-index paths
need additional acceptance before stage 3. Split indexes and linked worktree index
routing have real-Git coverage. Windows helper fixtures are currently skipped;
Windows/Linux runtime validation and minimum-version execution remain outstanding.
The application shell now exposes typed read-only repository IPC; the history benchmark/reader and
isolated fixtures also invoke the hardened runner. Discovery avoids triggering the macOS placeholder Git's
Command Line Tools installation dialog.

## E2E strategy

方案核对日期：2026-10-29。本节确定后续实施方向，本次未安装框架、修改 CI
或运行 E2E；当前桌面壳尚未接入 Git，不能宣称业务全链路已覆盖。

### 推荐分层与平台矩阵

| 层 | 覆盖与边界 | 计划运行平台 |
| --- | --- | --- |
| Rust 真实仓库集成测试 | 调用生产 GitRunner / ops，验证状态、特殊路径、安全配置、部分提交保留已有暂存、hook/签名拒绝、锁与外部修改；不证明 UI/IPC 正确 | macOS、Windows、Linux CI；平台跳过项单列，不能算通过 |
| Vitest 组件测试 | 控件状态、键盘规则、选择与焦点逻辑 | 三平台 CI |
| Playwright UI 回归 | 用本地静态服务器提供 Vite 正式构建产物，测试无需 IPC 的布局、分隔线、菜单、键盘和主题；首选 Linux CI 的 Chromium/WebKit，macOS/Windows CI 补充 Chromium 冒烟 | Linux 主覆盖，macOS、Windows 冒烟；浏览器 WebKit 不等于 WKWebView |
| 原生 WebDriver E2E | WebdriverIO 客户端 → 官方 tauri-driver → 实际应用 UI → 正式 IPC → Rust → 临时 Git 仓库；验证打开/浏览/Diff、选择提交/Amend、分支与本地 bare 远端同步及失败反馈，按功能落地逐步加入 | Windows、Linux CI；Linux 需要显示服务（如 Xvfb）及 WebKitGTK 驱动 |
| 真实应用人工验收与 IDEA 录屏 | 发布构建、系统窗口/菜单/文件选择/认证、真实焦点和平台快捷键；覆盖 macOS 缺少自动全链路的部分 | v0.1 macOS 必须；v0.2 前补 Windows、Linux 本机 IDEA 对照 |

E2E 从用户入口操作，测试进程在应用外创建唯一临时仓库、bare 远端和两个工作副本，
并以真实 Git 检查 HEAD、index、工作区和远端结果。禁止 mock IPC、直接调用后端
来替代用户动作，或注入假成功。失败保留截图、日志和仓库状态；不操作真实远端。
浏览器层不能调用 Tauri IPC；需要后端的场景交给原生层，不为浏览器增加生产 fallback。
测试夹具、断言和必要的浏览器测试替身仅能放在测试代码中，且替身结果不算 E2E。

### 官方支持现状与 macOS 备选

[官方手动配置](https://v2.tauri.app/develop/tests/webdriver/manual-setup/)
确认原生 tauri-driver 的桌面支持范围为 Windows/Linux。
Windows 通过 msedgedriver 驱动 WebView2，需匹配运行时版本，驱动放 PATH 或显式
指定；Linux 使用 WebKitWebDriver（Debian 系通常为 webkit2gtk-driver），需与应用
WebKitGTK 环境匹配。macOS 的 WKWebView 没有对应的原生驱动；Safari 自动化
不能据此当作驱动嵌入式 WKWebView 的证据。

[当前官方概览](https://v2.tauri.app/develop/tests/webdriver/)还推荐
WebdriverIO 的 Tauri service：其默认 embedded provider 通过注册
`tauri-plugin-wdio-webdriver` 在应用内启动服务，从而支持 macOS；另一个
`tauri-plugin-wdio` 提供后端访问与 mock 能力。这与原生 tauri-driver 的支持范围
是两条不同路线。本文选择 Windows/Linux 的外部驱动，不采用嵌入 provider 或 mock 插件。
官方也提及 CrabNebula 跨平台 fork，macOS 需要付费 API key；成本与本项目适配未验证，暂不采用。

社区候选 [danielraffel/tauri-webdriver](https://github.com/danielraffel/tauri-webdriver)
包含 `tauri-plugin-webdriver-automation` 和 `tauri-wd` CLI；插件必须注册到应用，
在进程内提供 HTTP 自动化服务。仓库提供 Apache-2.0 / MIT 双许可证；查询时可见
27 次提交、开放 issue/PR，未显示归档，但最近提交日期及持续维护承诺**未验证**，
不能仅凭存在仓库认定维护活跃。作者声明支持 macOS，不代表 Gitwood 已验证。
代价包括额外依赖、自动化端口攻击面、插件与 Tauri 版本耦合，以及测试包和发布包差异。

若未来试验上述嵌入方案，应使用默认关闭的独立 `e2e` Cargo feature，将 optional
依赖、插件注册及相关 capability 全部编译隔离，发布构建禁止该 feature，并验证
最终包无服务/权限残留；不能只依靠 debug_assertions，因为普通开发包也会启用。
这仍是在应用源码中增加测试专用开关，按当前规则**不可接受，暂不实施**。
独立测试构建并不自动豁免约束；需要先明确修改约束，且试验也只能走正式业务 IPC，
不能增加业务测试命令。当前 macOS 采用真实发布应用人工全链路，自动化缺口如实保留。

### IDEA 对照与未验证项

按实施计划“对照方法”，在本机 IDEA 2025.3 与 Gitwood 中对同一测试仓库执行同组
操作并录屏，将差异记入 interaction-spec：单击/双击/右键、多选与 speed search、
分支弹窗分组与子菜单、默认按钮/Esc/焦点返回、平台快捷键与菜单启用条件、Diff 跳转/
同步滚动/折叠/应用箭头、提交图选择、分隔线/面板显隐/主题切换，以及成功/失败/取消后
选择、说明文本和滚动恢复。仅对当阶段已实现功能验收；布局持久化实现后才验证重启恢复。
原生文件对话框、认证与系统菜单也必须手工检查；自动化截图不能代替 IDEA 行为对照。

**未验证**：本项目各平台驱动连接、CI 图形会话稳定性、原生对话框自动化范围、
WebView2/驱动版本组合、macOS 社区插件与当前锁定 Tauri 的兼容性及维护时效、
付费方案成本/适配、浏览器构建产物在当前壳的无 IPC 场景覆盖，以及最低 Git 2.35.2
实际执行结果。三平台 CI、最低 OS 和 Intel Mac 均需分别留证，文档调研不算运行通过。

补充官方参考：[Tauri WebDriver CI](https://v2.tauri.app/develop/tests/webdriver/ci/)、
[WebdriverIO 插件配置](https://webdriver.io/docs/desktop-testing/tauri/plugin-setup)、
[Playwright 本地服务器](https://playwright.dev/docs/test-webserver)。

## Verification boundaries

Local build results, CI platform runs, minimum-OS compatibility, Intel execution,
and memory/CPU baselines are separate evidence. Only the macOS native-architecture
build and local checks are required for this task. Stage 0 filename fixtures call
real Git directly with argument arrays: Unicode, spaces and leading dashes on all
platforms, plus tabs/newlines on non-Windows systems. Fixtures use unique system
temporary directories and clean up after themselves, avoiding timestamp collisions.
GitRunner security/process tests and alternate-index commit tests now exercise
real temporary repositories, including special filenames, ownership rejection,
hook/signing failures and locks. Executable shell-helper fixtures are Unix-only;
Windows skips them explicitly in source. This does not establish three-platform
runtime acceptance or completion of all stage 0 work.
History reader tests use temporary repositories outside the source tree and cover
the production adapter. These fixtures do not independently establish full
GitRunner discovery/cancellation or domain/security acceptance.

## 仓库会话与 IPC

阶段 2 后端使用 `repo::session::Sessions` 管理一个当前仓库；切换/关闭释放旧监听，
取消旧 Git 子进程并递增 generation。`history_layout` 是显式刷新入口，会递增 generation，
重建关系和布局；其返回的 generation 用于后续所有读取。
刷新过程中 Git/过滤失败时，错误也携带已分配的 generation，以便修正条件后重试。读取完成再次检查 generation，
旧请求返回 `repository.stale`。仓库所有权错误携带路径、诊断和 `repository.unsafeDirectory` 文案键；
bare 返回 `repository.bareUnsupported`，不更改 Git 安全配置。

IPC：`open_repository(path)`、`recent_repositories()`、`close_repository()`、
`repository_status(generation)`、`repository_refs(generation)`、
`history_layout(generation, filter)`、`history_rows(generation,start,end)`、`history_position(generation,hash)`、
`commit_details(generation,hash)`、`commit_files(generation,hash,start?,end?)`。
浏览界面另用 `repository_changes(generation,viewport)` 与
`commit_file_tree(generation,hash,viewport)` 获取 Rust 文件树的可见窗口。
`repository_status` 现在仅返回 `{branch,changes,untracked}` 摘要；完整状态及目录/平铺索引
保存在会话中。viewport 含 expanded、flat、start/end 和至多两个 positionIds；
返回所选/顶行 ID 的新位置，用于在节点插入后保持选择与滚动。
已核对现存调用方为当前 UI，无外部协议调用方。
文件列表页返回 `{total,start,files}`，默认从 0 开始最多 200 个文件。
每页最多 200 行，半开区间；图节点为 `[row,column,color,start,end]`，边为
`[edge,from,to,color,direction,dotted]`（方向 -1/1，dotted 为布尔值），详情 LRU 上限 1000 条。
全量关系和布局留在 Rust；完整说明仅在单条详情读取。最近仓库复用 settings.json schema 1，
缺字段默认为空；按成功解析的顶层路径去重，保留失败标记，最多 20 条。

过滤使用 `--author=value`、`--since=value`、`--until=value` 和固定字符串 `--grep=value`；
说明命中与 hash 子串命中取并集，其余过滤取交集。引用只接受完整 refs/heads、refs/remotes 或 refs/tags 名称，以及分离 HEAD 的固定 HEAD 标记。
路径在 `--` 后独立传参，并启用 `GIT_LITERAL_PATHSPECS=1`。
过滤可见性与关系读取分离：先读取该分支/全部 refs 与 HEAD 的完整父关系，
再取得路径/作者/日期/说明或 hash 命中的集合。对每个命中节点迭代遍历父关系，
每条路径遇到第一个命中祖先即停止，去重后按原拓扑行号排列；直接父边保留实线，
跳过中间提交的边标记 dotted，重新布局后随所有半行边段传入 Canvas。
路径匹配查询显式使用 `--parents --full-history -- <literal path>`：Git 会改写路径历史的父关系；
这里只使用其命中 ID，连通性和 dotted 判定始终基于未经过路径简化的完整关系，
因此路径与作者/说明过滤叠加仍保留最近命中祖先。
源码依据是同一 revision 的 `FilteredController.kt`（建立 CollapsedGraph 后调用生成器）、
`collapsing/DottedFilterEdgesGenerator.kt`（创建 GraphEdgeType.DOTTED）、
`api/elements/GraphEdgeType.java`（DOTTED 注释为 collapsed fragment）、
`impl/print/elements/EdgePrintElementImpl.kt`（DOTTED → LineStyle.DASHED）。
本项目使用独立迭代祖先遍历，不移植该生成器的双向 numbers walk，也不实现边界箭头。
全量关系读取包含 all refs 及当前 HEAD（包含分离 HEAD），空仓库为空图。
提交详情通过 log --no-walk=unsorted --stdin 批量读取，以 NUL 分隔字段。
文件列表通过 diff-tree -r -z --name-status -M 读取；合并提交比较第一父，根提交比较空树。

## 状态解析

porcelain v2 -z 支持 ordinary、rename/copy（原路径、相似度）、unmerged、untracked、ignored，
解析 branch oid/head/upstream/ab，分别输出 index/worktree、未跟踪、忽略和冲突集合。
同一文件可以同时出现在 index/worktree。路径保留原始 bytes 和有损 UTF-8 display，
包括空格、制表符、换行和非 UTF-8 字节；macOS APFS 无法创建非 UTF-8 文件名，解析器仍保留该字节。
子模块状态遵循 runner 既有安全策略隐藏，不进入子仓库读取。
引用使用 NUL 字段的 for-each-ref；upstream ahead/behind 采用 Git track,nobracket 输出，
保留 gone 状态；远端符号 HEAD 单独记录目标，标签保留附注标签 peeled object。

## 刷新与监听

notify 8 原生事件递归监听工作区、per-worktree git_dir、common_dir，关闭或切换即释放。
忽略 Git objects/logs 子树、*.lock（含 index.lock）和访问事件；工作区/index 触发 status，
HEAD、refs、packed-refs、配置变更触发 status/refs/history，监听错误触发全量重查。
300 ms 静默防抖，空闲阻塞等待事件、不轮询。窗口重新获得焦点发出全量检查信号。
`repository-refresh` 事件载荷为 `[generation,{status,refs,history}]`；前端收到后调用
history_layout 刷新 generation，再请求所需窗口。前端合并同时到达的信号并串行处理：
仅 status 时读取状态摘要/Commit 树；仅 refs 时更新分支树及标签；仅 history 时重建历史及窗口。
历史刷新用 history_position 解析所选 hash 与视口顶端 hash 的新行号，保持选择与像素偏移；
历史 generation 保留独立状态与不可变提交文件缓存，布局本身带 generation 标记；
取消的旧读取在持锁写入前再检查 token，不能覆盖新布局。普通状态刷新、主题或分隔线更新不触发历史重排。

### 阶段 2 只读浏览界面

File → Open… 通过已安装的 dialog 2.8.4 目录选择器，将路径交给 open_repository 校验；
capability 仅增加 dialog:allow-open。没有注册 fs 插件，也没有 fs:* 或 save/message 权限。
File → Recent Projects 来自现有 settings 最近记录；启动恢复最近一次成功打开的项目。
原生菜单与欢迎页共用仓库入口。open/close 在前端排队，避免 discovery 完成次序改变最终仓库。
仓库切换、分页、过滤、提交详情和树搜索分别用请求序号丢弃旧响应，后端仍检查 generation。
原生 View 菜单与左侧工具栏共用设置；set_tool_window 只接受 Commit/Log 枚举及可见性，
更新菜单、设置和 preferences-changed，不开放通用菜单执行。

Commit 区显示 Changes/Unversioned Files；同文件合并 index/worktree 标记，冲突独立着色，
支持目录/平铺、展开/折叠、虚拟滚动及现有 speed search。原型使用 Changes 工作方式，
本次不启用 IDEA 的独立 staging-area 模式；暂存状态只作只读提示，不提供勾选或暂存动作。
分支树按 Local/Remote（远端及目录）/Tags 分组；HEAD、上游及 ahead/behind 随元数据展示。
分支/标签选择只过滤历史，不检出。提交表说明与 ref 标签同列，作者和日期分列；
日期默认采用作者时间（上游 isPreferCommitDate=false），使用系统 locale 的短日期/时间。
日期列参照 `ui/table/column/VcsLogDefaultColumn.kt` 的 formatPrettyDateTime 调用。
本机 IDEA app.jar 的 DateFormatUtil.doFormatPretty 字节码及 util-8.jar 的 UtilBundle.properties
另确认：61 分钟内用 Math.rint 的偶数舍入显示 Moments/A minute/N minutes/1 hour ago，
之后显示今天/昨天和时间。界面像素及系统设置的一致性仍需实际 IDEA 对照。

List 的 overlay 接口驱动单个可见高度 Canvas；水平滚动和高 DPI 坐标一起变换，按主题
--graph-lanes-N 着色，节点覆盖边段。可见行详情缓存限制在窗口附近，后端详情 LRU 为 1000。
文件树的目录索引、状态和当前提交文件列表留在 Rust，树页最多 200 节点；前端只保留窗口及
有限邻域。分页树 speed search 逐页复用现有 matcher，仅扫描已展开节点并取消过期搜索。
信息区显示完整说明、hash、作者、时间、父提交；containing_branches 独立异步查询包含该提交的本地/远程分支，返回最多 200 项与总数。中央 Diff 已接通工作区及提交文件选择。

性能证据见 [phase2-browse.md](perf/phase2-browse.md)。原生菜单/对话框、WebView 绘制、
刷新后的真实焦点和滚动、IDEA 录屏及 Windows/Linux 运行均未验证；实现状态不等于阶段 2 整体验收。

## Diff

阶段 2 第 6、7 步的核心与只读组件已接入 IPC 和 status 刷新；无写操作。
不代表阶段 2 整体验收完成。Rust 只读原始字节，比较当前一个文件；UI props 接收
当前文件两端文本/特殊状态及比较结果，CodeMirror 虚拟化编辑器视口，不读取仓库。

### Rust 公开 API 与数据契约

`gitwood_core::diff` 导出：

- `read_content(&GitRunner, &RunOptions, ContentSource, ReadLimits) -> Result<Content, DiffError>`，async。
  `options.cwd` 必须为仓库/工作树根目录，复用调用方环境、取消、超时及 safe.directory。
  `ContentSource::Commit { revision, path }` 支持 HEAD 或完整 40/64 位提交 ID；
  `Index { path }` 使用 `:0:<path>`；`Worktree { path }` 使用 std::fs。
  路径是字面量、根相对路径，拒绝绝对路径及 `..`；保留 OsStr 字节。
  Git 先 `rev-parse --verify --end-of-options <spec>` 固定对象 ID，再 `cat-file -s <oid>`，
  大小合格才 `cat-file blob <oid>`；对象 ID 固定后不因 index 更新而读到另一 blob。
  不运行 textconv/checkout filters。缺失端、冲突 index 无 stage 0、非 blob 返回错误，
  不能当作空文件成功；将缺失版本映射为显式空端由后续仓库接入负责。
- `ReadLimits { max_bytes }` / `DEFAULT_MAX_BYTES`：默认 8 MiB。工作区先查大小，
  再通过 `Read::take(limit+1)` 处理读取期间增长；超限不解码、不传文本。
  工作区 canonical 路径必须仍在根内，只接受 regular file。内部符号链接解析到目标；
  拒绝外部目标。路径解析到打开之间的外部恶意重命名竞态尚未消除，
  同步 std::fs 与 compare 应由后续后台 worker 调用，不能阻塞 UI executor。
- `classify(Vec<u8>, ReadLimits) -> Content`：`Text { text, byte_len, empty, missing_final_newline }`、
  `Binary { byte_len }`、`Undecodable { byte_len }`、`TooLarge { byte_len, limit }`。
  判定顺序：大小 → 任意位置 NUL → 严格 UTF-8；不猜编码，后续再评估编码支持。
  NUL 扫描覆盖整个受限内容，比 Git 常用的前部采样更保守，不读取 `.gitattributes` 来覆盖规则。
  空文件不算缺少末尾换行；非空文本最后字节不是 LF 才标记缺少末尾换行。
  原始 CRLF/LF 保留。读取时增长触发的 TooLarge 长度是已观测下界 limit+1，
  不是不断变化的文件最终长度；其他长度为已读字节数或 size/metadata 快照。
- `compare(&str, &str, CompareOptions) -> DiffResult`：similar 3.2.1，行比较及 Unicode 词比较。
  `CompareOptions { context_lines: 4, whitespace: Whitespace::Exact }`；保留明确的空白策略边界，
  不接受尚未实现的忽略模式。所有比较共用 1 秒算法 deadline，超时由 similar 返回有效的
  更粗变更块，不能保证最小编辑序列；分词、输出和内存分配不受硬实时上限保证。
- `DiffResult { blocks, alignment, folds, left_lines, right_lines }`。
  `ChangeBlock { kind: Insert/Delete/Modify, left, right, left_inline, right_inline }`；
  `Alignment { left, right }` 包含等同及变更段；`FoldRange { left, right }` 配对折叠两端。
  `LineRange { start, end }` 全部从 0 开始，半开区间；空区间是插入/删除锚点。
  `InlineRange { line, start, end }` 的列为 UTF-16，不含 CRLF 的终止符，适配 CodeMirror。
  非空文本按 LF 切行，不额外输出末尾 LF 后的虚拟行；空文件行数 0。
  CRLF/LF 差别、末尾换行差别仍产生变更块；仅 EOL 差别无可见词宽，用块背景及末尾标记显示。
  serde 为 camelCase，Content 用 `kind` 标签，与 `ui/src/features/diff/types.ts` 对应。

### CodeMirror 与生命周期

`DiffView.svelte` 的 props：`left: string | Content`、`right: string | Content`、
`result: DiffResult`；纯文本字符串也会生成空文件/末尾换行状态。TS 的 `LineRange`、
`InlineRange`、`ChangeBlock`、`Alignment`、`FoldRange`、`DiffResult`、`Content` 与 Rust
同构，只有 serde 的 camelCase 命名差别。此阶段每次 props 切换视为换文件，重建当前两个
EditorView；卸载/换文件时 destroy，并断开 ResizeObserver、快捷键和旧测量回调。
没有全文件缓存、空闲轮询、持续动画或额外 WebView。

选择两个 `EditorView` 自行同步：`@codemirror/merge` 6.12.3 的 MergeView 自行计算
Chunk、对齐 spacer 和折叠，不以 Rust 的变更块/折叠/对齐结构作为唯一输入；
其内建重算会引入两套比较语义。本组件直接用 Rust 区间创建 Decoration，
中间 SVG 带和禁用 `<<` / `>>` 按钮按编辑器实测行边界绘制；双栏不强制插入等高空行。
滚动按分段行号映射，保留行内像素偏移；折叠 widget 高度参与测量，非全文件比例。
滚动中二分查询可见变更块，仅测量候选连接带；只响应 scroll/resize/编辑器 geometry 事件。
折叠按钮一次展开配对两端；不重新比较或重建 EditorView。分隔栏支持拖动和键盘左右调整，
复用 `constrainSplit` 的尺寸约束；宽度不持久化。

原始文本仅通过 CodeMirror doc 进入视图；UI 将 CRLF 规范为 LF，保持行号/UTF-16 列。
不启用语言包、可编辑状态、HTML 渲染、写入 IPC 或假的应用成功提示。
空文件显示说明并保留空编辑器锚点；二进制/解码失败/过大各有专门说明和字节长度。
末尾换行状态放在每栏页脚。

CodeMirror 6.43.15 / style-mod 4.1.4 动态插入 `<style>`，生产 CSP 仍为严格策略。
`ui/index.html` 的 meta 使用 `__TAURI_STYLE_NONCE__`；锁定的 tauri 2.12.5
`src/manager/mod.rs::{set_csp,replace_csp_nonce}` 在加载 HTML 时替换 token 并加入
style-src；组件读取该 meta，传给 `EditorView.cspNonce`。Vite 开发环境保留 token，
使用现有 devCsp；没有扩大生产 unsafe-inline 权限。源码与前端构建已核对，
尚未在原生发布 WebView 中验证 nonce、CodeMirror 样式和 CSP 的实际联动。

### IDEA 行为依据与差异

本机 `/tmp/intellij-community` revision `0698a5fe297dfa08069f015cc423a6a85ed7fc60`，
是此前下载的 master 快照，不等于计划参照的 IDEA 2025.3 tag。以下路径相对该仓库；
本次按源码行为独立实现，未复制 JetBrains UI/图标。

| 对照点 | 源码出处 | 当前状态 |
| --- | --- | --- |
| 变更背景、行内高亮、连接带与两向应用入口 | `platform/diff-impl/src/com/intellij/diff/tools/simple/SimpleDiffChangeUi.java` 的 createHighlighter/createInlineHighlighter/drawDivider/createAcceptOperation | 组件已实现背景、按词区间及 SVG；应用按钮按本阶段要求禁用。词边界由 similar Unicode 分词提供，并非 IDEA ComparisonManager，复杂重排不保证相同最小块。 |
| 行号、只读、无 soft wrap、同步滚动默认启用 | `platform/diff-impl/src/com/intellij/diff/tools/util/base/TextDiffSettingsHolder.kt` PlaceSettings | 已实现；无自动软换行。 |
| 上下文默认 4 | 同文件第 59 行 `CONTEXT_RANGE = 4` | 已采用。源码 `EXPAND_BY_DEFAULT = true`，本示例为演示交互初始折叠，尚未接设置。 |
| 折叠区间及两端同步展开 | `platform/diff-impl/src/com/intellij/diff/tools/util/FoldingModelSupport.java` FoldingBuilderBase/addGroup/createBlock | 已实现相邻差异各留 4 行，开头/结尾不保留外侧上下文，至少隐藏 2 行。只支持一次完整展开；未实现上游 4/8/16 嵌套层级、折叠状态缓存、再次收起。 |
| 分段同步行号 | `platform/diff-impl/src/com/intellij/diff/tools/util/BaseSyncScrollable.java` transferLine；`SyncScrollSupport.java` transferVisualLine | 实现首尾边界、区间内 offset 截断和尾部延伸，不做文档比例。浏览器像素/WebKit、滚动条拖动仍需实际对照。 |
| F7 / Shift+F7 | `platform/platform-resources/src/keymaps/$default.xml` NextDiff 第 606 行、PreviousDiff 第 971 行；`Mac OS X 10.5+.xml` 继承 `$default` | 已追加 keymap 并在组件作用域注册。Windows/Linux 只添加该继承条目；macOS function-key 系统设置及跨平台运行未核对。 |
| 差异跳转到首/尾 | Next/PreviousDiff 的快捷键已核对，边缘导航具体提示/循环行为未核对 | 当前不循环，不伪造 IDEA 边缘提示。 |
| 空文件、二进制、非 UTF-8、过大、无末尾换行 | 专门状态按需求实现；IDEA 对应提示与无末尾换行的标记位置未核对 | 不宣称该提示与 IDEA 1:1；当前末尾标记在页脚。 |

### 语法高亮（待决定）

本次不接语言包。阶段 2 后续比较 Lezer 语言包（CodeMirror 增量解析、视口高亮，
代价是 JS heap 与每种语言包体积）和 Rust tree-sitter（解析留 Rust，需处理语法库、
可见区间查询、UTF-8/UTF-16 转换、缓存与 IPC 高亮跨度）。用相同的 JS/TS、Rust、Java、
JSON、Markdown 夹具，记录冷/热解析耗时、Rust/WebView 分进程峰值内存、安装包增量、
语言覆盖、编辑器滚动/换文件响应；都映射 `syntax.*`。目前无这两种方案的对比实测，
不据现有依赖判断胜出；选型仍待决定，不把阶段 2 第 7 步整体标记完成。

### 验证范围

真实临时 Git 仓库覆盖 HEAD、提交 ID、stage 0 index、工作区、超限与路径拒绝；
安全 sentinel 证明 blob 读取不触发本地 fsmonitor/filter/textconv/pager 等助手。
Vitest 覆盖滚动分段、差异导航、两端折叠展开、连接带几何和真实 CodeMirror DecorationSet。
Node 环境没有浏览器 DOM，不代表 EditorView 挂载、按钮、焦点、拖动或实际同步滚动已验证。
性能数据及实际命令结果见 [phase2-diff.md](perf/phase2-diff.md)。
Gallery 的 Diff 标签展示内置 50/51 行数据，包含修改/删除/插入/折叠/缺少末尾换行；
Gallery 示例仅在开发构建加载；正式 shell 的 Preview 使用真实仓库 IPC。


### Diff 预览 IPC

- `worktree_diff(generation,path,area)`：path 沿用 RepoPath 的 bytes/display，area 为 staged/unstaged/untracked；只按 bytes 查找真实 status，原路径从 Rust 状态取得。
- `commit_diff(generation,hash,path)`：hash 经 validate_hash，路径必须属于 commit_files，重命名原路径由该列表取得；第一父或空端。
- `containing_branches(generation,hash)`：后台 for-each-ref --contains，最多 200 项及 total。

三项均有显式 ACL，完成后检查 generation。Diff 的文件读取及比较放在 spawn_blocking；非文本两端均不传仓库文本。UI 选择/刷新 effect cleanup 丢弃旧结果，仅当前文件保持两端 EditorView。相同文件刷新保留像素滚动；更换文件/区域释放旧视图。规则与未验证项见 interaction-spec/diff.md。

## 阶段 3：选择、写操作与撤销

本节更新上文阶段 0/2 的只读或 probe 描述；既有只读区域 API 仍保留。
`repo::inclusion` 描述前端会话的节点规则和带 token 的块选择，完整 status 和树留在 Rust。
`repository_changes` 的可见页增加 inclusion states/count；`inclusion_states` 在 Space 多选时
按最多 200 个 ID 聚合，避免依赖当前可见页或把完整文件列表传给前端。

新增显式 ACL IPC：`selection_preview(generation,path)`、
`inclusion_states(generation,selection,ids)`、
`commit(generation,selection,message,amend,expected)`、`cancel_write(generation)`、
`last_commit_message(generation)`、`commit_message_history(generation)`、
`rollback_files(generation,ids,start,end)`、`rollback(generation,ids,deleteAdded,expected)`、
`rollback_hunk(generation,hunk)`。不接受前端自造 patch、环境变量或任意 Git 命令。

Commit 预览以 HEAD ↔ 工作区为比较端；Rust 保留 `diff::compare` 本次实际生成的块区间，
提交前按原始路径重新读取内容并核对 token。`diff::selected_text` 以左侧原文保留未选块，
仅用右侧替换选中块；`unified_patch` 用 similar 3.2.1 生成带 context、CRLF 和无末尾换行
标记的统一差异，路径逐字节 C-style octal 引用。已有文本文件（含重命名、mode 改变）支持分块；新增、删除和非文本内容走已有 whole-file `git add -A --` 路径。前端不生成补丁。

`ops::snapshot` 流式对 status/HEAD、真实 index bytes、所有变更路径的文件 bytes、mode 和
symlink 文本计算指纹；status 读取前后也核对一致性。`CommitSelection.expected` 是调用方
可选的状态前提，正式会话必传；直接 core 调用仍在 prepare/commit 前捕获和检查快照。
成功后检查原始路径及新出现的变更、HEAD 和 index；出现冲突返回独立 recovery，保留快照
和说明。非 UTF-8 display 不用于命令参数。

Lease 共享独立写状态：拒绝第二个写操作；提交、文件撤销和块撤销在 ops 中共用进程级
串行锁。取消写 token 与历史读取 generation 分离，写运行时禁止重置 generation；UI 合并
监听刷新，任务结束后处理。关闭会话会发出取消，已完成写操作的结果不会因读取刷新被丢弃。
Unix 的现有 runner 终止整个私有进程组；Windows 仍只有直接 Git 子进程取消保证。

Rollback 用独占真实 index.lock + alternate index；`restore --staged --worktree
--source=<pinned HEAD> -- <paths>`，新增路径 `update-index --force-remove`。默认保留新增
工作文件，显式选择才删除；重命名恢复原路径并删除目标。普通文本块 rollback 在真实
index 锁内对工作区执行 reverse patch check/apply，index 保留。前后检查不是跨文件原子
事务；中断后的部分工作区恢复和绕过锁的外部直接改写需要用户检查，不能假称可自动回滚。

详细交互、源码已读/未核对项目和验证边界见 [commit.md](interaction-spec/commit.md) 与
[rollback.md](interaction-spec/rollback.md)。CommitSession 负责前端任务/表单和结果三态，
Svelte 原生 dialog 展示确认与全局 recovery；无额外 WebView、轮询或新增依赖。

部分重命名额外记录 `CommitSelection.renames` 的 old/new 原始路径。生成补丁时使用
`rename from/to` 和真实 old/new mode。对齐时只在临时 index 内把 HEAD 基线的 old entry
移动到 new，将原始 index 尚未改名的 entry 同样移动；已经暂存的 rename 保留其 blob/mode。
再以改名后的基线生成内容差异并三方应用，避免把“旧名删除 + 新名新增”错误地当作
add/add 或 delete/modify 冲突，也不消耗未选中的暂存文本。mode 由 Git 元数据进入补丁；
没有进入本次提交的原有暂存 mode delta 仍保留。真实 index 仍只在提交成功并全部检查后安装。

## 阶段 4 后端：分支与远程同步

本节描述本次后端实现；不表示 UI、IDEA 2025.3 实机对照或跨平台验收完成。
没有新增依赖、凭据存储、通用命令 IPC 或网络轮询。

### IPC 与结果

所有写 IPC 接受 `generation` 和 `expected`（status fingerprint），复用 Lease 的
独立写 token 和 ops 进程级串行 gate；拒绝重叠、旧快照与已存在的 index.lock。
写完成（包括错误）发送 `repository-refresh`，status/refs/history 均为 true。

| IPC | 额外参数 |
| --- | --- |
| create_branch | name, start?, checkout |
| checkout_branch | name, mode: normal/smart/force |
| checkout_remote_branch | remote, branch, mode |
| rename_branch | name, newName |
| delete_branch | name, force |
| set_upstream | name, upstream?（完整 refs 名；null 取消） |
| fetch | remote?, all |
| update_project | strategy?: merge/rebase；省略时从当前分支的 branch.rebase、pull.rebase 更新 |
| abort_update | stash?（本次 update 返回的 stash 对象 ID） |
| push_preview | options；只读，无 expected，完成后核对 generation |
| push | options: remote?, branch?, forceWithLease, tags |
| commit_and_push | selection, message, amend, options |

分支结果 `kind`：completed、localChanges(files: RepoPath[])、notMerged(name)、
stashRecovery(stash, files, detail)。路径仍保留原始 bytes 与 display。
远程结果：completed、push(success, refs)、conflicts(files, stash?)、
stashRecovery(stash, files)、interrupted(stash?, error)。
Push refs 使用 porcelain 的 flag/source/target/summary；`!` 表示失败，保留每个 ref
结果以呈现部分成功。新增 rejection 字段：null、needsUpdate、remoteRule、staleLease、other；summary 保留脱敏原文。
PushPreview 返回 remote/branch/commits（提交对象 ID）和 total/start；会话保存完整列表，IPC 每页最多 200 条。push_preview 增加可选 start/limit；start=0 刷新快照，后续页读取缓存。
CommitPushResult 返回 commit 与可空 push；提交失败返回原错误，不调用 push；
index recovery 时 push 为 null；成功后推送拒绝保留 commit 与 push 两段结果。

`remote_progress` 载荷 `[generation, message]`：Git stderr 的 CR/LF 完整记录（包含
--progress 输出），默认 15 秒（RunOptions.heartbeat_interval 可配置）无 stderr 时发 `remote.noOutput`，没有默认操作超时。
可选 RunOptions timeout 仍有效，取消继续使用进程组。无持续空闲任务。

### 命令及 IDEA 源码依据

源码路径相对 `/tmp/intellij-community/plugins/git4idea/backend/src/`；本地 master
快照不等于 IDEA 2025.3 tag。这里是行为参照，未复制上游代码。

| 操作 | Git 参数数组 | 依据 |
| --- | --- | --- |
| 校验 / 新建 | check-ref-format --branch name；rev-parse --verify --end-of-options start^{commit}；branch name pinned / checkout -b name pinned -- | branch/GitCheckoutOperation.java；commands/GitImpl.java |
| 检出 / 强制 | checkout name -- / checkout --force name -- | branch/GitCheckoutOperation.java checkoutOrNotify(force) |
| 远程检出 | checkout --track -b branch refs/remotes/remote/branch -- | commands/GitImpl.java checkout；使用远程 branch 短名，重名时 Git 拒绝，不覆盖 |
| 重命名 / 删除 | branch -m old new；branch -d name / 单独 force 调用 -D | branch/GitDeleteBranchOperation.java 的未合并确认流程 |
| upstream | branch --set-upstream-to fullRef name / --unset-upstream name | update/GitUpdateExecutionProcess.kt |
| Smart Checkout | stash push -m message；checkout；stash apply oid；stash list --format=%H 匹配 hash；apply 后重新定位并复核索引才 drop | branch/GitCheckoutOperation.java smartCheckout；stash/GitStashChangesSaver.java；commands/GitImpl.java stashSave/stashPop |
| Fetch | fetch --prune --progress remote / --all | fetch/GitFetchSupportImpl.kt；commands/GitImpl.java fetch |
| Update | fetch；stash push -m message；merge --no-edit upstream / rebase upstream；stash apply oid | update/GitUpdater.java；默认先 branch.name.rebase，再 pull.rebase，回退 merge；本项目不实现 shelve |
| Abort | merge --abort / rebase --abort；有保存的 stash 时恢复 | update/GitMergeUpdater.java、GitRebaseUpdater.java；本项目不做冲突解决 |
| Preview | log --format=%H HEAD ^remoteRef；无该 ref 时 HEAD --not --remotes | push/GitPushNativeResultParser.java 为输出协议依据；待推送集合语义尚需更完整 IDEA 对照 |
| Push | push --porcelain --progress --set-upstream [--force-with-lease] [--tags] remote HEAD:refs/heads/branch | push/GitPushNativeResultParser.java；目标优先显式值、upstream，首次回退同名 |

IDEA `GitImpl.stashSave` 不加 --include-untracked，stashPop 不加 --index；本实现同样
只保存跟踪文件，不恢复原先暂存划分。冲突保留 stash 对象 ID。取消发生于 stash 保存
途中时返回保留对象和 interrupted/recovery，避免把它当作普通可重试失败。

本地写指纹排除 branch.ab/upstream 头，避免自身 fetch/set-upstream 误报；仍包括 HEAD、
index、状态及变更文件内容。检查与写入并非跨工作区/ref 原子事务；无法阻止外部工具绕过
锁直接改文件。stash 按 hash 定位、apply、删除前复核索引；最终复核到 drop 仍存在外部插入/弹出窗口（Git drop 无 hash 条件更新），不能宣称
外部并发保护已完整验收。更新中断时保留活动 merge/rebase 与 stash，交由 abort 或终端处理。

### 认证、SSH 与 v0.1 限制

所有远程传输显式 `GIT_TERMINAL_PROMPT=0`，保留 credential helper、SSH config、
core.sshCommand、GIT_SSH/GIT_SSH_COMMAND。仅当用户没有设置 SSH command/helper 时使用
`ssh -o BatchMode=yes`；不加 StrictHostKeyChecking=no，不自动信任未知主机。
BatchMode 不允许密码/私钥口令交互，未知或改变的主机密钥也会失败。
用户自定义 SSH/helper 仍可能弹窗或挂起；不改写这些命令，用户可取消。

RemoteError 是 key/detail/guidance：remote.authentication、remote.hostKey、
remote.unreachable、remote.timeout、remote.cancelled、remote.failed；LC_ALL=C 下按
稳定 stderr 特征与退出码分类。guidance 为 remote.tryFetchInTerminal：请在终端执行一次
`git fetch`，保存凭据、解锁密钥或核对并确认主机密钥后重试。不要跳过主机密钥核验。
Git stderr、porcelain summary、进度事件中的 URL userinfo 脱敏；不记录原 URL。

v0.1 不提供交互凭据输入；后续通过受控 GIT_ASKPASS 辅助程序和一次性 IPC 请求实现，
不得把密码传回普通日志或持久化。真实 Keychain、SSH agent、主机密钥与跨平台行为本次
没有联网验证。Windows 的进程组取消仍是既有已知限制。

本次新增真实临时仓库测试见 `tests/branch.rs`、`tests/remote.rs`（位于 gitwood-core）。
远端仅本地 bare，不访问真实主机。新增回归覆盖 pre-receive 拒绝、update hook 部分 ref 拒绝、fetch first 和 stale lease、205 条会话分页、file:// 权限拒绝、FIFO 同步取消进程组、静默 heartbeat 和重试，以及 post-checkout/post-merge 外部插入 stash。
pre-receive 返回非零会拒绝整批推送，因此部分 ref 成功用 update hook 构造。
尚未覆盖 Windows 进程树取消、真实网络认证，以及最终 stash 复核到 drop 间的原子性。
