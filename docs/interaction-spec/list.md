# List

1. **入口**：P0 基础列表；调用方提供 `rowCount + getRow(index)`、可访问名称。内容由业务决定；macOS 另支持 Opt+↑/↓、Opt+Shift+↑/↓ 到首尾并扩选。普通导航适用于全部平台；Windows keymap 尚未接入。
2. **交互行为**：单击选择并聚焦；双击、Enter 调用 activate。多选模式 Shift 以锚点连续扩选，Cmd/Ctrl 切换单项，两者组合追加范围；单选模式忽略多选修饰键。↑/↓、Home/End、PageUp/PageDown 导航并滚入可见区；禁用项、标题、分隔线跳过。右键保留已有多选，点未选中行先单选，再交由调用方打开菜单。焦点停留在 listbox，以 `aria-activedescendant` 指向当前已渲染 option；提供 `aria-selected` 和总条目数。失焦使用非焦点选中色。
3. **对话框与反馈**：输入直接开启 [SpeedSearch](speed-search.md)；匹配规则、片段高亮、无匹配反馈、上下键跳转详见该条目。Esc 先关闭搜索并保留选择；没有搜索时交给外层。Enter 关闭搜索并激活当前行；没有选中项时无动作。组件不显示业务成功/失败通知。
4. **状态恢复**：选择按稳定 ID 持有，可绑定到调用方；activeIndex 和滚动位置存活于实例。固定行高从 CSS 测量，ResizeObserver 响应容器及主题尺寸变化，不重建列表。仅渲染可见范围加上下各 3 行；手动滚动使 active 行离开 DOM 时暂不设置 activedescendant。调用方刷新/重排数据时负责同步 activeIndex；源身份切换应新建实例。无持久化。
5. **Git 影响与测试**：不调用 Git。`ui/tests/list.test.ts` 覆盖十万行范围、边界、部分行、选择/范围/切换/禁用行和翻页；SpeedSearch 另测。DOM 焦点、滚动、辅助技术与双击需应用内或后续 E2E 验证。
6. **源码依据**：intellij-community revision `0698a5fe297dfa08069f015cc423a6a85ed7fc60`，已读 `platform/platform-impl/src/com/intellij/ui/ListSpeedSearch.java`、`SpeedSearchBase.java`、`SpeedSearchComparator.java`；首尾快捷键见 `platform/platform-resources/src/keymaps/Mac OS X 10.5+.xml` 的 `List-selectFirstRow` 等。Swing 基础选择模型未核对源码，当前 Shift/Cmd 行为需人工对照。
7. **交付范围**：P0 / v0.1，阶段 1 基础控件，未挂接业务列表。

实现状态：**已实现**（`ui/src/components/List.svelte` 及生产辅助函数）；IDEA 录屏对照：未验证；真实 Git：不适用。
