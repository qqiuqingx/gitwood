# Branch Popup — P0 / 阶段 4

| 条目 | 行为与启用条件 | 状态 |
| --- | --- | --- |
| 入口 | 窗口顶部当前分支控件打开同一 WebView 内的 Popup；状态栏/日志树继续复用当前分支数据。`Git.Branches` 在 Windows/Linux 为 Ctrl+Shift+`，来源 `$default.xml`；macOS keymap 显式移除该绑定。IDEA 的动作 ID 见 `GitQuickListContentProvider.java:24`。 | 已实现 |
| 分组与搜索 | Recent、Local、按远端分组的 Remote、Tags；Popup 复用 List/SpeedSearch，输入即搜索和高亮。当前分支显示 HEAD/ahead/behind。 | 已实现 |
| 键盘与焦点 | ↑/↓、Enter、Esc、Right/→ 子菜单、Left/← 返回；关闭后恢复入口焦点。依据 `ListPopupImpl.java` 的 Enter、Right/Left、`disposeAllParents` 行为，具体 WebView 焦点仍需人工验收。 | 已实现 |
| 分支子菜单 | Checkout、New Branch from、Update、Push、Rename、Delete；Compare、Diff、Rebase/Merge 和 Checkout-and-Rebase 以 P1 禁用标识。当前分支不能 Checkout/Delete。 | 已实现 |
| 新建/检出 | 新建默认勾选 Checkout；本地修改时提供 Smart/Force/Cancel；远程分支走 track checkout。 | 已实现 |

源码依据：本地 intellij-community master 快照，`plugins/git4idea/backend/src/actions/GitQuickListContentProvider.java:24`；弹窗通用行为见 [popup.md](popup.md)。该快照不是 IDEA 2025.3 tag，未做录屏对照。
