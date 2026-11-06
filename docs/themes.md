# 主题

应用包含 Light、Dark、Walnut，并从平台主题目录加载用户主题；用户主题加载、热重载和设置预览自 v0.2 启用。
Light/Dark 的 Int UI 调色板、文件状态及语法颜色参考 intellij-community 的
`platform/platform-resources/src/themes/expUI/expUI_{light,dark}.theme.json` 和
`expUI_{light,dark}Scheme.xml`，许可证 Apache-2.0，出处见 NOTICE。
上游未直接定义的 Gitwood 语义按同一调色板补齐，Diff 背景是对应语义的调和色；
这不是完整导入 IDEA 主题。Walnut 以原型的 bg/panel/editor/fg/dim/selection 等变量为基准。
字体沿用系统字体栈，不打包字体。

## 格式与继承

```json
{
  "$schema": "./theme-schema.json",
  "schemaVersion": 1,
  "name": "My theme",
  "author": "Your name",
  "dark": true,
  "extends": "walnut",
  "colors": { "accent": "#83a598" }
}
```

`colors` 是按点分组的扁平变量表，除颜色还包含字体和尺寸。字段必须显式提供，
`extends` 可省略，省略时按 dark 选 Light/Dark；继承只覆盖显式且有效的变量。
变量缺失沿用父主题；未知变量、类型错误、无效颜色、循环继承、未知父主题、
缺字段、无效 JSON 和不支持的版本都返回 `{code,path,message}` 错误。
无效版本/JSON 回退完整默认主题；单个无效变量保留继承值。
颜色只接受 #RGB/#RGBA/#RRGGBB/#RRGGBBAA，禁止 URL、CSS 表达式或外部资源。

主题设置的 **Open Theme Directory / 打开主题目录** 会在目录不存在时先创建，再由 Rust 侧 opener 调用系统默认文件管理器打开；按钮不传入路径参数。
字体只允许本地字体名称列表；尺寸限 0–99.999…px，行高 1–2，字重 100–900。
内置主题由 include_str! 打包。解析器支持校验继承链，不扫描用户目录。

## CSS 命名

点转换为连字符，camelCase 同时转换为 kebab-case：
`background.primary` → `--background-primary`，
`font.editor.lineHeight` → `--font-editor-line-height`。
`graph.lanes` 是 1–32 个颜色的数组，生成零起始的 `--graph-lanes-0` 等变量，
并输出 `--graph-lanes-count`。切换时删除旧主题变量，防止残留较长的泳道数组。
所有变量在 Rust 校验后通过 CSSOM 注入 document.documentElement；不重建组件或读 Git。
`system` 由前端 prefers-color-scheme 的 change 事件跟随系统，只在有效主题改变时取主题。
原生菜单和系统标题栏由操作系统绘制，不属于 CSS 主题覆盖范围。

项目头像使用 `project.avatar.0.start` / `.end` 至 `project.avatar.8.start` / `.end`，
对应 `RecentProjectIconHelper.kt` 的 `ProjectIconPalette` 九组渐变。
Light / Dark 分别使用上游浅色 / 深色值，Walnut 沿用 Dark；
`project.avatar.text` 为白色 `#FFFFFF`，`project.avatar.size` 默认 `20px`。
大小按控件尺寸校验（0–2048px）；其余值按颜色校验。
颜色索引为 Java 字符串哈希的 `abs(hash(path)) % 9`，不保存递增索引。
CSS 变量如 `--project-avatar-0-start`，方向从左下 end 到右上 start，圆角半径 20%。

## 用户主题目录与加载

- macOS：`~/Library/Application Support/Gitwood/Themes/`
- Windows：`%APPDATA%\Gitwood\Themes\`
- Linux：`~/.config/gitwood/themes/`

Tauri 的 `app_config_dir` 按应用标识指向 `io.github.qqiuqingx.gitwood`，与上述用户目录不同；
主题加载按平台基础目录直接拼接 `Gitwood/Themes`（Linux 为 `gitwood/themes`），不会扫描
`app_config_dir`。目录不存在时按空目录处理，不自动创建。

只读取目录直属的 `.json` 普通文件，不递归；主题目录本身可以是符号链接（如 dotfiles 管理），
目录内的符号链接、子目录和非 JSON 文件跳过，超过
256 KiB 的文件拒绝加载。ID 为 `user:<文件名去掉 .json 扩展名>`，与 `system`、`light`、
`dark`、`walnut` 等内置选择隔离。用户主题只可 `extends` 内置主题；其他用户主题继承会作为
`unknown_parent` 错误报告。

加载前按文件名（忽略大小写排序，大小写相同时按原文件名排序）。显示名称按去首尾空格并
忽略大小写比较；内置主题名保留，剩余重复名称由排序在前的文件生效，后续文件报告
`duplicate_name`。格式、字段或颜色错误会以文件名及 `{code,path,message}` 列在设置里，
该文件不加入主题列表，也不影响其他主题。当前用户主题无效或缺失期间，保存的主题 ID 保持不变，
界面按“跟随系统”渲染，并在主界面和主题设置中提示主题暂不可用；无效文件的详细错误仍列在设置里。
主题文件恢复有效后，热重载会自动重新应用已保存的主题。菜单不勾选缺失主题，也不自动勾选“跟随系统”；
只有用户在主题设置中主动选择其他主题并确认时才改写保存值。

只要目录存在，监听器只对其做非递归监听并以 250 ms 静默期防抖。目录尚不存在时，监听最近
的现存父目录中通向主题目录的创建事件，随后把监听切换到主题目录；无空闲轮询。变更后刷新
主题列表与 View ▸ Theme 原生菜单，并向所有项目窗口发送主题目录变更事件。窗口重新注入
当前有效主题的 CSS 变量，不读取 Git 或重建组件。设置中选中主题立即在当前窗口预览；确认后应用
保存该主题，取消则恢复打开设置时的选择。当前保存主题暂不可用时，Apply 不会改写保存值，除非用户
主动选中其他主题。主题目录路径可复制，打开目录按钮留待 M7 opener 插件。
监听目标和事件路径都先规范化已存在的祖先，再拼接尚不存在的路径段；因此 `/var` 与 `/private/var`
等符号链接别名及 Windows `\\?\` 规范路径不会造成目录前缀比较失配。

## 对比度提示

对以下文字前景 / 背景组合计算 WCAG 2.x 相对亮度比值：`text.primary/background.primary`、
`text.secondary/background.primary`、`text.header/background.header`、
`text.header.secondary/background.header`、选中与非活动选中行的文字 / 背景、
`text.primary` 及 `syntax.comment` 在基础、Diff 新增、删除和修改行背景上的组合、
按钮普通 / 悬停 / 禁用文字与背景、输入框正文 / 占位文字与背景、菜单正文 / 选中 / 禁用文字
与背景、Tooltip 文字与背景。设置对当前选中主题列出低对比组合；不阻止使用。

UI 文字字号达到 24 px，或粗体达到 18.67 px 时最低比值为 3:1；其他文字为 4.5:1。
带透明度的背景先合成到 `background.primary`，前景再合成到已渲染背景后计算相对亮度。
透明颜色仍只接受主题格式中的十六进制颜色，不接受 CSS 表达式或 URL。

## 变量清单

JSON Schema 为编辑器提供每个变量的类型和补全。下表列出首版全部变量。

| 变量 | Light 默认值 |
| --- | --- |
| `background.primary` | `#FFFFFF` |
| `background.secondary` | `#F7F8FA` |
| `background.header` | `#27282E` |
| `text.primary` | `#27282E` |
| `text.secondary` | `#6C707E` |
| `text.header` | `#EBECF0` |
| `text.header.secondary` | `#A8ADBD` |
| `text.header.outgoing` | `#5FAD65` |
| `text.header.incoming` | `#548AF7` |
| `text.disabled` | `#A8ADBD` |
| `accent` | `#3574F0` |
| `selection.bg` | `#D4E2FF` |
| `selection.fg` | `#000000` |
| `selection.inactive.bg` | `#DFE1E5` |
| `separator` | `#EBECF0` |
| `focus.border` | `#3574F0` |
| `button.bg` | `#3574F0` |
| `button.fg` | `#ffffff` |
| `button.hover.bg` | `#3574F0` |
| `button.disabled.bg` | `#EBECF0` |
| `button.disabled.fg` | `#6C707E` |
| `button.border` | `#3574F0` |
| `input.bg` | `#FFFFFF` |
| `input.fg` | `#27282E` |
| `input.border` | `#EBECF0` |
| `input.placeholder` | `#6C707E` |
| `input.disabled.bg` | `#F7F8FA` |
| `popup.bg` | `#FFFFFF` |
| `popup.fg` | `#27282E` |
| `popup.border` | `#C9CCD6` |
| `popup.shadow` | `#00000040` |
| `menu.bg` | `#FFFFFF` |
| `menu.fg` | `#27282E` |
| `menu.selection.bg` | `#D4E2FF` |
| `menu.selection.fg` | `#27282E` |
| `menu.disabled.fg` | `#6C707E` |
| `tooltip.bg` | `#27282E` |
| `tooltip.fg` | `#FFFFFF` |
| `tooltip.border` | `#27282E` |
| `scrollbar.track` | `#FFFFFF` |
| `scrollbar.thumb` | `#EBECF0` |
| `scrollbar.hover` | `#6C707E` |
| `toolbar.bg` | `#27282E` |
| `toolbar.hover.bg` | `#383A42` |
| `toolbar.fg` | `#EBECF0` |
| `tab.bg` | `#F7F8FA` |
| `tab.active.bg` | `#FFFFFF` |
| `tab.fg` | `#6C707E` |
| `tab.active.fg` | `#27282E` |
| `tab.active.border` | `#3574F0` |
| `diff.added.bg` | `#E3F7E7` |
| `diff.added.inline` | `#C5E5CC` |
| `diff.removed.bg` | `#FFEBEC` |
| `diff.removed.inline` | `#FAD4D8` |
| `diff.modified.inline` | `#88ADF7` |
| `diff.modified.bg` | `#C2D8F2` |
| `diff.gutter.bg` | `#F7F8FA` |
| `diff.gutter.fg` | `#AEB3C2` |
| `diff.gutter.added` | `#7FC784` |
| `diff.gutter.removed` | `#767A8A` |
| `diff.gutter.modified` | `#88ADF7` |
| `graph.lanes` | `#3574F0, #208A3C, #DB3B4B, #DF9303, #834DF0, #058B90` |
| `graph.head` | `#3574F0` |
| `graph.merge` | `#6C707E` |
| `label.branch.local` | `#208A3C` |
| `label.branch.remote` | `#3574F0` |
| `label.tag` | `#DF9303` |
| `label.head` | `#3574F0` |
| `status.added` | `#067D17` |
| `status.modified` | `#0033B3` |
| `status.deleted` | `#6C707E` |
| `status.renamed` | `#0033B3` |
| `status.untracked` | `#B23247` |
| `status.conflicted` | `#DE1B2E` |
| `status.ignored` | `#7D5020` |
| `syntax.keyword` | `#0033B3` |
| `syntax.string` | `#067D17` |
| `syntax.comment` | `#8C8C8C` |
| `syntax.number` | `#1750EB` |
| `syntax.type` | `#27282E` |
| `syntax.function` | `#00627A` |
| `syntax.operator` | `#27282E` |
| `syntax.variable` | `#27282E` |
| `font.ui.family` | `-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif` |
| `font.editor.family` | `ui-monospace, "SFMono-Regular", Consolas, monospace` |
| `font.ui.size` | `13px` |
| `font.editor.size` | `13px` |
| `font.editor.lineHeight` | `1.5` |
| `font.small.size` | `12px` |
| `font.weight.heading` | `600` |
| `line.height.ui` | `1.5` |
| `space.inline` | `8px` |
| `space.panel.x` | `12px` |
| `space.panel.y` | `8px` |
| `space.header.y` | `9px` |
| `splitter.size` | `5px` |
| `border.width` | `1px` |
| `focus.width` | `2px` |

控件补全：List/Tree 行高参考上游 rowHeight=24；SpeedSearch 背景/边框使用同名键。
上游未指定的搜索高亮、禁用透明度和几何尺寸采用 Gitwood 语义值；Walnut 沿用原型调色板。
Popup shadow 是颜色，组件用固定几何投影；不接受任意 CSS 表达式。透明度范围 0–1，控件尺寸范围 0–2048px。

| 补全变量 | Light 默认值 |
| --- | --- |
| `list.disabled.opacity` | `0.5` |
| `list.row.height` | `24px` |
| `list.selection.bg` | `#D4E2FF` |
| `list.selection.inactive.bg` | `#DFE1E5` |
| `list.selection.inactive.text` | `#27282E` |
| `list.selection.text` | `#000000` |
| `popup.height` | `312px` |
| `popup.padding` | `8px` |
| `popup.viewport.margin` | `8px` |
| `popup.width` | `312px` |
| `search.bg` | `#FFFFFF` |
| `search.border` | `#EBECF0` |
| `search.error.bg` | `#FFEBEC` |
| `search.error.text` | `#DE1B2E` |
| `search.match.bg` | `#D4E2FF` |
| `search.match.text` | `#27282E` |
| `search.text` | `#27282E` |
| `search.width` | `182px` |
| `tab.close.size` | `24px` |
| `tab.disabled.opacity` | `0.5` |
| `toolbar.active.bg` | `#D4E2FF` |
| `toolbar.button.size` | `24px` |
| `toolbar.disabled.opacity` | `0.5` |
| `tooltip.offset` | `8px` |
| `tooltip.text` | `#FFFFFF` |
| `tree.disclosure.size` | `12px` |
| `tree.indent` | `12px` |
