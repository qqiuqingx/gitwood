# 阶段 0：提交图布局实测

测量日期：2026-10-29。范围仅为阶段 0 第 6 步的提交关系读取、解析、布局和可见行
查询，不代表阶段 0 其他项目完成，也不包含 Canvas、IPC 或 WebView。
在 `phase0/graph` worktree 本地测量；基线提交
`aad024f23eb8180bb2c19e93925c85bcbe35503f`，本次实现保持未提交。

## 环境

| 项目 | 实测值 |
| --- | --- |
| CPU，`sysctl -n machdep.cpu.brand_string` | Apple M2 Max |
| 内存，`sysctl -n hw.memsize` | 34,359,738,368 字节（32 GiB） |
| `sw_vers` | macOS 27.0，Build 26A428 |
| `uname -m` | arm64 |
| `git --version` | git version 2.54.0 (Apple Git-157) |
| `rustc --version` | rustc 1.99.1 (58cf8f9c3 2026-10-13) |
| `cargo --version` | cargo 1.99.1 (5f94df478 2026-08-27) |
| 编译 | release，workspace 的 LTO、codegen-units=1；无新增依赖 |

## 夹具与复现

macOS/Linux：需要 Python 3 和 Git，从仓库根执行。默认写到
`/tmp/gitwood-fixtures/graph-50k`；也可传入其他仓库外目录。已有夹具或仓库内输出
会被拒绝，不清理或覆盖用户目录。

```sh
./scripts/make-fixture-repos.sh /tmp/gitwood-fixtures
cargo build -p gitwood-core --release --example graph_bench
/usr/bin/time -l target/release/examples/graph_bench /tmp/gitwood-fixtures/graph-50k
```

Windows 提供 PowerShell 包装，核心生成器相同，需要 Python 3（`py -3` 或 `python3`）
及 Git 在 PATH 中。以下命令提供用法，本机未执行 Windows/PowerShell 验证：

```powershell
.\scripts\make-fixture-repos.ps1 D:\Temp\gitwood-fixtures
cargo build -p gitwood-core --release --example graph_bench
.\target\release\examples\graph_bench.exe D:\Temp\gitwood-fixtures\graph-50k
```

Linux 用 `/usr/bin/time -v` 记录 RSS，GNU time 的 KiB 需乘 1024；本报告数值均来自
macOS `time -l` 的字节值，未在 Linux 上运行。

生成器用 fast-import：1 个根，200 条特性分支各 100 个提交；每次合并前 main 推进
149 个提交，再做一次双父合并；所有特性分支均从同一根分叉。初始提交含 10,000 个
静态文件，每个提交修改 `progress.txt`，最终 10,001 个文件。合并树继承第一父节点，
是关系图压力夹具，不模拟真实文件冲突解决。

实际生成并用 `rev-list`、`for-each-ref`、`ls-tree`、`count-objects` 和 `fsck --full`
复核：

| 仓库指标 | 实测值 |
| --- | --- |
| 位置 | `/private/tmp/gitwood-fixtures/graph-50k`（`/tmp` 指向 `/private/tmp`） |
| HEAD | `640adf270daa4a778f503dbf4d39bfc3d9262366` |
| 提交 / 关系边 / 根 | 50,001 / 50,200 / 1 |
| 分支 / 双父合并 | 201（含 main）/ 200 |
| HEAD 树文件 | 10,001 |
| pack | 1 个，150,005 个对象，Git 报告 15.01 MiB |
| 最长边的行距 | 49,900 行 |
| 行距 >=30 的边 | 400 条 |
| 单行最多穿行边（不含端点） | 200 条 |
| 关系输出 | 4,108,242 字节 |
| 关系输出 SHA-256 | `d65186b818532ce322806ddca288000d58963a74fe1ca36ff1e4e58eb36d171c` |
| `git fsck --full --no-progress` | 成功，退出码 0 |

行距与穿行边数从同一份关系输出按 hash -> 行号复算：边在 `child < row < parent`
时穿行，不是根据“节点的列号”推断图宽。该夹具采样的节点最高列为 1，长边仍占据
更多列，不能把节点列号当作宽度。

## 测量口径

- 读取：生产薄适配层 `history::read::read_log` 的阻塞调用，包括 Git 启动、遍历、
  stdout/stderr 收集和等待退出。GitRunner 尚未合入，不含其发现、取消或调度成本。
- 实际参数：`git --no-optional-locks -c core.fsmonitor=false --no-pager log
  --no-color --no-decorate --no-notes --no-show-signature --no-ext-diff --no-textconv
  --format=%H %P --topo-order --all HEAD --`，以参数数组传递，cwd 指向夹具。
- 解析：`CommitGraph::parse_log`，包括两遍解析、临时 hash map、重复父去重和输入验证。
  原始输出在解析后释放，再开始布局计时。
- 布局：`GraphLayout::new(graph, &[])`，包括反向 CSR、迭代 DFS、全图链 index/颜色及
  边区间索引构建。无 ref 元数据时使用 DAG tips 行顺序；屏幕列在查询时生成。
- 可见行：每次 120 行；首次查询为 `[0,120)`，另外 1,000 次跳转查询起点为
  `i * 7919 % (commit_count - 120 + 1)`。用 `Instant` 分别计时并 `black_box` 消费
  结果；包含临时位置排序、内存分配和返回元素构建，不含返回后统计扫描与释放。
  中位值取排序后索引 500，P95 取索引 949，最大值取索引 999。
- 每轮消费 24,383,070 个节点/半边元素（含首次查询），覆盖大量穿行长边。没有缓存
  全图屏幕列或将全量元素序列化。
- 五轮均为独立进程、顺序执行，没有并行构建任务。先前已生成、遍历和试跑过夹具，
  未清空系统文件缓存；因此是缓存已预热的本地测量，不宣称冷盘或冷启动结果。
- 以下只记录最后一版基准的五轮实测，不将较早试跑混入统计。

## 耗时结果

读取/解析/布局单位为 ms；查询单位为 µs。每格各自四舍五入，因此相加可能有
0.001 ms 的显示差异。读取+解析直接使用未舍入数值相加。

| 轮次 | 读取 | 解析 | 读取+解析 | 布局 | 首次查询 | 查询中位 | 查询 P95 | 查询最大 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 265.356 | 21.593 | 286.948 | 0.494 | 29.542 | 786.208 | 1340.167 | 1448.875 |
| 2 | 206.094 | 21.102 | 227.196 | 0.491 | 29.250 | 788.417 | 1346.458 | 1459.708 |
| 3 | 210.143 | 21.560 | 231.703 | 0.489 | 30.333 | 788.375 | 1350.417 | 1488.750 |
| 4 | 213.169 | 21.336 | 234.506 | 0.525 | 30.083 | 785.459 | 1350.083 | 1462.458 |
| 5 | 206.705 | 21.382 | 228.087 | 0.504 | 30.917 | 781.833 | 1358.666 | 1614.042 |

五轮各指标的中位数：读取 **210.143 ms**，解析 **21.382 ms**，读取+解析
**231.703 ms**，布局 **0.494 ms**；查询中位耗时的五轮中位数 **786.208 µs**，
查询 P95 的五轮中位数 **1350.083 µs**。这两项不是将 5,000 个样本合并后的分位数。
首次查询所在的图顶端很窄，其约 30 µs 的耗时不能代表整张图的随机跳转。

## 内存结果与统计边界

`GraphLayout::estimated_bytes()` 五轮均为 **3,526,952 字节（3.364 MiB）**，含
hash 槽、父边 CSR、反向 CSR、index/颜色、max-parent 区间树、结构体及数组 capacity。
排除解析 map、DFS 栈、原始 log、查询返回数据及临时位置、分配器开销、进程代码和
Git/WebView。这是常驻拥有数据的估算，不是 OS RSS，也不是应用总内存。

外部 `/usr/bin/time -l` 的 `maximum resident set size`：

| 运行口径 | 字节 | MiB |
| --- | ---: | ---: |
| 读取+布局全过程，第 1 轮 | 45,301,760 | 43.203 |
| 第 2 轮 | 45,318,144 | 43.219 |
| 第 3 轮 | 45,334,528 | 43.234 |
| 第 4 轮 | 45,301,760 | 43.203 |
| 第 5 轮 | 45,350,912 | 43.250 |
| 单独 Git 关系读取（写出同一关系文件） | 45,318,144 | 43.219 |
| Rust 单进程：从关系文件解析+布局+1,001 次查询，无 Git 子进程 | **14,925,824** | **14.234** |

macOS `time` 通过 `wait4` 取得退出资源；XNU 会在退出时合并已等待子进程统计，
`ru_maxrss` 取最大值。因此读取+布局全过程的值可能由 Git 峰值主导，既不是 Rust
单进程峰值，也不是父子进程同时占用的总和。核对依据是 Apple 的
[`time.c`](https://github.com/apple-oss-distributions/shell_cmds/blob/main/time/time.c)、
[`kern_exit.c`](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_exit.c)
和 [`kern_resource.c`](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_resource.c)
中的 `wait4`、退出时 `ruadd`、`ru_maxrss` 最大值规则。

为隔离 Rust 本身，实际额外运行：

```sh
git --no-optional-locks -c core.fsmonitor=false --no-pager \
  -C /tmp/gitwood-fixtures/graph-50k log --no-color --no-decorate --no-notes \
  --no-show-signature --no-ext-diff --no-textconv '--format=%H %P' \
  --topo-order --all HEAD -- > /tmp/gitwood-graph-relations.txt
/usr/bin/time -l target/release/examples/graph_bench \
  --log-file /tmp/gitwood-graph-relations.txt
```

该隔离轮读取文件 **0.410 ms**、解析 **21.356 ms**、布局 **0.514 ms**；查询中位
**781.125 µs**、P95 **1353.042 µs**。14.234 MiB 是含解析临时分配和查询的进程生命期
RSS 峰值，不是布局稳态 RSS；该轮的文件读取耗时不用于替代 Git 读取耗时。
本机 `time -l` 另报 peak memory footprint 14,336,432 字节，未与 RSS 混用。

原始 stdout/RSS 保留在本机 `/tmp/gitwood-graph-final-{1..5}.txt`、对应
`-rss.txt`、`/tmp/gitwood-graph-final.json`、`/tmp/gitwood-graph-git-rss.txt` 和
`/tmp/gitwood-graph-isolated{,-rss}.txt`。初次在受限沙箱内运行 time 时读取
`kern.clockrate` 被 OS 拒绝；表中 RSS 来自之后获准读取系统统计的完整成功运行。

## 验证与尚未覆盖

最后一次实际执行结果（macOS arm64）：

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，19 个测试，无 ignored/filtered；其中新增 12 个
  history 单元测试和 4 个真实 Git 读取测试。一次重复验收暴露测试临时目录时间戳
  并发碰撞，已用进程内原子序号修正，之后全 workspace 检查通过。
- `cargo build -p gitwood-core --release --example graph_bench`：通过；五轮及隔离轮
  基准实际执行成功。
- `sh -n`、Python AST 语法检查、生成器拒绝覆盖/仓库内输出检查、`git diff --check`：通过。
- 用真实临时仓库验证多 ref/分离 HEAD、浅边界、退出码/stderr、缺失可执行文件及禁用
  fsmonitor/external diff/textconv/签名校验的读取路径。纯解析覆盖 SHA-256 和 CRLF。
- 未运行 Windows/Linux、PowerShell、最低 OS/Intel、应用打包、Canvas/IPC 渲染、真实
  IDEA UI 对照、冷缓存磁盘读取、数百万提交或极端超宽 DAG；这些结果不能外推。

算法来源和差异见 [architecture.md 的提交图小节](../architecture.md#提交图)。当前
保留完整普通长边，未实现 IDEA 默认长边折叠/箭头、完整 ref 排序、图筛选、虚线和
交互元素；GitRunner 发现/取消及刷新一致性尚待后续接入。宽图的可见输出仍可能很大，
需要在 Canvas/IPC 接入时单独量测传输量、绘制与帧耗时。
