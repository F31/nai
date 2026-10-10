## Why

`tests/registry.yaml` 里 A10 是 `pending`，notes 只有一句「轻云里程碑」。V3.6 §30.4 把它拆成两半（旧编号 E07 与 E08），两半都不是「多云更好用」这类体验话，而是**可以被证伪的判据**：

| 出处 | 原文 |
| --- | --- |
| §30.4 A10 | 云端断连时 Local 实时链路与本地静音不受影响；低云配置在申报实例与负载下测连接数、内存与延迟 |
| §30.4 映射 | E07 云端断连、E08 低云配置 → A10 |
| V3.5 E07 | Local 实时链路及本地静音不依赖控制云；远程能力失效按其门控停用 |
| V3.5 E08 | 在申报实例与负载下测连接/内存/延迟，超预算可限流，**不能写未经测量容量** |
| §27.3 | 连接恢复后进入 Prepare 与权限复核，**不自动恢复旧的 Auto 状态** |

三句话里有两句是**否定式**的：本地链路**不**依赖云，容量**不**许写没量过的数。否定式的判据最容易在文档里被写成承诺、在代码里被漏成默认值——没有人会因为「静音其实要过一次云网关」而看到红灯，直到云真的断了的时候。

W25 刚把「档位与部署」建起来（`Tier`、`Residency`、`IdlePolicy`、离线与迁移），A10 的「低云配置」正是**同一条档位主线**的下一段：低云是一个档位预设下的容量声明，而「云断连不伤本地」是它必须成立的前提。因此本轮把 A10 建立在这套基座上，而不是另起一套。

## What Changes

- **依赖方向图（`LinkGraph` + `audit_local_autonomy`）**：把「本地关键链路不依赖云」变成**值**。节点分 `Local | Cloud`，边是 `from depends_on to`；`cloud_reach(node)` 沿依赖求传递闭包，审计对每个关键节点断言其闭包**不含任何云节点**。直连违规与**两跳之外**的违规都会被指出，且「未声明」「被声明成云节点」是另外两种可区分的违规标记。文档写「静音不需要云」不会回退，图断言会。

- **可用性是链路状态与节点归属的函数（`availability`）**：`Local` 节点在 `Reachable | Unreachable` 下**都是** `Available`——这句话本身就是 A10 的核心断言，落成代码即不可绕过。`Cloud` 节点在断连时以稳定标记 `cloud link unreachable` 失败关闭。

- **云能力门控（`CapabilityGate`）**：断连时**只有**云能力被从 `Active` 摘下，本地能力原样不动（`LinkTransition.deactivated` 为空）。重连时云能力进入 `NeedsPrepare` 而**不是** `Active`——「不自动恢复旧的 Auto 状态」由类型强制，`activate()` 必须被显式调用。

- **低云容量只能从测量构造（`Capacity::declare(.., Sizing)`）**：`Sizing::Unmeasured` 一律 `CAPABILITY_NOT_READY` 拒绝，**没有零默认值**（写进计划的 0 会被读成一次测量）。`Measurement` 必须点名**申报实例数与负载**（`instances > 0`、`load_pct ∈ 1..=100`）。`assess` 逐条比对连接/内存/延迟三项上限并**报出全部**越界族，而不是只报第一个。

- **超预算必须限流（`ThrottlePolicy` + `audit_low_cloud`）**：`Absorb` 是审计要抓的状态——规格说超预算**可**限流，而静默吞掉一次越界，正是把一个量过的天花板悄悄换回没量过的。`RateLimit` 下的越界是干净的。

- **不越权声明**：本轮证明的是**依赖方向与容量声明方式的架构判据**，不是「在真实多云实例上拔过网线」也不是「在申报负载下真的测到了这些数」。云链路是一宿主提供的值，`LowCloudProfile` 里的数字是**已测量声明**的忠实表示，不是本轮实测的。registry 只补证据并如实留痕，`pending → implemented` 且**不标 `tested`**。

## Capabilities
### New Capabilities
- `local-autonomy`：本地关键链路的云依赖方向与审计、链路状态下的可用性与云能力门控（含重连不恢复 Auto）、低云容量的可构造性（未测量即拒）与超预算限流审计。覆盖 registry **A10**。

### Modified Capabilities
- 无。既有 `deployment-lifecycle`（档位、驻留、TTL、离线、迁移）**不改**：本变更在其「低云档位」之上回答「断云时谁还活着、容量数字从哪来」。

## Impact

- 代码：`crates/nai-upgrade-core/src/` 新增 `cloud.rs`；`tests/` 新增 `cloud.rs`（21 项）；`lib.rs` 增导出。**不新增 crate，不改 workspace 依赖**（维持 18 包与依赖方向）。
- 规格：新增 capability `local-autonomy`；新增 `docs/cloud.md`、ADR 0028、报告 `docs/reports/2026-10-10-w26-cloud-disconnect.md`。
- 依赖：**零新增**。复用既有 `nai_contracts::v1::ErrorCode` 与 `Residency` 的稳定标记约定（`Violation`）。
- registry：A10 补证据并按实现 `pending → implemented`（`automation: fault`，真实云与真实断网演练未做，**不标 `tested`**）。
- 明确不做：不接真实云 SDK、不做真实断网/弱网注入、不做多云路由与故障转移；不实现 A11（多会话隔离）——它是独立主线。

## Non-Goals

- 不用图断言替代真实断网演练：本轮证明的是「本地关键路径的依赖闭包不含云」这条**结构性判据**，不宣称任何真实网络条件下的可用性曲线。
- 不让断连静默降级：云能力**失败关闭**（`Unavailable`/`Disabled`），不降格成本地能力继续跑；恢复必须过 Prepare 与权限复核。
- 不发明容量数字：`Capacity` 没有「未测量」的表示，无法被写成 0 或默认值糊弄过去。
