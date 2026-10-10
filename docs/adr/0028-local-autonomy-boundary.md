# ADR 0028：本地自治的边界——依赖方向作为值、容量的可构造性、重连进 Prepare

- 日期：2026-10-10
- 状态：Accepted
- 基线：V3.6 §30.4 A10（= V3.5 E07/E08）、§27.3（连接恢复入 Prepare）、§12.3；§31.4；承接 ADR 0027（部署生命周期）与 W15（ADR 0026）
- 变更：`openspec/changes/w26-cloud-disconnect/`
- 判据：`tests/registry.yaml` A10「云断连不伤本地链路」

## 背景

A10 的两半都是**否定式**判据，这是它和多数验收项不同的地方：

- E07：Local 实时链路及本地静音**不**依赖控制云；远程能力失效按其门控停用。
- E08：在申报实例与负载下测连接/内存/延迟，超预算可限流，**不能**写未经测量容量。

否定式判据不像「能不能开播」那样会被用户直接撞见：一条本不该存在的云依赖，只会在云真的断掉的那一次暴露；一个没人量过的容量数字，会在扩容那天以「怎么和计划不一样」的形式出现。所以本轮的难点不在功能，而在**把否定变成可执行的值**——什么形态的断言会随着代码腐化而变红。

四个问题必须先回答：逻辑放**哪个 crate**；「不依赖云」靠**什么**成立；容量数字**从哪来**；链路恢复后云能力**回到什么状态**。

## 决策

### D1 放进 `crates/nai-upgrade-core`（shared），不新开 crate

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 新增 `nai-cloud-core` | 依赖图、门控、容量各建一个 crate | **否决**：低云是**档位预设**下的一种容量声明（§14），与 W25 的 `Tier`/`Residency` 同源；另开 crate 会让「低云档位」与「档位迁移图」分居两处，同一条不变量出现两个真相。 |
| B. 放进 `nai-runtime` | 装配根掌握真实云连接 | **否决**：runtime 层在 portable 下不可单测；且 A10 的内容是**判据**（依赖方向、容量可构造性），不是接线——接线属于设备/轻云里程碑。 |
| C. 放进 `nai-kernel` | 会话与门控都是内核语义 | **否决**：kernel 只允许依赖 contracts（`xtask/src/deps.rs`），拿不到 `ErrorCode` 之外的共享约定，也无法复用 W25 的 `Violation` 稳定标记形状。 |
| D. 扩展 `nai-upgrade-core`（shared） | 复用 `ErrorCode` 与 `Violation` 约定，零新增依赖 | **采用**：分层不变，18 包不变，全部判据在 portable 下可测。 |

复用 W25 的 `Violation { token, subject }`，使驻留审计与自治审计产出**同构**报告条目。

### D2 「不依赖云」是一个值：依赖方向图 + 传递闭包

`LinkGraph` 节点分 `Local | Cloud`，边 `from depends_on to`；`cloud_reach(node)` 求可达云节点集合；`audit_local_autonomy` 对关键节点断言闭包为空。

| 方案 | 结论 |
|---|---|
| 只查直连依赖 | **否决**：`mute → clock → cloud.tts` 这类两跳依赖是最常见也最难在评审里看见的形态。 |
| 写成文档表格 | **否决**：表格不因后来加了一条边而变红；图断言会。 |
| 运行时探测（断网看看） | **否决作为唯一实现**：那是设备侧 `fault` 验收，不能替代静态可证的依赖方向判据。 |

三类违规分开报（未声明 / 不是本地 / 依赖云），因为修法不同：补声明、改归属、拆依赖。

### D3 容量只能从测量构造：`Sizing` 是唯一入口

`Capacity::declare(.., Sizing)`，`Unmeasured → CAPABILITY_NOT_READY`，`Measured(Measurement{instances, load_pct})` 必须点名条件。

| 方案 | 结论 |
|---|---|
| `Option<Measurement>` + 默认 0 | **否决**：计划里的 `0` 被下游读成「一次测量结果为 0」，而不是「没测过」。表达「没测过」的类型必须与「测过且是 0」不同。 |
| 只校验取值范围 | **否决**：范围合法不能证明数字是量出来的；`Measurement` 强制点名申报实例与负载。 |
| 只报第一个越界族 | **否决**：修完连接上限的人会以为已合规；`assess` 报出全部越界。 |

### D4 重连不自动恢复 Auto：`NeedsPrepare` 是第三个状态

`CapabilityMode::{Disabled, Active, NeedsPrepare}`。`disconnect()` 只摘云能力；`reconnect()` 把云能力放到 `NeedsPrepare`，不到 `Active`。

| 方案 | 结论 |
|---|---|
| 重连即 `Active` | **否决**：直接违反 §27.3「不自动恢复旧的 Auto 状态」——链路抖一下就把未复核的自动互动放回来。 |
| 重连还原断连前快照 | **否决**：快照还原的是权限与授权，而这些在断连期间可能已失效；恢复必须重走 Prepare 与权限复核，故只还原到「待复核」。 |
| 断连时全停（含本地） | **否决**：正是 A10 要禁止的行为；`disconnect()` 的循环只遍历 `NodeClass::Cloud` 条目，本地能力连遍历都不进。 |

## 后果

- 「本地静音不受云影响」不再是注释，而是 `availability(Local) == Available` 这条恒等式与 `disconnect()` 的作用域。
- 「不能写未经测量容量」不再是评审纪律，而是 `Capacity` 没有「未测量」表示的**类型事实**。
- 代价：图与容量都需要宿主在装配时提供（节点归属、链路状态、测量条件），比一个布尔开关多几个参数。这是有意为之——把「云可不可达」「量没量过」显式化，正是让它们可审计的前提。
- 报告形状统一：本轮与 W25 的审计条目同为 `Violation`，报告工具无须学第二种形状。

## 验证

- `cargo test -p nai-upgrade-core --test cloud` = 21 passed（依赖图 6、可用性 3、门控 5、容量 7）。
- `cargo xtask test --suite upgrade`、`cargo xtask check --registry`、`cargo xtask check --profile portable`（fmt + clippy `-D warnings` + workspace 测试）。
- `openspec validate w26-cloud-disconnect --strict --no-interactive`。

## 未做边界

- 不接真实云 SDK、不做真实断网/弱网注入、不做多云路由与故障转移；`automation: fault` 的真实演练属设备/轻云里程碑。
- `LowCloudProfile` 中的数字是**已测量声明**的忠实表示，不是本轮实测值。
- 不实现 A11（多会话隔离）——独立主线，见 ADR 0029。
