# W126 报告：R09「升级预检与下载失败」的下载阶段（续传 / 重问预算 / 哈希闸门）

- 范围：`nai-upgrade-core`（新增 `src/download.rs`、`tests/download.rs`）。
- 前置：`docs/reports/2026-10-16-state-of-implementation-audit.md`（W126 前置审计）裁定路径 A，R09 为第一项。
- 快照基线：`dbf3749`（W125 回填提交）。计数均为**现测**。

## 一、缺口（现测）

R09 的 registry notes 自 W15 起写着「真实下载器（断流/重试/校验失败）未实现，只覆盖内容到达后的校验与预检」。
本轮先核实这句是否仍然准确，结论是**准确，且原因比「没写」更细**：

| 入口 | 收到的字节 | 是否有下载决策 |
|---|---|---|
| `verify_package(… files: &[PackageFile])` | 调用方持有 | 无 |
| `install_offline(… artifacts: &[OfflineArtifact])` | 调用方持有 | 无（且刻意无网络，见 `offline.rs` 文档） |
| `UpgradeTransaction::prepare(request.files: Vec<PackageFile>)` | 调用方持有 | 无 |

全 crate **没有一处决定「向哪一段要、短读要不要重问、什么时候放弃」**。于是基线 §14.3/§1322 的
「网络断开与下载中断时原版本继续运行，可续传并校验」在实现与判据两侧都不存在：
断流只能表现为「字节没到」，而「字节没到」与「下载失败」**在类型上是同一件事**——没有可判的区分。
架构上也**不能**把 HTTP 客户端塞进本 crate：它是 `shared` 层，`lib.rs` 与 `offline.rs` 都把
「无宿主 / 无平台 SDK / 无网络依赖」写成**刻意**的性质（R11 亦明确否决 `allow_fetch` 开关）。

## 二、判据（真实证据）

`nai-upgrade-core` 实测：**8** 个 `Running` 目标 + `Doc-tests`，合计 **99 passed / 0 failed**
（此前 79；新增 `tests/download.rs` **20** 条）。分目标：

| 目标 | 条数 | 目标 | 条数 |
|---|---|---|---|
| unittests `src/lib.rs` | 3 | `tests/migration.rs` | 11 |
| `tests/cloud.rs` | 21 | `tests/plan.rs` | 12 |
| **`tests/download.rs`（新）** | **20** | `tests/upgrade.rs` | 8 |
| `tests/gates.rs` | 7 | `tests/lifecycle.rs` | 17 |

其中 **2** 条走在**真实 loopback TCP/HTTP/1.1** 上（测试内自写最小 peer 与 client，**零新增依赖**）：

- `an_interrupted_transfer_over_a_real_socket_resumes_and_verifies`：peer 第一条响应**声明完整
  `Content-Length` 却只送 1024 字节**（断流）；断言 peer 记账收到的 `Range` 偏移 == **`[0, 1024]`**
  ⇒ 「偏移来自台账」是在**线上**被证实的，不是在脚本里被声明的。
- `a_source_that_keeps_dropping_the_socket_is_refused_by_name`：每条响应只送 1 字节；
  断言记账 == **`[0, 1, 2]`** 且 `resource_exhausted` ⇒ 重问预算同时管住**次数**与**进度**。

判据分布（脚本传输 18 条）：续传/台账 6（含「已全部持久化 ⇒ 零窗口仍通过校验」与「超声明续传点 ⇒ 拒绝」）、
预算与重问分类 6（幅度 1 + 边界 5）、哈希闸门 2、上界与不越权 2、停止信号 2（入口的调用方 cancel/过期期限、
循环内的中途取消）。

## 三、注入验证（每条守卫都要证明它会响）

**10** 条独立注入，每条用编辑工具做、`cp` 逐字节还原（`diff -q` 核对）。逐条结果：

| # | 注入（移除的机制） | 变红的判据 | 条数 |
|---|---|---|---|
| M1 | 进循环后不用台账的值（`have = 0`） | 续传族（含两条 wire 级） | 10 |
| M2 | 去掉 append 后的回读确认 | `a_ledger_that_does_not_persist_is_a_named_failure` | 1 |
| M3 | 预算上界改为 `+1` | 三条边界判据（**幅度判据仍绿**） | 3 |
| M4 | 去掉短读清点 | 两条中断计数判据 | 2 |
| M5 | 去掉「多于窗口」守卫 | `more_than_the_window_is_refused` | 1 |
| M6 | 去掉损坏工件丢弃 | 两条损坏判据 | 2 |
| M7 | 哈希输入不读台账（用空缓冲） | 走到哈希/校验阶段的判据 | 8 |
| M8 | 不预先应用声明上界 | `the_declared_bounds_are_applied_before_any_window` | 1 |
| M9 | 去掉循环内的 `op.check()` | `a_cancellation_arriving_mid_transfer_stops_the_loop` | 1 |
| M10 | 去掉入口 `checkpoint` | `the_callers_stop_signals_are_honoured_before_the_first_window` | 1 |

M3 是**成对判据**的实测：夹具从常量派生 ⇒ 抬高/挪动上界**不会**让边界判据变红的那一半由幅度判据管，
而「闸门还在不在」由边界判据管——本轮两个方向都钉住了。M9/M10 是**自查产物**：
初稿只有入口一条取消判据，等于循环内的停止守卫**未被覆盖**（按纪律即装饰），故补两条判别性判据
（其中一条用「取到窗口就 `cancel()` 的传输」构造，使停止信号到达的时机是构造事实，不依赖墙钟）。

同时**删掉**一条重复守卫：`bound_declared` 之后那条 `checkpoint` 与入口那条处于同一条直线路径、
可观测差异为空（`bound_declared` 是纯计算）⇒ 属「同一件事两处定义」，只留入口一条。

## 四、registry 判定（**维持 `implemented`**）

`R09` **维持 `implemented`**，就地订正：本轮把下载**编排**（续传 / 重问预算 / 下载期校验）与
其故障语义落成判定面与载体，且两条判据走在真实 socket 上；但**真实分发端点、真实安装器、
真实磁盘与 GPU 度量仍是外部注入点** ⇒ 按 `admission-registry`「未完成或外部阻塞的目标 MUST 保留为
implemented/blocked，不得因映射自动升级为 tested」不升 `tested`（第五次使用「守卫通过 ≠ 本项达标」）。

本轮**零新增依赖**：peer 与 client 都是测试内最小实现，故 `offline.rs` 依赖的
「本 crate 无 HTTP 客户端 ⇒ 不会偷偷下载」这一**编译期性质**在 dev-dependency 层面仍逐字成立。

## 五–七、门控 / 提交推送 / CI 观测
由本轮**回填提交**填入（回填提交自身不占位）。
