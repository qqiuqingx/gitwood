# 阶段 2 Diff 实测与验证记录

日期：2026-10-30。本 worktree 同时有其他阶段的未提交实现；以下只记录实际运行，
不表示阶段 2 或多平台验收完成。没有网络访问、安装依赖、commit/push 或启动 tauri dev。

## 比较耗时

环境：macOS 27.0（26A428），arm64；rustc 1.99.1（58cf8f9c3 2026-10-13），
Git 2.54.0（Apple Git-157）。具体芯片型号未获取；不据此推断其他机器性能。

生产 API：`diff::compare`；similar 3.2.1，默认精确空白模式，上下文 4 行，
包括行比较、Unicode 词区间、alignment 及 folds 输出。
测试 `crates/gitwood-core/tests/diff.rs::measure_twenty_thousand_lines`：
左右各 20,000 行，左 477,780 bytes，右 477,777 bytes；
把 `let value_15000 = 15000;` 的值改为 42，产生 1 个修改块。
计时在构造字符串后开始，到 compare 返回结束，不包括 Git/磁盘读取、进程启动、
编译、IPC、CodeMirror 挂载或浏览器绘制。只测一次，未计算平均值、p95 或内存峰值。

| 实际命令 | 最后一次测得耗时 |
| --- | --- |
| `cargo test --offline -p gitwood-core --test diff --test git_security -- --nocapture`，其中上述用例 | debug 2.248708 ms |
| `cargo test --offline --release -p gitwood-core --test diff measure_twenty_thousand_lines -- --nocapture` | release 628.875 µs（0.628875 ms） |

不能将单处修改的耗时外推为全文件替换、重复行、超长单行、多文件或浏览器滚动性能。
比较共用 1 秒 similar 算法 deadline，复杂输入可能得到更粗的有效变更块；
这不等于整个 API 的硬实时上限（分词、分配及输出仍需时间）。
尚未实测：最坏输入、WebView 内存/CPU、滚动帧率、连续换文件后的释放与长期稳定性。

## 实际验收命令

| 命令 | 结果与边界 |
| --- | --- |
| `cargo clippy --offline -p gitwood-core --all-targets -- -D warnings` | 通过 |
| `cargo test --offline -p gitwood-core` | 失败于并行后端的 `repository::watcher_real_events`（测试文件 tests/repository.rs）：`unable to start FSEvent stream`。没有改动监听实现，也没有将该失败写成通过。该次在 repository 测试处终止。 |
| `cargo test --offline -p gitwood-core --test diff --test git_security -- --nocapture` | Diff 9 个、安全 18 个测试均通过；含本任务新增的 blob 助手执行 sentinel 测试。 |
| `cargo test --offline -p gitwood-core -- --skip watcher_real_events` | 显式跳过上述用例后其余测试通过（含随后并行任务新增的 history_filtered）；不证明文件监听可用。 |
| `pnpm -C ui check` | 0 errors / 0 warnings |
| `pnpm -C ui test` | 9 个测试文件、69 个测试通过；Node 无 DOM，不代表 UI 手工交互通过。 |
| `pnpm -C ui build` | 通过；正式包不包含 DEV Gallery / Diff 示例，尚未接正式 shell/IPC。 |
| `NODE_ENV=development pnpm -C ui exec vite build --outDir /private/tmp/gitwood-phase2-gallery` | 额外临时构建包含 Gallery、DiffView 与 CodeMirror，验证组件实际打包；未启动服务器。临时构建不能用于发布。 |
| `rustfmt --edition 2021 --check`（仅本任务修改的 6 个 Rust 文件，见下方） | 通过；未运行 cargo fmt --all，也未格式化 repo/ 或 src-tauri/。 |
| `git diff --check` | 通过 |

仅格式化/检查：`src/diff/{mod,content,compare}.rs`、`src/git/command.rs`、
`tests/diff.rs`、`tests/git_security.rs`（路径均位于 crates/gitwood-core/）。

## 明确保留的验收项

- 在本机原生发布 WebView 中验证 CodeMirror CSP nonce、编辑器样式、行号、
  F7/Shift+F7、滚动条拖动、左右轮流滚动、折叠展开后的定位、分隔栏及焦点。
- 用相同真实文件与 IDEA 2025.3 录屏对照；本次源码参考是 0698a5fe 的 master 快照。
  默认上下文与快捷键有源码依据；无末尾换行提示样式/位置仍未核对，当前使用页脚文字。
- Lezer 语言包 vs Rust tree-sitter 的内存/体积/语言覆盖实测仍待决定，没有接语言包。
- Windows/Linux、最低 OS、Intel Mac、真实 Git UI/IPC 闭环、超大文件 UI 等未验证。
