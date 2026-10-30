# 阶段 1 控件交付与接入

开发构建初次加载 URL hash `#gallery` 时挂载 `Gallery.svelte`，例如 `http://localhost:1420/#gallery`。入口位于 main.ts 的 `import.meta.env.DEV` 分支，动态 import 在生产构建消除。生产包检查不含 Gallery 模块、十万行示例与 Gallery 专有文案。此任务未启动开发服务器。更改 hash 后需刷新页面。

展示包括十万行按索引生成的虚拟列表、树、可关闭标签、工具栏开关、禁用项、分组/子菜单弹窗。
示例文字与搜索反馈来自新增的 `ui/src/i18n/controls.ts`。示例数据只存在于 Gallery，不冒充仓库数据。

| 组件 | 公开 API |
| --- | --- |
| List | 必需 `rowCount: number`、`getRow(index): ListRow`、`label`；可选 `multi`、可绑定 `selectedIds: ReadonlySet<string>` / `activeIndex`、`role: listbox/tree`、`search`（默认 true）、`searchLabel/noMatchLabel`、`activateOnClick`、`row: Snippet<[ListRow,index,query]>`；回调 `onselectionchange(ids,index)`、`onactivate(row,index,reason)`、`onrangechange({start,end})`、`onquerychange(query)`、`onnavigate(event,index): boolean`、`oncontextmenu(event,row,index)`；实例 `focus()`、`selectIndex(index,extend?,toggle?)` |
| Tree | 必需 `source: TreeSource`、`label`、`expanded: ReadonlySet<string>`、`onexpandedchange(nextSet)`；可绑定 `selectedIds`；可选 `onselectionchange(ids,row)`、`onactivate(row)`、`searchLabel/noMatchLabel`。展开状态由调用方保存，异步数据刷新共用唯一来源 |
| SpeedSearch | List/Tree 的 `search` 直接挂接搜索行为；浮层组件 props 为 `query/noMatch/label/noMatchLabel`，不抢焦点。纯函数 `matchSpeedSearch(query,text): MatchRange[] \| null`、`highlightParts(text,ranges)`、`findSearchMatch(count,getText,query,active,direction,includeCurrent?)` 可供其他容器复用 |
| Highlight | `text`、可选 `query`；仅文本插值与 mark，无 HTML 注入 |
| Popup | 必需 `items: readonly PopupItem[]`、`label`、`anchor: HTMLElement \| Rect`、`onclose()`；可选 `submenu`、`defaultAction: {label,run,enabled?}`、`searchLabel/noMatchLabel`、`onexecute()`（执行叶子时通知祖先关闭整条链）。调用方按需挂载，onclose 时卸载；items 支持 `kind: item/heading/separator`、disabled、children、run。无开放/关闭动画和额外 WebView |
| Toolbar | `label`、`items`；动作 `{id,label,icon:Snippet,run,shortcut?,disabled?,pressed?}` 或 `{id,separator:true}`。pressed 受控，run 由业务改变状态；tooltip 使用调用方提供的本地化动作名/快捷键 |
| Tabs | `label`、`tabs: {id,label,disabled?,closeLabel?}[]`、`activeId`、`onchange(id)`；可选 `onclose(id)`、`panel: Snippet<[activeId]>`；调用方负责关闭后选择仍存在的标签 |

ListRow 的核心字段为 `id/label`，另可提供 disabled、kind 及 tree ARIA 元数据。`getRow` 必须同步返回已准备好的行；常规渲染只为可见窗口取行；导航、多选范围和搜索会按需读取其他索引。`onrangechange` 的 end 为 exclusive，可提前准备 Rust 分页数据；调用方在页数据到达后提供新的 getRow 回调使 Svelte 更新。不能直接在 getRow 中发出每行 IPC 请求。

TreeSource 为 `rootCount/getRoot(index)/childCount(id)/getChild(id,index)`；节点含 `id/label/hasChildren/disabled?`。扁平化只访问已展开节点，保留结构描述符，DOM 仍只渲染 List 可见窗口。源必须是无环树且节点 ID 唯一。外部大规模重排或折叠后由业务层决定选择/祖先恢复策略。

控件变量已纳入 Rust Light 目录、三套主题、Schema、CSS 默认值与主题文档，扫描测试确保所有组件 var 引用均有定义。行高和视口边距通过 CSS 测量，无持续循环。

keymap 注册表的 `native-menu` 绑定只作声明，不由 WebView 分发。Cmd+0/9 由原生菜单唯一处理；Gallery 仅注册 HideActiveWindow 演示弹窗关闭。

已运行的纯函数测试覆盖虚拟范围、多选、树扁平化/左右键、匹配/片段/循环导航、定位翻转、快捷键解析/分发。Vitest 为 Node 环境，没有安装 jsdom/happy-dom，也未新增依赖；**组件交互需在应用内或后续 E2E 验证**。

合并后的手工检查顺序：

1. List 单击、Cmd/Ctrl 切换、Shift 正反范围与收缩、首尾/翻页；十万行滚动时 DOM 行数保持可见窗口加 6 行缓冲以内。
2. List/Tree 直接输入 gb、GitBr、无匹配文本；检查片段高亮、Up/Down 循环、Backspace、Esc 保留选择、Enter 激活；补充中文 IME/WebKit 验证。
3. Tree 连续 Right 展开/进入、Left 返回/折叠、disclosure 与双击；切换主题时选择/展开不变。
4. Popup 放到视口四角，改变窗口尺寸与 CSS 行高；分组/禁用项跳过；Right/Left 子菜单，Esc 先退搜索再退菜单，点击外部与子项执行后焦点返回；验证 defaultAction 启用和禁用两种情况。
5. Toolbar Tab、左右/Home/End、Enter/Space、开关/禁用/tooltip；Tabs 切换、关闭、焦点、Cmd+W、Cmd+Shift+[/]、Ctrl+Left/Right。

当前只有源码依据与本地纯函数证据；IDEA 录屏、macOS WebView、Windows/Linux 和真实 Git 均未验收。SpeedSearch 已核对本地 matcher 源码；不实现拼音与键盘布局纠错，详见 SpeedSearch 清单。

阶段 2 Diff 示例：新增 Diff 标签，挂载 `features/diff/DiffView.svelte`，
数据来自 `diff-example.ts` 的内置 50/51 行文本与明确区间，无 IPC。
包含中文词修改、删除、插入、末尾换行差别与两组未修改折叠。
可检查 F7/Shift+F7、两边同步滚动、展开按钮、分隔栏拖动/左右键、禁用应用箭头；
主题切换不重建编辑器。生产入口仍排除 Gallery。

Diff 的 `left/right` props 可用字符串或 `Content`；二进制、无法 UTF-8 解码、超限
应传对应 Content 状态，不能用伪文本代替。详见 architecture 的 Diff 小节。
上述交互和生产 nonce/CSP 仍须浏览器与原生 WebView 实测；此任务未启动 tauri dev。

阶段 2 接入补充：List 现支持可绑定 scrollTop、minRowWidth、onviewportchange 和 overlay，
用于在同一可见视口上叠加 Canvas；Tree 支持 visibleRows/onrangechange 的 Rust 分页来源与
content 文本片段。List/Tree 的 searchRows 异步回调用于逐页复用原 matcher，过期搜索不选择行。
Tree expanded 的 `*` 表示全部展开，`!<id>` 表示在全部展开时单独折叠该节点（Rust 文件树）。
生产浏览入口为 App.svelte 的 File/Open 和欢迎页；Gallery 仍只含开发示例。
