# 主题

首版只加载打包的 Light、Dark、Walnut；用户主题加载、热重载、预览属于 v0.2。
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

## 用户主题目录（v0.2）

- macOS：`~/Library/Application Support/Gitwood/Themes/`
- Windows：`%APPDATA%\Gitwood\Themes\`
- Linux：`~/.config/gitwood/themes/`

这些是未来用户主题目录；当前设置使用 Tauri app_config_dir，不扫描这些目录。

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
