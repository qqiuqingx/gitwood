# Rollback — P0 / 阶段 3

| 条目 | 入口、确认、执行与恢复 | 状态 |
| --- | --- | --- |
| 文件和目录 Rollback | Commit 工具栏 Rollback…；针对树中高亮选中的节点，独立于本次提交勾选。目录在 Rust 展开为真实变更文件。确认列表虚拟滚动，每页至多 200 条；重命名列出原路径 → 新路径 | 已实现 |
| 确认与新增文件 | 模态确认列出将撤销的路径；默认不选“删除本地新增文件”。Cancel/Esc 返回之前焦点，不写 Git。选择删除时只删除确认范围的新增/未跟踪文件，不递归清理目录 | 已实现 |
| 已暂存和未暂存文件 | 同时恢复 HEAD 中的 index 与工作区版本；删除文件重新出现。新增文件从 index 移除，默认保留工作区；重命名恢复源路径并删除目标路径，不受新增文件选项控制。初始仓库可取消新增文件暂存 | 已实现 |
| 差异块 Rollback | 索引将“Diff 差异块应用箭头、撤销变更”列为 P0，因此本阶段实现普通文本块的 ↶ 和 Rollback lines；均先确认。撤销 HEAD ↔ 工作区所选块，只改工作区，保留 index 及其他块 | 已实现 |
| 并发与锁 | 与提交共用会话写门和 ops 串行锁。检查快照、锁、HEAD 和真实 index；外部锁不删除。撤销前核对确认时指纹；外部状态变化时拒绝。命令执行后的检查失败显示错误并保留界面选择 | 已实现 |
| 进度、取消和错误 | 全局任务进度、取消进程组、完整错误诊断。Git restore/apply 中断可能已恢复部分路径，提示检查当前状态；不伪造成功或自动重试。index 的安装失败保持真实 index，不覆盖外部 index | 已实现 |

## 快捷键与源码依据

本地快照 revision 同 `commit.md`。

- `$default.xml` 中为 **Ctrl+Alt+Z**；`platform/platform-impl/src/com/intellij/openapi/keymap/impl/MacOSDefaultKeymap.kt` 的 `mapModifiers` 将 Ctrl 与 Meta 互换，macOS 绑定为 **Cmd+Option+Z**，已按该规则核对。使用 keymap registry 的物理 KeyZ 匹配，处理 Option 将 event.key 变为 Ω 的情形。
- 已读取 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/RollbackChangesDialog.kt`：确认范围来自 includedChanges，`delete.locally.added.files` 通过 `PropertiesComponent.isTrueValue` 读取，所以首次默认 false。IDEA 会持久化该偏好；Gitwood 本阶段每次打开确认框默认 false。
- **未核对源码**：Git tree 中实际位置是 `plugins/git4idea/backend/src/rollback/GitRollbackEnvironment.java`，不是旧版 `src/git4idea/rollback`。blob 离线缺失，禁用 lazy fetch 后无法读取；Gitwood 的 staged/unstaged/rename 恢复策略采用下述明确 Git 语义，需补 IDEA 源码和运行时对照。
- **未核对源码**：partial line rollback 的 LineStatusTracker renderer。当前已有普通文本文件（含重命名及 mode 变化）支持内容块撤销，名称和 mode 元数据保持；其他变化走文件确认。

## 实际 Git 命令

执行时持有真实 `<per-worktree index>.lock`，复制真实 index 到私有临时 index，设置 `GIT_INDEX_FILE`：

```text
git update-index --force-remove -- <每个新增/重命名目标路径>
git restore --staged --worktree --source=<确认时固定的 HEAD hash> -- <受影响的已有路径和重命名源路径>
```

真实 index 在检查后通过 held index.lock 原子安装。新增文件删除使用 Rust `remove_file` 对已验证的单个路径执行；重命名先移除目标再恢复源，以处理大小写不敏感平台的 case-only rename。不存在 `git clean` 或目录递归删除。

块级操作先从后端 compare 的区间生成 HEAD-relative patch，再执行：

```text
git apply --reverse --check --whitespace=nowarn
git apply --reverse --whitespace=nowarn
```

补丁经 stdin 传原始 bytes。路径参数始终是 `--` 后的独立 OsString，开启 `GIT_LITERAL_PATHSPECS=1`，不使用 shell。检查相对路径及父目录，拒绝通过父 symlink 逃出仓库；文件名 display 不参与路径解析。

## 测试与限制

`commit_session.rs` 真实仓库覆盖修改/新增/未跟踪/删除/已暂存文件、初始仓库、目录范围、重命名、以 `-` 开头路径、锁冲突、外部内容变化、部分块保留其余内容。特殊字节路径与 CRLF patch 由同一文件内的提交测试及现有 filenames/parser 测试覆盖。

并发保护是执行前后检测加 Git index/ref 锁，不是任意外部编辑器之间的跨文件原子事务；无法锁住恶意进程绕过 Git 锁的直接文件/ref 改写，也不能承诺逆转 hooks 的任意副作用。若外部进程恰好在最终检查后改写同一个工作文件，仍存在系统级竞态。未做原生 WebView、IDEA 录屏、Windows/Linux、最低 Git 版本运行验收。
