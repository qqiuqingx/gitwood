# 交互清单索引

P0 目标为 v0.1 / macOS。阶段状态以 `../implementation-plan.md` 为准；本索引不宣称阶段整体完成。
本次控件为独立生产组件，开发构建 #gallery 为手动验证入口。实现不等于 IDEA 对照或真实 Git 验证。

| P0 条目 | 交付位置/阶段 | 当前证据 |
| --- | --- | --- |
| List：虚拟滚动、选择、焦点与键盘 | [list.md](list.md)，阶段 1 | 已实现；纯函数测试 |
| Tree：展开、折叠、选择与可见节点 | [tree.md](tree.md)，阶段 1 | 已实现；纯函数测试 |
| SpeedSearch：匹配、高亮、跳转 | [speed-search.md](speed-search.md)，阶段 1 | 已核对默认配置 matcher 源码；纯函数测试 |
| Popup：搜索、分组、子菜单、默认按钮与焦点 | [popup.md](popup.md)，阶段 1 | 已实现；定位纯函数测试 |
| Toolbar：动作、开关、tooltip、禁用态 | [toolbar.md](toolbar.md)，阶段 1 | 已实现；静态检查 |
| Tabs：切换与关闭 | [tabs.md](tabs.md)，阶段 1 | 已实现；静态检查 |
| 平台 keymap 与 WebView 冲突处理 | [keymap.md](keymap.md)，阶段 1 | 已实现 macOS 子集；纯函数测试 |
| Splitter、六处分隔线、最小尺寸 | 阶段 1，并行任务 | 本任务未验收 |
| 工具窗口显隐、焦点与布局记忆 | 阶段 1，并行任务 | 本任务未验收 |
| Light/Dark/Walnut、跟随系统、切换保留状态 | 阶段 1，并行任务 | 本任务未验收 |
| IDEA 提交工作方式：预览、暂存区、Changelist/Shelf 配置 | 阶段 1 第 2 步 | 待人工对照 |
| 打开/最近/切换仓库、worktree、Git 路径与 safe.directory | [browse.md](browse.md)，阶段 2 | 已实现 |
| 仓库状态与特殊文件名、文件状态反馈 | [browse.md](browse.md)，阶段 2 | 已实现 |
| 分支/提交/文件/详情/Diff 联动和选择恢复 | 阶段 2 | 后续建立详细条目 |
| 分支列表、HEAD、远程与跟踪关系 | [browse.md](browse.md)，阶段 2 | 已实现 |
| 历史、分页、说明/hash/分支/作者/日期/路径过滤 | [browse.md](browse.md)，阶段 2 | 已实现 |
| 提交图：泳道、合并线、颜色、HEAD 与标签 | [browse.md](browse.md)，阶段 2 | 已实现 |
| 只读 Diff：双栏、词高亮、F7/Shift+F7、同步滚动、折叠 | 阶段 2 | 后续建立详细条目 |
| Diff：空文件/二进制/编码/无末尾换行、错误反馈 | 阶段 2 | 后续建立详细条目 |
| 文件监听、取消旧请求、刷新后选择/滚动恢复 | [browse.md](browse.md)，阶段 2 | 已实现 |
| 提交面板：Changes/Unversioned、只读选择、目录分组 | [browse.md](browse.md)，阶段 2 | 已实现 |
| 提交面板：勾选、提交、Diff 预览接线 | [commit.md](commit.md)，阶段 3 | 已实现 |
| 文件/差异块部分提交、保留原有暂存内容 | [commit.md](commit.md)，阶段 3 | 已实现 |
| 提交说明/历史、Amend、Commit；Push 禁用 | [commit.md](commit.md)，阶段 3/4 | 已实现 |
| Diff 差异块撤销箭头、撤销变更及确认范围 | [rollback.md](rollback.md)，阶段 3 | 已实现 |
| hook/签名失败、Git 锁及外部并发后的恢复 | [commit.md](commit.md)，阶段 3 | 已实现 |
| 分支弹窗：搜索、分组、子菜单、启用条件 | [branch-popup.md](branch-popup.md)，阶段 4 | 已实现 |
| 新建/切换/重命名/删除分支、跟踪远程分支 | [branch-dialogs.md](branch-dialogs.md)，阶段 4 | 已实现 |
| Fetch/Pull/Update/Push：目标、策略、待推送提交 | [push.md](push.md)、[update.md](update.md)，阶段 4 | 已实现 |
| 分叉/本地修改/冲突引导与中止 | [branch-dialogs.md](branch-dialogs.md)、[update.md](update.md)，阶段 4 | 已实现 |
| 认证取消/网络失败/拒绝推送/缺少上游的反馈 | [push.md](push.md)，阶段 4 | 已实现 |
| 业务右键菜单：分组、文字、启用条件、键盘入口 | 各阶段对应业务 | 后续建立详细条目 |
| 后台进度、通知、错误对话框、成功/失败/取消的状态恢复 | 各阶段对应业务 | 后续建立详细条目 |

所有本次条目均未进行 IDEA 人工录屏对照；没有标记“已与 IDEA 对照”。阶段 2 浏览已接真实 Git；各条目状态仍只标实现，未完成应用内人工验收。

P1 名称：三方合并与冲突解决、Cherry-pick、Revert、Reset、Stash、标签、从提交新建分支、检出历史版本、历史与工作区比较、单文件历史、Blame、提交图多选与组合过滤、用户主题与热重载/预览/对比度、Windows/Linux keymap 与平台差异、辅助技术验收。

P2 名称：交互式 Rebase、Shelf/Changelist 对应能力、补丁创建与应用、克隆、远端管理、IntelliJ 主题导入。

| 阶段 2 收尾 P0 | 交付 | 状态 |
| --- | --- | --- |
| 工作区/提交文件 Diff、预览、区域切换及刷新 | [diff.md](diff.md) | 已实现 |
| 包含提交的分支异步查询与折叠 | [browse.md](browse.md) | 已实现 |
