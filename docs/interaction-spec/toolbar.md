# Toolbar

1. **入口**：通用工具栏，调用方提供本地化 label、图标 snippet、动作回调、快捷键展示、禁用/pressed 状态。各平台由调用方用已选 keymap 格式提供显示文字。
2. **交互行为**：单击 native button 执行动作；pressed 为受控开关状态；分隔线不可操作。Tab 可达所有按钮，Left/Right 循环切换可用按钮，Home/End 到首尾；禁用按钮跳过。悬停或焦点显示 tooltip，包含动作名称与快捷键。无列表多选、双击特效、右键菜单和 speed search。
3. **对话框与反馈**：Enter/Space 沿用 native button 激活，Esc 交由上层、不执行动作；离开悬停及焦点后隐藏 tooltip。禁用项保留可悬停提示。组件不显示业务成功或失败；回调负责反馈。
4. **状态恢复**：pressed、enabled 和实际动作由调用方控制；组件只保存瞬时焦点/悬停，不持久化。主题改变不重置开关。
5. **Git 影响与测试**：无内置 Git 行为。静态检查覆盖 Svelte/类型/ARIA；本任务没有 DOM 测试环境，按钮、tooltip 和键盘顺序需应用内/E2E 验证。
6. **源码依据**：参考路径 `platform/platform-api/src/com/intellij/ui/ToolbarDecorator.java`、`platform/platform-api/src/com/intellij/ui/ToolbarService.kt`；本次具体实现**未核对源码**，使用 native button 与可访问工具栏规则。IDEA 的真实图标、延迟 tooltip 和逐业务启用条件需后续录屏细化。
7. **交付范围**：P0 / v0.1，阶段 1 基础控件；业务动作不在本次范围。

实现状态：**已实现**；IDEA 录屏对照：未验证；真实 Git：不适用。
