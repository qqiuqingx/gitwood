# Branch Dialogs — P0 / 阶段 4

| 条目 | 行为与启用条件 | 状态 |
| --- | --- | --- |
| New Branch | 名称即时校验；Start point 可选；Checkout branch 默认选中；无效名称禁用 Create。 | 已实现 |
| Rename | 当前本地分支可重命名，保留输入和错误状态。 | 已实现 |
| Delete | 普通删除调用 `branch -d`；未合并分支要求用户勾选 Force 后二次确认，当前分支禁用删除。 | 已实现 |
| Checkout local changes | Normal、Smart Checkout、Force Checkout、Cancel；后端返回受影响文件时在结果区展示。 | 已实现 |
| Unstash conflict | stash recovery/冲突结果保留 stash ID 和恢复提示；冲突解决本身属于 v0.2。 | 已实现 |

对话框是应用内自绘 `<dialog>`，不打开新窗口；ESC/取消不写仓库，完成后由 `repository-refresh` 恢复分支树、提交图和 Commit 面板选择。
