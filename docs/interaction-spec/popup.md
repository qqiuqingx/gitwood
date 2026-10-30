# Popup

1. **入口**：调用方按需挂载 Popup，传 HTMLElement 锚点或鼠标位置矩形、菜单项和关闭回调；关闭时卸载。只在当前 WebView 自绘，不开窗口。全部平台 Enter/Esc/方向键，无额外业务快捷键。
2. **交互行为**：下方对齐锚点，触边向上/横向翻转并夹到视口；子菜单从选中行右侧打开，右侧空间不足则向左。点击可用行执行，禁用项/分组标题/分隔线不选中；不多选。内含虚拟 List + SpeedSearch，输入即搜索、高亮和匹配跳转遵循 [SpeedSearch](speed-search.md)。Right 打开子菜单、Left 返回父级；每级复用同一个窗口中的组件。右键无额外动作。菜单结构变化由调用方提供新 props。
3. **对话框与反馈**：打开后聚焦首个可选项。Esc 先清搜索，再关闭当前层；点击外部关闭。Enter 执行可选的 defaultAction（启用时），未设 defaultAction 则执行当前项或打开子菜单；defaultAction 禁用时不执行其他项。执行子菜单叶子时关闭整条菜单链。没有成功提示，回调负责真实动作及错误反馈。无匹配时保留当前项并显示无匹配反馈；Enter 仍作用于当前项。
4. **状态恢复**：关闭/卸载后把焦点交回打开前且仍在 DOM 的元素，返回子菜单时回到父列表。各层独立保存选择与滚动；返回后不重建父层；resize/scroll/ResizeObserver 驱动定位，无轮询。调用方须在关闭回调中卸载组件，锚点祖先不应使用创建固定定位包含块的 transform/contain；当前无跨 DOM portal。视口特别小的时候限制最大尺寸，内部列表滚动。
5. **Git 影响与测试**：无内建 Git 写操作；回调由业务接入。`ui/tests/popup.test.ts` 覆盖锚点、鼠标、上下/左右翻转、越界夹取和巨型弹窗；多层关闭、焦点、默认按钮及 WebKit 层叠需应用内/E2E 验证。
6. **源码依据**：revision `0698a5fe297dfa08069f015cc423a6a85ed7fc60`；已读 `platform/platform-impl/src/com/intellij/ui/popup/list/ListPopupImpl.java` 的 Enter、ListActions.Right/Left、onChosen、disposeAllParents、分组选项选择。定位与焦点恢复为当前 WebView 适配，底层 PopupFactory 的具体屏幕策略未核对源码，需录屏验证。
7. **交付范围**：P0 / v0.1，阶段 1 通用 Popup；真实分支菜单项目、条件与 Git 行为属于阶段 4。

实现状态：**已实现**；IDEA 录屏对照：未验证；真实 Git：不适用。
