# W140：媒体因果链——把「清队列」改成点名并报告

- 日期：2026-10-18
- 上游：[W139 勘定报告](2026-10-18-w139-causal-chain-appraisal.md)（六条结论 + 落点 (e)）
- 产物：`crates/nai-media-core/src/lineage.rs`（新）、`crates/nai-media-core/src/lib.rs`（`MediaGate` 接线）、
  `crates/nai-media-core/tests/lineage.rs`（新，7 条）、`crates/nai-media-core/tests/gate.rs`（+4 条）、
  `docs/adr/0114-media-causal-lineage.md`、
  `openspec/changes/archive/2026-10-18-w140-media-causal-lineage/`

## 一、开工前的勘定（延续 W139，逐条现测）

| 项 | 现测结果 | 对本轮形状的约束 |
| --- | --- | --- |
| `MediaGate` 的生产装配点 | `crates/nai-runtime/src/composition.rs:160`（`GATE_CAPACITY=64`）、`apps/output-keeper/src/lib.rs:99`；其余 `MediaGate::new` 全在 tests 与 `examples/local_control.rs` | 印证 W139 第 6 条：只有这一层有生产装配者 |
| `poll` 的签名 | `pub fn poll(&mut self, now: Instant)`——**没有 `utc_ms`** | 取消**不能**依赖时钟 ⇒ `recycle_at_ms` 必须在登记时写入；省下 11 个既有调用点的改动 |
| `enqueue` / `consume` 的调用点 | 全仓只在 `crates/nai-media-core/tests/gate.rs` | 这两条登记点在**生产路径上没有调用者**（如实登记） |
| registry 88 条验收条目 | 无「媒体因果链 / 级联取消」对应条目 | 本轮**不**标任何条目为 `tested`，只落内部判据 |
| xtask 的测试文件枚举 | 只枚举 `tests/contracts/*.test.mjs` 一类 JS 判据，**不枚举** Rust 测试文件 | 新增 `tests/lineage.rs` **无需**在 `engineering_checks` 登记 |
| 「宽限」的既有参考实现 | 全仓 `crates/**.rs` 中 `grace` / `Grace` **零命中**；`drain` 只有 `Life::Draining`、`PluginDraining` 与 `control-api/src/lib.rs:582` 的有界排空 | 印证 W139 第 3 条判据：宽限今天零参考实现，形状要自己定 |

## 二、落了什么

### 2.1 `src/lineage.rs`（新，仅依赖 `std::collections::BTreeMap`）

| 项 | 内容 |
| --- | --- |
| `LineageId` | `session_id` / `utterance_id` / `generation_epoch` 三元组 |
| `FacetKind` | 五值闭集：`UtteranceRow`（可恢复行＝父）、`PendingGate`、`AudioBlock`、`SlotTicket`、`OutputAck` |
| `NodeState` | 四值闭集：`InFlight` / `Released` / `Cancelled` / `Recycled`；终态由 `TERMINAL_STATES` **一处**派生 |
| `LineageNode` | `state` + `recycle_at_ms`（**值**，登记时写入，事后不再读时钟） |
| `LineageReport` | `manifest: Vec<LineageRef>`，`count()` = `manifest.len()`——类型上没有第二个数字可存 |
| `LineageError` | 三值闭集：`ScopeMismatch` / `UnknownParent` / `EpochRegression`，三个都可达且都有用例 |
| `LineageRegistry` | `register` / `release` / `cancel_scope` / `reclaim` / `advance_epoch` + 读取面 |

### 2.2 `MediaGate` 的三条登记点

| 位置 | 动作 |
| --- | --- |
| `arm`（`lib.rs`） | `cancel_scope()`（点名上一代）→ `advance_epoch` → 为 `lease.allowed_utterance_ids` 逐条登记 `UtteranceRow` → `reclaim(utc_ms)` |
| `poll` 失败分支 | `cancel_scope()` 并把报告存入 `cancellation`（**在**清队列之前） |
| `enqueue` / `consume` | `enqueue` 登记 `PendingGate` 子节点（在容量检查之后，故重复入队语义不变）；`consume` 释放它 |

`consume` 多了一条拒绝：队列点名了某 utterance 而 lineage 里**没有**在途子节点 ⇒ `MediaError::Stale`。
这是「两处口径不一致时选拒绝」，不是放宽。

**签名零变更**：`arm`/`poll`/`enqueue`/`consume` 的签名与返回值都没动，
`apps/output-keeper`、`crates/nai-control-api`、`crates/nai-runtime` 的调用点一行未改。

## 三、判据（11 条新增）

`tests/lineage.rs`（7 条，针对 registry 自身）：

| 用例 | 钉住什么 |
| --- | --- |
| `facet_kinds_are_a_closed_set_with_distinct_names` | `FacetKind` 闭世界，5 值名字互不相同 |
| `terminal_states_include_cancelled_and_recycled_and_number_three` | 终态两格在、三格派生、四值被**划分**（非终态恰一格） |
| `cancellation_names_every_child_it_drops_and_the_count_is_the_manifest` | 逐条点名 +「清单长度 == 实际改状态的节点数」+ 第二次报 `0` |
| `grace_is_a_recorded_value_and_reclaim_never_waits` | 两条不同截止时间只收该收的；在途永不收；同时钟第二次报 `0` |
| `a_child_without_a_parent_and_an_out_of_scope_id_are_refused` | 三种拒绝逐条可达 |
| `release_names_the_child_it_moved_and_reports_zero_the_second_time` | 释放报 `0` 是「报告出来的 0」，不是跳过 |
| `reclamation_is_bounded_across_generations` | 跨 5 代后只剩当代，`len()` 恒等于 2 |

`tests/gate.rs`（+4 条接线）：`arm_declares_the_recoverable_row_of_every_admitted_utterance`（空清单也报，
`None` 与 `Some(empty)` 是两个事实）、`rearming_names_the_generation_it_dropped`、
`a_mute_names_what_it_dropped_rather_than_reporting_a_number`、
`enqueue_registers_the_gate_child_and_consume_releases_it`。

## 四、非空洞注入验证（4 组，全部真红）

| 编号 | 注入 | 变红的用例 |
| --- | --- | --- |
| I1 | `cancel_scope` 只改状态、不入清单 | `cancellation_names_…`（lineage）、`rearming_names_…`、`a_mute_names_…`（gate）——共 **3** 条 |
| I2 | `TERMINAL_STATES` 去掉 `Recycled` | `terminal_states_include_cancelled_and_recycled_…`——**1** 条 |
| I3 | `reclaim` 忽略记录值（`recycle_at_ms <= utc_ms` → `true`） | `grace_is_a_recorded_value_and_reclaim_never_waits`——**1** 条 |
| I4 | `enqueue` 不登记 `PendingGate` 子节点 | `enqueue_registers_…`、`a_mute_names_…`、既有 `every_consumption_rechecks_…`——共 **3** 条 |

还原后 `sha256sum -c`：`src/lineage.rs` 与 `src/lib.rs` 均逐字节 `OK`。
I4 让一条**既有**用例也变红，说明 `consume` 那条一致性拒绝是真的在拦东西，不是装饰。

## 五、诚实边界（MUST NOT 越线）

- 本轮落的是**定义性的对象 + 三条登记点 + 针对它自身的判据**。
  **没有**写、也 MUST NOT 写「LLM 在途 / TTS 未播 / 渲染队列已在同一因果链上」。
- `FacetKind` 五值里只有 `UtteranceRow`（`arm` 登记）与 `PendingGate`（`enqueue` 登记）**有登记者**；
  `AudioBlock` / `SlotTicket` / `OutputAck` **定义了但没有登记者**——那是留给未来三种 facade 的接缝，
  不是已覆盖的证据。
- `enqueue` / `consume` 全仓只在 `tests/gate.rs` 被调用 ⇒ 这两条登记点在**生产路径上没有调用者**。
  生产路径上真正会写 lineage 的只有 `arm`（output-keeper 的 `Action::Arm`）与 `poll` 的取消分支。
- `tests/registry.yaml` 88 条里没有对应条目 ⇒ 本轮**不**标任何条目为 `tested`。

## 六、门控（本机）

`cargo xtask check --profile full`（`CARGO_INCREMENTAL=0` + `CARGO_BUILD_JOBS=1`）⇒ **`GATE_EXIT=0`**。

| 步骤 | 结果 |
| --- | --- |
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean（零告警） |
| `cargo test --workspace --locked --exclude nai-xtask` | 85 个 `Running` 目标 + 18 个 `Doc-tests`，102 条 `test result:` 汇总，**1133 passed / 0 failed** |
| `cargo test --locked --package nai-xtask --lib` | 通过 |
| JS 判据套件 | 14 套：57 / 30 / 22 / 14 / 18 / 24 / 34 / 22 / 16 / 16 / 15 / 12 / 11 / 5，全绿 |
| `node scripts/frontend-gate.mjs` | 「全部步骤通过（工程基础，非渲染/人工 UI 验收）」；桌面 `test:ui` 71/71 |

`nai-media-core` 各目标：`unittests` 28、`tests/gate.rs` **11**（原 7，+4）、`tests/lineage.rs` **7**（新）、
`tests/frame_slots.rs` 7、`tests/mixing.rs` 18、`tests/timeline.rs` 1。新增 11 条判据在门控里全部 `ok`。

**中途的间歇 `LNK1104`（非代码缺陷，如实记录）**：首次门控在 `nai-engine` 的 bin 测试上命中
`LNK1104`，其后三次**每次都是另一个目标**（`pipeline` → `engine` → `nai-avatar-2d` 的 `render`），
八次预热重试也是每次换一个目标。判据按既有纪律取「**逐目标能过**」：`cargo build --workspace --all-targets`
串行预构建 `PREBUILD_EXIT=0`，且 `cargo test -p nai-engine --no-run` 单跑 `EXIT=0` ⇒ 非本轮代码回归。
处置方式是**限并发 + 串行预构建 + 把测试二进制先「只链接不运行」预热好**（`--no-run`，最终 `EXIT=0`，
`target/debug/deps` 446 个 exe），再跑门控即一次绿。**未**改任何代码、**未**放宽任何判据。

## 七、CI 观测

推送 **`a75074a`**，运行 **`39082021461`**，回读 `head_sha` = `a75074a85d9a50da1e0f4488c9f441b5f649eaad`
与推送 SHA **完整 40 位一致**（三件齐：`cb=` 纳秒唯一参数 + `Cache-Control: no-cache` + 回读 `head_sha`）。

| 平台 | 结论 | 步数（API 口径） |
| --- | --- | --- |
| `foundation (ubuntu-24.04)` | `completed / success` | 20 |
| `foundation (windows-latest)` | `completed / success` | 20 |

逐平台看完**整条步骤序列**。非 success 项全部是 `skipped` 且逐条核到原因：
`Publish the gate failure summary as annotations` 在门控通过时不执行（两平台同）；
`Install Linux C++ protobuf runtime` 与 `Full three-language contract suite (Linux C++ toolchain)`
只在 Linux 跑（Windows 侧 skipped）。步号从 16 跳到 30 是 `if` 未触发的步**不生成记录**（既有口径，
MUST NOT 拿 `jobs[].steps` 条数与 workflow 的 `- name:` 步数相减）。

## 八、具名登记（本轮发现但不做）

| 编号 | 事项 | 归宿 |
| --- | --- | --- |
| N1 | `AudioMixer::takeover`（`mixing.rs:404-411`）仍**报计数不报清单**——三处同型缺口的第三处 | 它**没有生产装配者**（`AudioMixer::new` 11 处全在 tests），本轮不碰；要先有装配者再补可见性，否则是给空气造家具 |
| N2 | 终态口径三处打架，分歧点在值 11（SQL 6 条级联 `… NOT IN(11..15)`） | 需先裁定 11 是否终态，再把 SQL 侧与本 registry 对账；本轮未动 SQL |
| N3 | `AudioBlock::validate` 的音频链在生产路径上没被调用 ⇒ epoch 在音频侧消失 | 属链路断点，不是 registry 能补的 |
| N4 | `FacetKind::AudioBlock` / `SlotTicket` / `OutputAck` 定义了但无登记者 | 留给对应 facade 有生产装配者时再登记 |
| N5 | 已取消的节点要等到下一次 `arm` 才被回收（不即时） | 有界（每代 ≤ `capacity` 条 + 每 utterance 一个子节点），不是泄漏 |
| N6 | `target-check/` 是 10-15 遗留的 scratch 目录（12G 构建缓存 + 前轮门控脚本 `gate-w1xx.sh` / `msvc-env.sh`），未被 `.gitignore` 覆盖且未提交 | **本轮不清理**：删它要连带删掉前轮处理 LNK1104 的门控脚本，代价大于收益。改为用**显式 `git add -- <path>`** 规避「`-A` 把 12G 缓存吸进索引」这个已复现两次的陷阱。是否把它写进 `.gitignore` 留给后续轮次裁定 |
