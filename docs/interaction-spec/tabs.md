# Tabs

1. **入口**：调用方提供 tabs、activeId、onchange 及可选 onclose；标签名称和关闭按钮名称由调用方本地化。macOS：Cmd+Shift+[/]、Ctrl+Left/Right 切换；Cmd+W / Ctrl+Shift+F4 关闭可关闭的当前标签。Windows/Linux keymap 留空，普通导航可用。
2. **交互行为**：单击标签切换，Left/Right 在可用标签中循环，Home/End 首尾，切换后焦点跟随；禁用项跳过。Tab 通过活动标签进入内容，关闭按钮为独立 native button，点击关闭交给调用方。ARIA tablist/tab/tabpanel、selected、controls/labelledby；仅一个活动 panel。无多选、speed search、右键与双击动作。
3. **对话框与反馈**：Enter/Space 激活 native tab 按钮；Esc 交上层，关闭不隐含确认框；需要未保存内容确认的业务由 onclose 决定。keymap 作用于此 Tabs 及 panel 内未消费的按键，默认不接管输入控件。无业务成功提示。
4. **状态恢复**：activeId 与 tabs 受控；onclose 后调用方选择相邻标签，组件在 DOM 更新后聚焦新的 active tab。panel snippet 切换是否销毁内容由调用方决定，需要保持独立页状态时应放在页外数据模型。无布局持久化。
5. **Git 影响与测试**：无。keymap 解析/分发纯函数由 `ui/tests/keymap.test.ts` 覆盖；点击、关闭后的焦点、disabled 与 panel 关系需应用内/E2E 验证。
6. **源码依据**：已核对 revision `0698a5fe297dfa08069f015cc423a6a85ed7fc60` 的 `platform/platform-resources/src/keymaps/Mac OS X 10.5+.xml`，NextTab、PreviousTab、CloseContent、CloseActiveTab。控件参照路径 `platform/platform-api/src/com/intellij/ui/components/JBTabbedPane.java` / `platform/platform-impl/src/com/intellij/ui/TabbedPaneWrapper.java`，具体鼠标及焦点策略**未核对源码**，需录屏对照。
7. **交付范围**：P0 / v0.1，阶段 1 基础控件。

实现状态：**已实现**；IDEA 录屏对照：未验证；真实 Git：不适用。
