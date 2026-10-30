# Tree

1. **入口**：P0 文件/分支分组树；source 按根/子节点索引读取，调用方提供稳定 ID、名称、是否有子节点、展开集合。适用 macOS / Windows / Linux 的普通方向键；macOS 首尾快捷键继承 List。
2. **交互行为**：复用 [List](list.md) 的虚拟滚动、鼠标/键盘多选和焦点机制。单击行只选中；单击 disclosure 展开/折叠并选择该行；双击分支切换展开，双击叶子激活。Right：闭合分支先展开，已展开分支进入第一直接子节点，叶子不动。Left：已展开分支先折叠，否则跳父节点，根不动。Shift 可扩选；右键行为继承 List（业务菜单暂未接入）。ARIA 为 tree/treeitem、level、expanded、posinset、setsize。
3. **对话框与反馈**：Enter 调用业务 activate（不默认展开）；Esc 先关闭搜索，随后由外层决定。输入即搜索，仅匹配当前可见节点；分词/驼峰规则和高亮、无匹配反馈见 [SpeedSearch](speed-search.md)。不递归搜索折叠子树、不触发 Git 操作。
4. **状态恢复**：展开状态由调用方持有，原因是后续异步子节点与仓库刷新需共享唯一状态源；本组件只发布新 Set。选择按 ID，活动节点按 ID 映射到当前可见行。展开集合变化只重算可见扁平描述符；隐藏子树不读取。外部折叠导致活动节点隐藏时活动项清空，调用方应按业务规则恢复祖先；本阶段不持久化。
5. **Git 影响与测试**：无。`ui/tests/tree.test.ts` 覆盖扁平次序、ARIA 数据、隐藏子树不读取、左右键规则、空树、5000 层迭代遍历；DOM 操作和外部刷新后的焦点需应用内/E2E 验证。
6. **源码依据**：同 revision `0698a5fe297dfa08069f015cc423a6a85ed7fc60`；已读 `platform/platform-impl/src/com/intellij/ui/TreeSpeedSearch.java`（`SpeedSearchComparator(false, true)`，默认不搜索折叠节点）、`TreeExpandCollapse.java`。`platform/ui.tree/src/com/intellij/ui/tree/ui/TreeAction.java` 文件本地缺失，读取触发 promisor fetch 后网络失败，左右键底层实现**未核对源码**，采用标准 Swing 树规则，待人工对照。
7. **交付范围**：P0 / v0.1，阶段 1；不含真实文件树或分支业务。

实现状态：**已实现**；IDEA 录屏对照：未验证；真实 Git：不适用。
