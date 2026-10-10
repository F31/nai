# W26 实测报告：云断连不伤本地链路与低云容量（A10）

- 日期：2026-10-10
- 变更：`openspec/changes/w26-cloud-disconnect/`
- 决策：[ADR 0028](../adr/0028-local-autonomy-boundary.md)；指南：[docs/cloud.md](../cloud.md)
- 判据：`tests/registry.yaml` A10

## 结论

A10 的两半——**云端断连时本地实时链路与本地静音不受影响**（E07）、**低云容量不写未经测量的数字**（E08）——在 `nai-upgrade-core`（shared 层）内落成可执行判据，**零新增依赖**，18 包与依赖方向不变，全部在 portable 下可测。

registry A10 由 `pending` 升为 `implemented`，**不标 `tested`**：`automation: fault` 的真实断网/弱网注入与真实云 SDK 接入均未做，本轮证明的是**依赖方向与容量声明方式的架构判据**。

## 目标行为

| 判据 | 实现 | 关键设计 |
| --- | --- | --- |
| 本地关键链路不依赖云 | `LinkGraph` + `audit_local_autonomy` | 断言**传递闭包**不含云节点，两跳依赖同样被检出 |
| 本地静音脱离链路状态 | `availability(link, class)` | `(_, Local) => Available` 是一条恒等式，本地可用性不是链路状态的函数 |
| 断连只摘云能力 | `CapabilityGate::disconnect` | 循环**只遍历** `NodeClass::Cloud` 条目，本地能力不在作用域内 |
| 重连不恢复 Auto | `CapabilityMode::NeedsPrepare` | 恢复落到「待复核」而非「运行」，须一次显式 `activate()` |
| 容量必须量过 | `Capacity::declare(.., Sizing)` | `Unmeasured` 被拒；无零默认值——「没测过」与「测到 0」类型不同 |
| 越界全部报出 | `assess` | 逐项比对连接/内存/延迟，报出**全部**越界族 |
| 越界必须限流 | `audit_low_cloud` + `ThrottlePolicy` | 超预算却 `Absorb` 是违规——静默吞掉会把量过的天花板换回没量过的 |

## 实测数字

- `cargo test -p nai-upgrade-core --test cloud`：**21 passed / 0 failed**
  - 依赖图 6：纯本地通过、直连云依赖被拒、两跳依赖被检出、未声明节点可区分、关键节点声明为云可区分、自环与未知边被拒。
  - 可用性 3：本地在两种链路下均可用、云断连失败关闭、本地静音在断云时仍可用且审计干净。
  - 门控 5：断连只摘云能力、重连落到 `NeedsPrepare` 而非 `Active`、待复核须显式激活、转换不触碰本地能力、重复注册冲突。
  - 容量 7：未测量被拒、测量须点名实例与负载、测量基础被保留、预算内为 `Within`、越界报全三项、`Absorb` 是违规而 `RateLimit` 干净、预算内两种策略皆干净。
- `cargo xtask test --suite upgrade`：见下「门控」。
- `cargo xtask check --profile portable`：见下「门控」。

## 逐项验收映射（A10）

| 条款 | 覆盖 | 位置 |
| --- | --- | --- |
| 云端断连时 Local 实时链路不受影响 | 依赖闭包无云 + `availability(Local) == Available` | `audit_local_autonomy`、`availability` |
| 云端断连时本地静音不受影响 | 同上，且本地门控状态在断连转换中不变 | `CapabilityGate::disconnect` |
| 远程能力失效按其门控停用 | 云能力失败关闭，恢复入 Prepare，不自动恢复 Auto | `CapabilityGate`、`CapabilityMode` |
| 低云配置在申报实例与负载下测连接/内存/延迟 | `Measurement{instances, load_pct}` 强制 | `Capacity::declare` |
| 超预算可限流 | `assess` → `Throttled{breaches}` | `assess` |
| 不能写未经测量容量 | `Sizing::Unmeasured` 被拒，无默认值 | `Capacity::declare` |

## 门控

- `cargo xtask check --profile portable`（`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4`）：**EXIT=0**，73 个测试目标（56 `Running` + 17 `Doc-tests`；较 W25 的 72 目标多 1，即本轮新增的 `tests/cloud.rs`），0 失败。
- `cargo test -p nai-upgrade-core --test cloud`：**21 passed / 0 failed**。
- `cargo xtask check-deps`：18 包，依赖方向不变（`nai-upgrade-core` 仍为 shared 层）。
- `cargo xtask check --registry`：通过（A10 证据与状态一致）。
- `openspec validate w26-cloud-disconnect --strict --no-interactive`：`Change 'w26-cloud-disconnect' is valid`。
- clippy `--all-targets -D warnings` 与 `fmt --all --check` 在 portable 内一并通过。

## 未覆盖边界

- **真实断网/弱网注入未做**：`automation: fault` 的真实演练需要设备/轻云环境；本轮以宿主注入的 `CloudLink` 值覆盖逻辑分支，未在真实网络上拔线。
- **真实云 SDK 未接**：本 crate 无任何网络客户端，云可达性是宿主提供的值。
- **容量数字非本轮实测**：`LowCloudProfile` 里的数字是**已测量声明**的忠实表示；真实测量需在申报实例与负载下进行。
- **A11（多会话隔离）未做**：独立主线。
