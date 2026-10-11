# 云断连与低云容量

本页说明 A10（V3.7 §30.4，旧编号 E07/E08）在代码里的落点：**本地关键链路对控制云的依赖方向**与**低云容量数字的来源**。实现位于 `crates/nai-upgrade-core/src/cloud.rs`（shared 层），与 W25 的档位/驻留/离线/迁移（[deployment.md](deployment.md)）同一 crate。

## 1. 依赖方向图：本地关键链路不含云

`LinkGraph` 把系统建模成有向图：节点带归属 `Local | Cloud`，边表示 `from depends_on to`。

```text
media.local_mute ──▶ media.clock ──▶ session.control      （本地关键链路，全 Local）
                                        │
                                        ╳  ← 任何指向 cloud.* 的边都会被 audit 检出
```

`audit_local_autonomy(&graph, critical)` 对每个关键节点做**传递闭包**求值，断言其可达的云节点集合为空。三类违规分开报，因为它们要修的地方不同：

| 稳定标记 | 含义 | 修法 |
| --- | --- | --- |
| `local critical node depends on cloud` | 关键节点（直连或 N 跳）可达云节点 | 拆依赖或把该节点移出关键路径 |
| `local critical node is not local` | 关键节点自身被声明为云节点 | 改归属 |
| `local critical node is undefined` | 关键节点没在图中声明 | 补声明 |

只查直连会放过 `mute → clock → cloud.tts` 这类两跳依赖；闭包把两跳与 N 跳一视同仁。

## 2. 可用性是链路状态与归属的函数

```rust
availability(link, class):
    (_,                    Local)               => Available
    (Reachable,            Cloud)               => Available
    (Unreachable,          Cloud)               => Unavailable { "cloud link unreachable" }
```

本地节点在两种链路状态下**都是** `Available`——这句话就是 A10 的核心断言，落成代码即不可绕过。`CloudLink` 是宿主提供的值，本 crate 不持有网络客户端。

## 3. 云能力门控：断连只摘云能力，重连进 Prepare

`CapabilityGate` 记录每个能力的归属与 `CapabilityMode`：

- `disconnect()`：**只遍历云能力**，把处于 `Active` 的摘到 `Disabled`。本地能力不在循环作用域内，故「本地不受影响」是结构性事实。
- `reconnect()`：把 `Disabled` 的云能力挪到 `NeedsPrepare`，**不**到 `Active`。这是 §27.3「不自动恢复旧的 Auto 状态」的类型化实现——只有一次显式 `activate()` 才能让它运行。

`NeedsPrepare` 与 `Active` 是两个状态，而不是一个布尔加一段注释。

## 4. 低云容量只能从测量构造

```rust
Capacity::declare(connections, memory_mib, latency_ms, sizing)
    Sizing::Measured(Measurement { instances, load_pct })  → Ok(Capacity)
    Sizing::Unmeasured                                     → Err(CAPABILITY_NOT_READY)
```

- **没有零默认值**：写进计划的 `0` 会被下游读成「一次测量结果是 0」，而不是「没测过」。表达「没测过」的类型必须和「测过且是 0」不同，故 `Unmeasured` 是一个显式分支而非 `Option::None` 配默认。
- `Measurement` 必须点名**申报实例数**（`> 0`）与**负载**（`1..=100`），把「在什么条件下量的」变成容量的一部分。

`assess(&capacity, &budget)` 逐项比对连接/内存/延迟三项上限，**报出全部**越界族（`connections over budget` / `memory over budget` / `latency over budget`），而不是只报第一项。

## 5. 超预算必须限流

`ThrottlePolicy::{RateLimit, Absorb}`；`audit_low_cloud(&profile)` 在**越界且策略为 `Absorb`** 时报 `budget breach absorbed instead of throttled`。规格说超预算**可**限流，而静默吞掉一次越界，正是把一个量过的天花板悄悄换回没量过的。`RateLimit` 下的越界是干净的。

## 6. 审计标记一览

| 标记 | 触发 |
| --- | --- |
| `local critical node depends on cloud` | 关键节点传递闭包含云节点 |
| `local critical node is not local` | 关键节点被声明为云节点 |
| `local critical node is undefined` | 关键节点未在图中声明 |
| `budget breach absorbed instead of throttled` | 超预算但策略为吸收 |

标记是**稳定的英文 token**（与 `Violation` 的既有约定一致），不随 locale 变化，报告可据此分类而无需 debug 构建。

## 7. 未做边界

- 不接真实云 SDK、不做真实断网/弱网注入、不做多云路由与故障转移——`automation: fault` 的真实演练属设备/轻云里程碑。
- `LowCloudProfile` 里的数字是**已测量声明**的忠实表示，不是本轮实测值。
- 不实现 A11（多会话隔离）——独立主线。
