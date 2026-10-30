# 阶段 2：只读仓库浏览（P0 / v0.1 macOS）

布局、栏目和菜单文案参照已确认原型。下列状态只表示代码已实现；未进行 IDEA 人工录屏对照或原生 WebView 验收。所有入口只读 Git，不勾选提交、不暂存、不检出分支。

| P0 条目 | 入口、行为与反馈 | 恢复与验证场景 | 状态 |
| --- | --- | --- | --- |
| 打开与最近仓库 | File → Open…（Cmd/Ctrl+O）调用目录选择器；取消不改变仓库；Recent Projects 和欢迎页使用真实最近记录；启动恢复最近成功项目 | 非仓库、bare、safe.directory 拒绝显示文本错误；不写 Git 配置；并发 open/close 排队，忽略旧响应 | 已实现 |
| Commit 只读文件树 | Changes / Unversioned Files；按目录分组，可切换平铺；同文件合并 staged/unstaged 提示；status.* 着色；冲突与重命名提示 | 单击选择、disclosure/双击展开、方向键与多选复用 Tree；输入即搜索已展开节点；Rust 分页；文件名含中文、空格、换行、非 UTF-8 bytes 不混淆 | 已实现 |
| Commit 工具栏 | Refresh、Group by Directory、Expand All、Collapse All；只读阶段 Amend/Commit/Commit and Push 禁用 | 刷新保留稳定文件 ID、展开与滚动；无提交勾选框 | 已实现 |
| 分支树 | HEAD (Current Branch)、Local、Remote/远端分组、Tags；当前分支标 HEAD、上游/跟踪信息；speed search | 单击叶子按完整 ref 过滤历史；只选目录不发历史请求；折叠、选择及滚动随 refs 刷新保持 | 已实现 |
| 提交列表 | 图、说明/ref 标签、作者、日期；鼠标/上下/首尾/翻页选择；只向后端取可见行 | hash 为选择 ID；End 可跳转未缓存页；上下半行连接；宽图支持水平滚动；主题或分隔线改变不读 Git | 已实现 |
| 过滤栏 | Text or hash 输入并确认/失焦；Branch 可搜索列表；User、Date、Paths 输入并 Apply/Clear；说明/hash 取并集，与其他过滤取交集 | 路径保留字节，放在 -- 后；无命中显示空状态；连续过滤串行接收新 generation，忽略旧选择详情/分页 | 已实现 |
| 过滤图 | 最近命中祖先相连，跳过中间节点为虚线；分叉保持可达路径、去重与拓扑顺序 | 真实临时仓库覆盖隔一个命中、分叉合并、无/全命中、路径加说明/作者 | 已实现 |
| 信息区文件树 | 上方显示所选提交的目录树；单击选择文件，方向键和搜索复用 Tree；重命名显示原路径 | 当前提交文件及目录索引在 Rust，窗口最多 200 节点；快速切换提交/展开/搜索会丢弃旧响应 | 已实现 |
| 信息区详情 | 完整说明、hash、作者/email、时间、父提交均纯文本；合并文件比较第一父，根提交比较空树 | 提交选择后并行/独立请求，旧详情不得覆盖新选择；无选择显示说明 | 已实现 |
| 区域刷新与布局 | status 仅刷新 Commit；refs 更新分支和标签；history 重算图；窗口焦点使用后端信号 | 根据所选 hash 和视口顶行 hash 重新定位；保留像素偏移及树展开集合；隐藏面板保留挂载状态 | 已实现 |
| 工具窗口 | 原生 View 菜单或左侧 Commit/Git 按钮显隐；分隔线沿用原壳设置 | 设置保存及菜单勾选同步；拖动/主题切换不重置数据 | 已实现 |

源码依据：intellij-community revision `0698a5fe297dfa08069f015cc423a6a85ed7fc60`。
过滤图为 `platform/vcs-log/graph/src/com/intellij/vcs/log/graph/impl/facade/FilteredController.kt`、
`collapsing/DottedFilterEdgesGenerator.kt`、`api/elements/GraphEdgeType.java`、
`impl/print/elements/EdgePrintElementImpl.kt`；日期列为
`platform/vcs-log/impl/src/com/intellij/vcs/log/ui/table/column/VcsLogDefaultColumn.kt`。
基础键盘、选择、speed search 的源码依据沿用 List/Tree 清单。

本地证据：`history_filtered.rs` 真实仓库测试；`browse.test.ts` 的绘制指令/日期/过滤/目录/刷新纯函数；
`browse-controller.test.ts` 的异步过期丢弃与区域刷新（单元测试替换 IPC，不算 E2E）。
未验收：IDEA 当前 staging-area 配置的人工对照、原生文件对话框/菜单、焦点、WebView 图像与滚动连续性、跨平台。

待办：包含提交的分支查询；中央 Diff 接线及文件双击预览；原生系统日期格式与 IDEA 人工对照。

## 阶段 2 收尾 P0（状态：已实现）

- Commit 文件单击/键盘选择及 Log 文件选择更新中央 Diff，仅请求当前文件。
- 包含提交的分支独立异步读取，展示完整本地/远端 ref；默认前三项，和另外 N 个可展开；服务端最多 200 项并返回总数。
- 依据：`platform/vcs-log/impl/src/com/intellij/vcs/log/ui/details/commit/CommitDetailsPanel.kt` 的 ContainingBranchesPanel。当前固定前三项，与上游按可用宽度折叠有差异，未人工对照。

原待办中中央 Diff 接线及 contains 查询已实现；真实 WebView 与 IDEA 人工验收仍未进行。

阶段 3 更新：Commit 的只读/禁用描述已由 [commit.md](commit.md) 和 [rollback.md](rollback.md) 的实现取代；文件浏览与 Staged/Unstaged 查看仍只读。
