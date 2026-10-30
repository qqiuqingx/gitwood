# Push — P0 / 阶段 4

| 条目 | 行为与启用条件 | 状态 |
| --- | --- | --- |
| 入口 | Branch Popup、顶部 Push 按钮和 Commit and Push…；原生 Git 菜单的 Push… 使用 Cmd+Shift+K（Windows/Linux 为 Ctrl+Shift+K），依据 `platform/dvcs-impl/resources/intellij.platform.vcs.dvcs.impl.xml:79` 与 `MacOSDefaultKeymap.kt`。 | 已实现 |
| 目标与预览 | 默认使用当前分支 upstream；remote/branch 可编辑。`push_preview` 按最多 200 个提交分页，目标新分支由 branch 输入值表达，列表选择只保存提交 ID。 | 已实现 |
| 选项 | Force Push (with lease) 默认关闭；Push Tags 独立勾选。 | 已实现 |
| 反馈 | `remote_progress` 驱动进度和取消；成功/拒绝分别提示，拒绝保留分类（needsUpdate、remoteRule、staleLease、other），建议先 Update。 | 已实现 |
| 认证/网络 | 显示 authentication、hostKey、unreachable、timeout、cancelled、failed 分类和终端 `git fetch` 指引；v0.1 不提供凭据输入对话框。 | 已实现 |

PushPreview 当前只返回提交 ID，右侧不猜测变更文件；完整文件树仍通过 Git Log 查看。这是后端契约边界，不是成功提示替代真实数据。
