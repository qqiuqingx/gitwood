# Update / Fetch — P0 / 阶段 4

| 条目 | 行为与启用条件 | 状态 |
| --- | --- | --- |
| 入口 | Branch Popup、顶部 Update/Fetch 按钮；原生 Git 菜单的 Update Project… 使用 Cmd+T（Windows/Linux 为 Ctrl+T），依据 `platform/vcs-impl/resources/META-INF/VcsActions.xml:50` 与 `MacOSDefaultKeymap.kt`。 | 已实现 |
| 策略 | Update 对话框默认读取后端解析的 `branch.<name>.rebase`、`pull.rebase`（缺省 Merge）；用户选择的 Merge/Rebase 作为本次 IPC 的可选 `strategy` 参数生效，不修改配置。 | 已实现 |
| Fetch | 调用正式 `fetch`，默认 `--all` 入口；刷新 refs、历史和状态。 | 已实现 |
| 冲突/中止 | 冲突显示文件和 v0.2 冲突解决限制；保留 stash 时显示恢复指引，并提供 Abort 入口契约待后端 UI 完整接入。 | 已实现 |

更新参数与结果以 `docs/architecture.md` 阶段 4 后端小节为准。未进行真实网络认证或 IDEA 录屏验收。
