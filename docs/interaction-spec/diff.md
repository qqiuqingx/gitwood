# 只读 Diff — P0 / 阶段 2

状态：已实现。未标记 IDEA 人工对照通过。

- Commit 单击或键盘选文件即预览，采用已确认原型 selectFile 行为；IDEA Preview Diff 开关和本机 staging-area 配置未人工核对。无暂存动作。
- 工作区区域：Staged 为 HEAD ↔ index，Unstaged 为 index ↔ 工作区，Untracked 为空 ↔ 工作区。同文件同时有两类修改默认 Unstaged，提供 Staged/Unstaged 切换。这是本次区域契约，尚无证据声称与 IDEA 当前配置一致。
- 提交文件：第一父 ↔ 提交；根提交为空端，合并提交按第一父。新增/删除缺失端为空，重命名左侧用后端原始路径，冲突只提示。
- 顶部路径与版本标签；上一处/下一处复用 DiffView F7/Shift+F7；切换销毁旧 EditorView，status 刷新重新加载当前工作区文件，保留同文件像素滚动。
- `$default.xml` 中 `Diff.ShowDiff` 为 **Ctrl+D**；macOS 由 `platform/platform-impl/src/com/intellij/openapi/keymap/impl/MacOSDefaultKeymap.kt` 的 `mapModifiers` 将 Ctrl 与 Meta 互换，绑定为 **Cmd+D**，已按该规则核对。
- 过期响应：effect cleanup 丢弃旧选择/版本/generation；Rust 完成读取后再次核对会话。
- 二进制、无法解码、过大显示专门状态，不比较；无三方合并、无差异应用写操作。

未验证：原生 WebView 焦点/滚动、IDEA 录屏、Windows/Linux。

阶段 3 更新：Commit 默认增加 HEAD ↔ 工作区预览及文本差异块包含 gutter、撤销箭头（含重命名和 mode 变化）；新增和删除文件整体勾选。既有 Staged/Unstaged 和历史 Diff 保持只读。详见 [commit.md](commit.md)、[rollback.md](rollback.md)，状态：已实现。
