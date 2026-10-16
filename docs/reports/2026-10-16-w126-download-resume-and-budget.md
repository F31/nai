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

## 五、门控（全量 `cargo xtask check --profile full`）
**跑了两次**，两次结论逐项相同；计数均为**现测**。

| 次 | 覆盖的工作树 | `GATE_EXIT` | 耗时 | `LNK1104` | `error[` | OOM |
|---|---|---|---|---|---|---|
| 1 | 主提交 `1ca82ce` | **0** | 4m32s | 0 | 0 | 0 |
| 2 | 补提交 `33d0239` | **0** | **282s** | 0 | 0 | 0 |

- 测试目标 **96** 个全 `0 failed`：**78** 个 `Running` 目标 + **18** 个 `Doc-tests`
  （原始 `Running` 行 **79**，首行 ``Running `target\debug\nai-xtask.exe check --profile full` `` 是 gate runner
  自身回显，**不计**），合计 **1017 passed / 0 failed**。
- Node 侧 **13** 个套件、**323** passed / **0** failed（逐套件 `# fail 0`）。
- 步骤全过：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、
  `cargo test --workspace --locked --exclude nai-xtask`、`cargo test --locked -p nai-xtask --lib`、
  `node scripts/frontend-gate.mjs`。
- `nai-upgrade-core` 现测 **8** 个 `Running` 目标 + `Doc-tests` = **99 passed / 0 failed**
  （`lib` 3 / `cloud` 21 / **`download` 20** / `gates` 7 / `lifecycle` 17 / `migration` 11 / `plan` 12 / `upgrade` 8）。
- ⚠️ **为什么要跑第二次**：「归档前最后一读」在**主提交之后**才发现一处重复守卫（见 §六），改动虽只有 6 行，
  但**源码已变 ⇒ 不能沿用旧门控结论**。第二次跑在同一份改动上，计数与第一次逐项相同——这一点是**现测**，不是推断。
- 告警：仅增量编译锁文件「拒绝访问 (os error 5)」，跨多个 crate 共有、与本轮改动无关。
- 本 profile **未包含** `cargo xtask schema --check`（它需要 protoc 3.21.12，属**工具链前提**；
  该步骤的成因已由 `requires-compiled-artifact` + 仓库内凭据兑付，ADR 0086）⇒ **不作「已通过」声明**。
- 日志末行 `GATE_EXIT=0`；本轮门控输出**不含** `GATE_OK` 这一 token（与 W125 日志格式不同）⇒ 本报告只按实测写。

## 六、提交 / 推送
- **主提交 `1ca82ce`**（`test(w126): R09 下载阶段的续传、重问预算与哈希闸门`）：15 文件 / **+1796 −7**，
  push 区间 **`dbf3749..1ca82ce`**。
- **补提交 `33d0239`**（`test(w126): 删除 R09 下载循环前的重复守卫`）：1 文件 / **+6 −9**，
  push 区间 **`1ca82ce..33d0239`**。分支 `main`，SSH 443（`HostName=ssh.github.com -p 443`）。
- **补提交的来由**（如实披露）：「归档前最后一读」逐字核对判定词时，`grep` 发现
  `resume ledger exceeds declared size` 在 `crates/nai-upgrade-core/src/download.rs` **出现 2 次**（循环前 / 循环内）。
  两条守卫**同 token 同条件**，且首轮迭代时循环内读到的值必然等于循环前读到的值（`have == start`）
  ⇒ 循环前那条**永不单独生效**，属「同一件事 MUST NOT 两处定义」；反过来说，**删掉循环内那条判据仍全绿**，
  而**删掉循环前那条会静默收窄覆盖面**。处置：删循环前那条，只留循环内（它每轮迭代都重新从台账读回，是真正的闸门），
  `start` 保留**仅作报告计数**并加注释说明闸门位置。判据不变：`tests/download.rs` 20 条全过，
  其中 `an_oversized_resume_point_is_refused_not_clamped` 覆盖该守卫。
- 三方 SHA 一致：本地 `HEAD` == `origin/main` == 远端 `refs/heads/main`
  == **`33d02395c094300781bc71b20454e468b12cd847`**。
- 暂存一律用**显式路径**（不用 `-A`）：`.commitmsg_w126*.txt` 与 `target-check/` 因此天然不进提交。
- openspec 三连（主提交内）：`validate --strict` → `archive --yes` → `--archived --strict`
  **124 / 0**（主规格 **43** 份，未归档 validate `--all --strict` **43 / 0**）。

## 七、CI 观测
- **补提交 `33d0239`（= `HEAD`，有效结论）**：run id **38882423048**（`run_number` #195）。
  回读 `head_sha` = `33d02395c094300781bc71b20454e468b12cd847`，与推送 SHA **完整 40 位一致**
  ⇒ `completed / success`（创建 09:43:36Z、收尾 09:57:38Z）。逐 job：
  `foundation (windows-latest)` **success**（26 步，无失败）、`foundation (ubuntu-24.04)` **success**（26 步，无失败）。
- **主提交 `1ca82ce`（已被取代）**：run id **38881674808**（`run_number` #194），
  `head_sha` = `1ca82ceb73dbf048c198aac9d558866d00f5d737` **完整 40 位一致** ⇒ `completed / success`
  （收尾 09:51:06Z）。它覆盖的 SHA 已**不再是 `HEAD`**，故**不**作结论依据，仅为闭合「主提交当时是不是绿的」补查 **1** 次。
- 匿名 API 用量 **6** 次：① 触发确认（`?head_sha=` 完整 40 位，`total_count=1`）②③ 第一次完成态（run + jobs）
  ④⑤ 第二次完成态（run + jobs）⑥ 被取代 run 的补查。每次均带**唯一查询参数** + `Cache-Control: no-cache`；
  **未轮询**（相邻两次完成态查询间隔 **5m35s**）。
- ⚠️ **口径修正输入（第二次实测，修正 W125 的「约 12 分钟」）**：本轮 run 全程 **14m02s**。
  第一次完成态查询在 run 开始后 **13m34s**（09:57:10Z）时，ubuntu **已 success**、windows **仍 `in_progress`**，
  而 run 实际在 **28 秒后**（09:57:38Z）就双双收尾 ⇒ **windows 侧确实比 ubuntu 晚，但差距可以小于 1 分钟，
  也可能远大于 4 分钟**；「等到某个固定分钟数再单次查」**不是可靠判据**。
  ⇒ 单轮完成态**最少两次**查询（合计三次：触发 + 两次完成态），且**第二次必须在确认前一平台仍缺之后才发**。
- **回填提交**（docs-only）自身 CI：另做 **1** 次观测，结论记入 memory，不在本报告占位（回填提交自身不回填）。
