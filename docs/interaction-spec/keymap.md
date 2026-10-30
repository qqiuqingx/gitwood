# Keymap

1. **入口**：独立动作注册表 `action id → run/enabled/allowInInput`，作用域由 DOM keydown 冒泡决定；List、Tabs 内已使用，独立 Gallery 提供窗口动作集成示例。按 navigator.platform 选择 mac/windows/linux；原生菜单动作不重复注册。
2. **交互行为**：解析 IDEA XML 空格分隔的 keystroke，兼容 Cmd/Meta、Opt/Option/Alt、Ctrl/Control、Shift；精确匹配全部修饰键。ASCII 字母/数字/方括号使用 KeyboardEvent.code，避免 macOS Option/Shift 改写 key；其他键使用 key。选中 action 的 enabled 实时求值，未注册/禁用/输入控件中未获 allowInInput 的动作不接管。没有鼠标、多选和 speed search；文本输入交组件。
3. **对话框与反馈**：匹配到已注册且启用的动作才 preventDefault + stopPropagation，避免 WebView 默认关闭/导航重复发生；未接管、defaultPrevented、IME composing 的事件不阻断。Esc/Enter 的组件语义由 List/Popup 实现，不把 EditorEscape/EditorEnter 冒充列表动作。注册重复 ID 抛出明确错误，返回注销函数用于组件/窗口清理。
4. **状态恢复**：无全局单例、无空闲监听；动作表属于组件或窗口集成层，销毁时移除监听；快捷键不会代替布局/业务状态持有者。系统在 DOM 之前截获的快捷键无法通过 preventDefault 处理，需要原生菜单集成阶段核对。
5. **Git 影响与测试**：无。`ui/tests/keymap.test.ts` 覆盖修饰键、物理码、IME、平台选择、启用条件、冲突阻断、输入避让、注销和重复注册；实际 macOS WebView/系统快捷键需应用内/E2E 验证。Gallery 的窗口动作只更新演示区域，不声称已集成 App 的工具窗口。
6. **源码依据**：已解析本地 XML，revision `0698a5fe297dfa08069f015cc423a6a85ed7fc60`；`platform/platform-resources/src/keymaps/Mac OS X 10.5+.xml` 声明 parent="$default"。以下为已接入子集，源码 `ui/src/keymap/keymaps.ts` 逐项记录出处。

| Action ID | macOS 绑定（XML 原值） | 出处 | 调用路径 |
| --- | --- | --- | --- |
| List-selectFirstRow | alt up | Mac OS X 10.5+.xml | List |
| List-selectLastRow | alt down | 同上 | List |
| List-selectFirstRowExtendSelection | alt shift up | 同上 | List |
| List-selectLastRowExtendSelection | alt shift down | 同上 | List |
| NextTab | meta shift CLOSE_BRACKET / control RIGHT | 同上 | Tabs |
| PreviousTab | meta shift OPEN_BRACKET / control LEFT | 同上 | Tabs |
| CloseContent | meta W | 同上 | Tabs（可关闭时） |
| CloseActiveTab | control shift F4 | 同上 | Tabs（可关闭时） |
| ActivateVersionControlToolWindow | meta 9 | 同上 | 原生菜单（WebView 跳过） |
| ActivateCommitToolWindow | meta 0 | 同上 | 原生菜单（WebView 跳过） |
| Vcs.UpdateProject | meta T / control T | `platform/vcs-impl/resources/META-INF/VcsActions.xml:50`，macOS 经 `MacOSDefaultKeymap.kt` | 原生菜单（WebView 跳过） |
| CheckinProject | meta K / control K | `platform/vcs-impl/resources/META-INF/VcsActions.xml:11`，macOS 经 `MacOSDefaultKeymap.kt` | 原生菜单（WebView 跳过） |
| Vcs.Push | meta shift K / control shift K | `platform/dvcs-impl/resources/intellij.platform.vcs.dvcs.impl.xml:79`，macOS 经 `MacOSDefaultKeymap.kt` | 原生菜单（WebView 跳过） |
| Git.Branches | 无 / control shift BACK_QUOTE | `plugins/git4idea/backend/resources/intellij.vcs.git.backend.xml:226-228`；Mac OS X 10.5+.xml `remove="true"` | Popup；Windows/Linux 登记，macOS 不绑定 |
| HideActiveWindow | shift ESCAPE | $default.xml，macOS 继承 | Gallery 关闭弹窗 |

7. **交付范围**：P0 / v0.1，阶段 4；不包含双段快捷键、用户自定义映射和全量 IDE 动作。Update Project、Commit、Push 由原生 Git 菜单加速键唯一处理，WebView 不重复注册。

实现状态：**已实现**（框架及上述调用路径）；IDEA 录屏对照：未验证；真实 Git：不适用。

处理原则：有原生菜单项的动作由菜单加速键唯一处理，其余由 WebView keymap 处理。Cmd+0 / Cmd+9 以及 Cmd/Ctrl+T、K、Shift+K 标为原生菜单动作，registry 不消费、不分发。
