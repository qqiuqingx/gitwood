# SpeedSearch

1. **入口**：List/Tree 获得焦点后直接输入；不需点击搜索框或显式快捷键。Popup 通过 List 复用。macOS / Windows / Linux 使用相同文本键；Cmd/Ctrl/Opt/Alt 组合及 composition 中间事件不当作普通输入。
2. **交互行为**：搜索浮层不抢列表焦点；字符追加、Backspace 按 Unicode code point 删除末尾字符，compositionend 接受提交文本。大小写不敏感；首段可从名称任意位置开始，连续字符优先匹配；不连续跳转仅允许到驼峰、缩写尾词、数字和标点/路径分隔符后的词边界。查询按 nameToWordList 分词，纯空白词丢弃，剩余分段以通配间隔连接，支持 `*` 间隔；例如 `gbp → GitBranchPanel`、`fea bra → feature-branch`、`GitBr → GitBranchPanel`。不是任意字符子序列（`fbr` 不匹配 `foobar`）。返回 UTF-16 半开区间，合并相邻片段；Highlight 逐片段文本插值，保留 `<script>` 等文字原样。Up/Down 在匹配项间循环，Home/End 到首末匹配；搜索不是多选操作，每次定位单选。
3. **对话框与反馈**：无匹配时保留原选择，浮层使用 error 背景/文字变量、`aria-invalid` 和本地化“无匹配项”状态文本。Esc 只关闭搜索并保留选择，不关闭外层 Popup；第二次 Esc 才交给外层。Enter 关闭搜索后继续激活；Left/Right/PageUp/PageDown 关闭搜索后继续导航。清空查询移除浮层；不隐藏列表中的非匹配项。
4. **状态恢复**：搜索状态属于实例，失焦到组件外清除；选择、滚动仍保留。无超时定时器、无空闲轮询。匹配导航按索引扫描并提前返回，不分配完整匹配数组；未来 Rust 分页源需要业务侧本地可搜索索引或 Rust 搜索适配，不能让 `getRow` 为每个匹配候选单独 IPC。
5. **Git 影响与测试**：无。`ui/tests/speed-search.test.ts` 覆盖驼峰、缩写、分词、数字、路径、中文、空查询、无匹配、回溯、精确片段、恶意 HTML 文本和循环跳转。键盘输入、IME/WebKit、浮层颜色需应用内/E2E 验证。
6. **源码核对：已完成上述默认配置的源码阅读与用例核对**。revision `0698a5fe297dfa08069f015cc423a6a85ed7fc60`；`SpeedSearchComparator.obtainMatcher` 使用 `NameUtilCore.nameToWordList` 后以 `*` 连接并加前导 `*`；`NameUtil.MatcherBuilder` 默认 `IGNORE_CASE`，不启用 FIRST_LETTER，首字母大小写不强制一致。大小写仍影响混合模式的片段起点（`seemsLikeFragmentStart`）和小写跨词限制（`indexOfWordStart`）。`NameUtilCore.isWordStart` 包含大写缩写尾词、每个数字、汉字与假名脚本边界。`MinusculeMatcherImpl` 的 `maxMatchingFragment/isSkippingDigitBetweenPatternDigits`、`findLongestMatchingPrefix`、`improveCamelHumps` 决定连续优先、数字不可跳过、回溯和高亮片段，例如 CU 在 CurrentUser 中优先高亮 C/U。`matchWildcards` 中空格只寻找词起点，星号允许词内匹配；但 comparator 的分词会丢弃纯空白片段，因此 `foo ar` 在此配置下转成 `*foo*ar`，可匹配 foobar。`isMiddleMatch` 的最小长度只约束后续回溯，完整短后缀仍可匹配。超过 100 个 UTF-16 单元使用 substring 路径。`FixingLayoutMatcher` 已阅读，**按范围不实现键盘布局纠错、拼音和排序评分**；不启用 typo tolerant。算法移植登记于文件头和 NOTICE。
7. **交付范围**：P0 / v0.1，阶段 1。List/Tree 统一采用 `SpeedSearchComparator(false,true)` 配置；IDEA 各业务列表可能选择不同 comparator，仍需录屏确认。未声称所有 IDEA 搜索场景等价。

实现状态：**已实现并核对匹配源码规则**；IDEA 录屏对照：未验证；真实 Git：不适用。JS 与 JVM 全 Unicode 大小写转换（尤其补充平面字符）仍需跨运行时对照。
