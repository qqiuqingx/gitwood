# Commit — P0 / 阶段 3

以下状态只表示已实现；未标记“已与 IDEA 对照”或“已用真实 Git 验证”。
本次以 Changes 工作方式提交，不提供独立 staging-area 写操作。

| 条目 | 入口、行为、焦点与恢复 | 状态 |
| --- | --- | --- |
| 文件和目录勾选 | Changes 默认包含，Unversioned Files 默认不包含；目录按全部后代聚合 all/some/none。单击行选择并预览，不改变勾选；复选框点击返回树焦点。空格统一切换所选节点，包括不在当前可见页的节点；方向键、Shift 多选、Cmd/Ctrl 切换多选和 speed search 沿用 Tree/List | 已实现 |
| 双击和键盘打开 Diff | 双击文件或 Enter 更新预览并聚焦中央 CodeMirror；目录双击展开/折叠。Cmd/Ctrl+D 聚焦预览。目录 disclosure 与复选框不打开 Diff | 已实现 |
| 刷新恢复 | 选择规则、原始路径 bytes 与差异块 token 留在前端会话；刷新保留选择规则、说明和原有浏览选择。文件/目录勾选规则按路径匹配新状态。过期差异块 token 不自动映射到新块，须重新选择 | 已实现 |
| 提交预览与差异块 | Commit 默认 HEAD ↔ 工作区；Staged/Unstaged 保留只读入口。普通文本已有文件（含重命名及 mode 变化），每个块在左侧 CodeMirror gutter 勾选；部分块对应文件/目录混合态。新增、删除、二进制或超限文件以整文件提交，不能按未经验证的文本块拆分 | 已实现 |
| 选择 → patch | 前端只传节点 inclusion rules 与 `{path,token,blocks}`；Rust 缓存本次 `diff::compare` 的精确区间，核对快照，再以左侧保留未选块、右侧替换选中块，生成 Git C-style 路径引用的 unified patch。没有前端拼补丁或任意 Git IPC | 已实现 |
| 提交说明 | 可编辑多行说明；空白说明禁止提交；非 Amend 时还需至少一个选中文件/块。Commit and Push… 保持禁用并显示阶段 4 暂不可用 | 已实现 |
| Amend | 勾选读取真实 HEAD 的完整说明，保存原草稿；取消恢复草稿。无初始提交时禁用。失败不改变 Amend 或说明；成功清空说明、原草稿并退出 Amend | 已实现 |
| Commit Message History | Ctrl+M 或按钮读取 HEAD 最近 25 个多行说明，与本次会话成功提交说明合并、去重；选中填入说明框并返回焦点。Esc 关闭并返回说明框。不将仓库说明写入 HTML | 已实现 |
| 提交执行和取消 | Commit 调用正式 `commit` IPC → Lease → `ops::partial_commit`；同一会话拒绝第二个写操作，ops 层所有提交/撤销共用串行锁。全局状态栏及面板显示后台任务和取消入口；取消 token 直接终止 Unix Git/hook/gpg 进程组，等待结果后刷新 | 已实现 |
| 成功 | 清空说明，刷新状态、refs、历史；其余文件内容、原有不相关暂存保持。保留文件选择规则、树状态和焦点；已有 hunk overrides 清除，剩余差异重新预览 | 已实现 |
| 提交完成但 index 待恢复 | 独立 recovery 结果；全局模态窗口明确“提交已完成，请勿重复提交”，列出 hash、恢复目录、原因、RECOVERY.md 内容。不会显示普通提交成功 | 已实现 |
| 失败或取消 | 保留说明、Amend 和包含规则；ErrorDialog 显示 Git stderr、hook/签名/锁/并发诊断。不确定结果显示检查 HEAD 后再决定的原诊断，不自动重试 | 已实现 |
| 外部并发 | status 返回一致性检查后的 fingerprint；执行前核对 status/HEAD/index/变更文件内容，独占真实 index.lock。执行期间阻止 history generation 重置写 token；成功后对固定原路径及新增变更、HEAD、index 再检查，异常返回 recovery | 已实现 |

## 源码依据及未核对项

本地 intellij-community revision `0698a5fe297dfa08069f015cc423a6a85ed7fc60`，是 master 快照，**不是 IDEA 2025.3 tag**。

- 已读取 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/ChangesTreeImpl.java`：showCheckboxes 构造器通过 `setIncludedChanges(changes)` 初始化已有 Changes。
- 已读取 `platform/platform-resources/src/keymaps/Mac OS X 10.5+.xml:491-498`：`Vcs.ShowMessageHistory = control M`、`Vcs.ToggleAmendCommitMode = control alt M`。因此 macOS 说明历史使用 **Ctrl+M**；Amend 使用 Ctrl+Option+M。Windows/Linux `$default.xml:1057-1064` 为 Alt+M / Ctrl+M。
- **未核对源码**：核心 `platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/ui/ChangesTree.java` 的 Space/双击/焦点细节、`PartialLocalLineStatusTracker.kt` 及对应 Diff inclusion renderer、`ChangesViewCommitMessagePolicy.kt` 的成功/失败说明恢复、`plugins/git4idea/backend/src/checkin/` 的完整提交流程。文件在 Git tree 中存在，但 blob 未离线缓存；`GIT_NO_LAZY_FETCH=1 sparse-checkout add ...` 返回缺失对象，未联网。上述行为按用户指定交互和既有 Tree/原型实现，不能宣称已与 IDEA 源码完全一致。
- 已确认的原型 `docs/prototype/idea-git-interaction-prototype.html`：单击预览、双击 Diff、默认未勾选未跟踪文件、多行说明和独立 Amend；其中演示用的假提交提示没有进入产品。

## 验证位置和边界

- `crates/gitwood-core/tests/commit_session.rs`：真实仓库，经正式 Lease 选择 → compare 区间 → patch → ops 的整条路径；暂存 A 只选 B、同文件分块、重命名分块及 mode 变化（含未选中的 mode 暂存）、删除/未跟踪/特殊文件名、CRLF/空文件/无末尾换行、初始提交/Amend、core.hooksPath/commit.template、hook 拒绝、签名失败、锁、过期块、写互斥、取消 hook/gpg、recovery。
- `partial_commit.rs` 保留原有 15 项事务测试，包括 split index、linked worktree、恢复目录及手工恢复命令。
- `ui/tests/commit.test.ts`：真实前端提交控制器的三态结果、失败保留说明和选择、Amend、刷新、历史和启用条件；`commit-render.test.ts` 检查真实组件 SSR 的三态 ARIA、说明纯文本及按钮状态；`keymap.test.ts` 检查 Ctrl+M 和 Option 修改后的物理按键。
- SSR/Node 测试不证明原生 DOM 的 indeterminate 属性、焦点、CodeMirror gutter 像素位置或实际鼠标/键盘事件。未启动 tauri dev；未做 IDEA 人工录屏、原生 WebView E2E、Windows/Linux 运行验收。历史当前为 Git 最近说明加前端会话记录，不实现 IDEA 的跨项目持久化历史设置。

## 取消测试的同步约束

`session_write_rejects_second_write_and_cancels_hook` 等待真实 hook/GPG 通过 `.git/ready-pipe`
发出的 FIFO 就绪事件，同时监听提交 future 提前结束；就绪后才断言同会话第二次提交/撤销
被拒绝，再发出取消并等待结果。取消后比较 HEAD、真实 index 和两份 diff，随后实际完成
一次新提交，验证写 guard/index.lock 都已释放。

原 3 秒轮询超时从请求开始计算，错误包含进程级 `ops::WRITES` 队列耗时；16 个真实仓库
测试并发时可在正常排队过程中超时。临时追踪复现了请求已入队但 3.001 秒时仍未取得锁、
index.lock 尚不存在，其他请求的排队耗时为 3.1–3.9 秒。此次修复为测试事件同步，没有修改
生产串行锁或延长时间常量，也没有留下追踪日志。FIFO 观察线程在操作提前失败时由 Drop
发出中止字节并回收，避免测试自身遗留等待线程。GitRunner 的进程组取消测试仍独立覆盖
SIGTERM-resistant 后代，使用管道 EOF 判断资源释放，避免用 kill -0 误判僵尸进程。

## 2026-10-30 本地验收记录

- `cargo fmt --all --check`、`cargo clippy --offline --workspace --all-targets -- -D warnings`、`git diff --check` 均无错误。
- 原始 `cargo test --offline --workspace` 连续执行 2 次，均仅 `watcher_real_events` 因沙箱无法启动 FSEvent stream 失败；取消测试均通过。
- `cargo test --offline --workspace -- --skip watcher_real_events` 连续执行 **5 次，每次 116 项通过**，仅显式排除上述 1 项。原进程组取消测试五次均通过。
- 高负载验证：每轮先启动独立 `cargo test --offline -p gitwood-core`，确认其进入真实仓库测试文件后，同时执行 `cargo test --offline -p gitwood-core --test commit_session`。连续 **3 轮，每轮 16 项通过**；三个负载进程也都仅遇到已知 FSEvents 失败。
- `pnpm -C ui check`：0 errors / 0 warnings；`pnpm -C ui test`：14 个文件、99 项通过。
- `CARGO_NET_OFFLINE=true pnpm tauri build --debug --no-bundle`：macOS 本机编译成功，产物 `target/debug/gitwood`；未启动 tauri dev 或运行原生 UI 人工验收。

本机临时验收日志：`/tmp/gitwood-phase3-verification.json` 汇总每轮退出码、耗时及日志位置；
`/tmp/gitwood-session-queue-diagnosis.log` 是修复前带临时追踪的失败复现。
新增测试同步代码只位于 tests，生产接口没有测试开关/构造器，也没有新增依赖或仓库提交。
