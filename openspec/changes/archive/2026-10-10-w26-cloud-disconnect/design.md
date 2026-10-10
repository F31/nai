## Context

A10 要成立，必须先回答四个问题：这段逻辑**放哪**；「不依赖云」靠**什么**成立；容量数字**从哪来**；链路恢复后云能力**回到什么状态**。四个问题的答案都要能在 portable 下被证伪，否则「轻云里程碑」会退化成一页说明。

基线：V3.6 §30.4 A10、§27.3（重连入 Prepare）、V3.5 E07/E08；承接 W25 的 `deployment-lifecycle`（档位、驻留、TTL、离线、迁移）。

## Decisions

### D1 放进 `crates/nai-upgrade-core`，不新开 crate

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 新增 `nai-cloud-core` | 依赖图、门控、容量各建一 crate | **否决**：低云是**档位预设**下的一种容量声明，与 W25 的 `Tier`/`Residency` 同属 §14 主线；另开 crate 会让「低云档位」与「档位迁移图」分居两处，同一条档位不变量有两个真相。 |
| B. 放进 `nai-runtime` | 装配根掌握真实云连接 | **否决**：runtime 层在 portable 下不可单测；且 A10 的内容是**判据**（依赖方向、容量可构造性），不是接线。 |
| C. 扩展 `nai-upgrade-core`（shared） | 复用 `ErrorCode` 与 `Violation` 稳定标记约定，零新增依赖 | **采用**：与 `xtask/src/deps.rs` 分层一致，18 包与依赖方向不变。 |

复用了 W25 的 `Violation { token, subject }` 约定，使两类审计（驻留、自治）产出**同构**的报告条目——报告工具不必为第二个 capability 学第二种形状。

### D2 「不依赖云」是一个值：依赖方向图 + 传递闭包

`LinkGraph` 的节点分 `Local | Cloud`，边是 `from depends_on to`；`cloud_reach(node)` 沿边求可达云节点集合。

- **否决「只查直连」**：`mute → clock → cloud.tts` 这种两跳依赖是最常见也最难在评审里看见的形态，只查直连会全部放过。传递闭包把两跳与 N 跳一视同仁。
- **否决「写成文档表格」**：文档说「静音不需要云」不会因为后来加了一条边而变红；图断言会。
- **否决「运行时探测」**（「断网看看还响不响」）：那是设备侧的验收演练（A10 的 `fault` 级），不适合当成架构判据的**唯一**实现；本轮先把静态可证的依赖方向做出来。

三类违规分开报（未声明 / 不是本地 / 依赖云），因为它们要修的地方不同：补声明、改归属、拆依赖。

### D3 容量只能从测量构造：`Sizing` 是唯一入口

`Capacity::declare(connections, memory_mib, latency_ms, sizing)`，`Sizing::{Measured(Measurement), Unmeasured}`。`Unmeasured` → `CAPABILITY_NOT_READY`。

- **否决 `Option<Measurement>` + 默认 0**：`0` 写进计划会被下游读成「一次测量结果是 0」，而不是「没测过」。表达「没测过」的类型必须和表达「测过且是 0」的类型不同。
- **否决「只校验取值范围」**：范围合法不能证明数字是量出来的。`Measurement` 强制点名**申报实例数与负载**，把「在什么条件下量的」变成构造容量的一部分，而不是报告里的一句附言。
- `assess` **报出全部越界族**而非只报第一个：只报第一项会让修完连接上限的人以为已经合规。

### D4 重连不自动恢复 Auto：`NeedsPrepare` 是第三个状态

`CapabilityMode::{Disabled, Active, NeedsPrepare}`。`disconnect()` 把云能力从 `Active` 摘到 `Disabled`；`reconnect()` 把云能力从 `Disabled` 挪到 `NeedsPrepare`，**不**到 `Active`。

- **否决「重连即 Active」**：直接违反 §27.3「不自动恢复旧的 Auto 状态」——链路抖一下就把未复核的自动互动放回来，是安全回退。
- **否决「重连还原断连前快照」**：快照还原的是**权限与授权**，而这些东西在断连期间可能已经失效；恢复必须重新走 Prepare 与权限复核，故只还原到「待复核」而非「在跑」。

`disconnect()` 只遍历 `NodeClass::Cloud` 的条目，本地能力连遍历都不进——「本地不受影响」不是一句判断，是循环的作用域。

## Verification

- `cargo test -p nai-upgrade-core --test cloud`：21 项（依赖图 6、可用性 3、门控 5、容量 7）。
- `cargo xtask test --suite upgrade`：`nai-upgrade-core` 全量（含本轮 `cloud.rs`）。
- `cargo xtask check --registry`：A10 证据与状态一致性。
- `cargo xtask check --profile portable`：fmt + clippy `-D warnings` + workspace 测试。
- `node node_modules/@fission-ai/openspec/bin/openspec.js validate w26-cloud-disconnect --strict --no-interactive`。

## Environment boundaries

- 云链路是**宿主提供的值**，本 crate 不持有任何网络客户端；「云不可达」由调用方注入。
- `LowCloudProfile` 中的容量数字是**已测量声明的忠实表示**，不是本轮实测值；真实测量需申报实例与负载的环境。
- `automation: fault` 的真实断网/弱网注入与真实云 SDK 接入属设备/轻云里程碑，本轮未做，故 registry 只到 `implemented`。
