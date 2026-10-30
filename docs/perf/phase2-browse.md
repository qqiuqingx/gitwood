# 阶段 2 只读浏览实测

2026-10-30，macOS 27.0（26A428）arm64，Rust 1.99.1，Node 25.6.1，pnpm 12.16.1。
仓库 `/tmp/gitwood-fixtures/graph-50k` 已存在，本次没有重新生成；实际 50,001 提交。
Rust 使用 release 优化构建，调用正式 GitRunner → Sessions/Lease → history_layout/history_rows。
没有启动 tauri dev，也没有启动应用做界面测量。

复现命令（均离线）：

```sh
cargo run --offline --release -p gitwood-core --example browse_bench -- \
  /tmp/gitwood-fixtures/graph-50k /tmp/gitwood-browse-pages.json
node ui/scripts/browse-perf.mjs /tmp/gitwood-browse-pages.json
```

“首屏”在下表仅指 Rust 完成仓库识别、全量关系布局和第一批 40 行数据；
不含 Git executable discovery（在计时前完成）、watcher 注册、status/refs、详情、IPC 序列化/传输、
WebView 初始化或绘制。三轮在同一进程重新打开仓库，不清除 OS 文件缓存，不能当作冷启动测量。

| 轮次 | 仓库识别 ms | 布局 ms | 第一批 40 行 ms | Rust 首屏数据就绪 ms |
| --- | ---: | ---: | ---: | ---: |
| 0 | 34.882 | 573.863 | 13.205 | 621.950 |
| 1 | 33.685 | 313.496 | 13.322 | 360.504 |
| 2 | 34.044 | 313.330 | 12.773 | 360.147 |

每轮随后顺序请求以下半开区间（40 行）。ms 是 Rust 方法从调用到返回的耗时，
包含未缓存详情的 Git 子进程/读取、图元素生成及后端缓存处理；**不是 Tauri IPC 往返时间**。

| start | 边段数 | 轮 0 ms | 轮 1 ms | 轮 2 ms |
| ---: | ---: | ---: | ---: | ---: |
| 40 | 160 | 11.938 | 11.789 | 11.775 |
| 80 | 160 | 11.861 | 11.897 | 11.660 |
| 1000 | 479 | 11.657 | 11.797 | 11.891 |
| 25000 | 8159 | 11.880 | 12.001 | 12.307 |
| 49960 | 16080 | 12.132 | 12.178 | 12.355 |
| 0（已缓存） | 158 | 0.030 | 0.030 | 0.036 |

绘制指令转换使用第三轮的真实 history_rows JSON，Node 中直接调用生产纯函数 graphCommands。
每页预热 100 次，再测 1000 次；视口 960 px、行高 24 px。此项只测生成指令，不调用 Canvas，
不把 Node 耗时称为 WebView 每帧绘制时间。后续代码调整后应重跑脚本。

| start | 指令数 | p50 ms | p95 ms | 最大 ms |
| ---: | ---: | ---: | ---: | ---: |
| 40 | 200 | 0.0066 | 0.0113 | 0.0857 |
| 80 | 200 | 0.0025 | 0.0037 | 0.0789 |
| 1000 | 519 | 0.0168 | 0.0176 | 0.0892 |
| 25000 | 8199 | 0.0956 | 0.2813 | 0.5622 |
| 49960 | 16120 | 0.2477 | 0.8263 | 1.4292 |
| 0 | 198 | 0.0028 | 0.0037 | 0.1225 |

实际打开到首屏可见、每次 history_rows IPC 往返、Canvas 每帧绘制、滚动 FPS、应用与 WebView
内存/空闲 CPU：**未测**。原生文件对话框、菜单、FSEvents、真实滚动及 IDEA 录屏：未在本轮运行。
此夹具保留大量长边，40 行最多返回 16,080 段；实际图宽、像素绘制和 WebView 分配成本仍需实机验收。

本轮验收记录（macOS arm64）：

- `cargo clippy --offline --workspace --all-targets -- -D warnings`：通过。
- `cargo test --offline --workspace`：仅 `watcher_real_events` 因 `unable to start FSEvent stream` 失败。
- `cargo test --offline --workspace -- --skip watcher_real_events`：98 项通过，含 smoke；doc-tests 0 项。
- `pnpm -C ui check`：0 错误、0 警告；`pnpm -C ui test`：11 文件、83 项通过。
- `CARGO_NET_OFFLINE=true pnpm tauri build --debug --no-bundle`：通过，产物 `target/debug/gitwood`。
- 仅本任务 Rust 文件的 `rustfmt --edition 2021 --config skip_children=true --check ...`：通过。
- `git diff --check`：通过。

过程中一次全套运行的既有 hook 取消测试在等待 child-pid 时超时；单独重跑及最终全套重跑均通过，
未修改该测试或 GitRunner。FSEvents 的沙箱外验证沿用用户提供的背景，本轮未在沙箱外复验。
