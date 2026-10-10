## Why

`tests/registry.yaml` 里这条线的真实状态是三项 `pending`，且 notes 都把原因指向同一个空缺：

| 项 | 状态 | notes |
| --- | --- | --- |
| R11 离线升级与部署迁移 | pending | 「远程/轻云里程碑」 |
| A08 档位升级不重装内核 | pending | 「upgrade-core 未实现（W15/W25）」 |
| A09 空闲回收 | pending | 「媒体宿主里程碑」 |

三项同属 V3.6 §14「部署、档位与升级」，也正是 §31.4 的 **W25（档位升级、空闲回收、部署迁移 → A08、A09、R11）**。W15 已把**升级包**这条路径做出来了（校验、撤销、预检、事务、门控、成本），但升级器的另一半——**装完之后能力到底占不占资源、闲置多久才放手、离线环境怎么装、部署形态变了数据与权限怎么搬**——一行都没有。

规格把这一半写得很硬：

- §14.1 档位矩阵：「Lite → Standard → Pro 使用**同一迁移图**，不要求先装中间档」；「授权与商业权益只影响安装与启用，不改变契约与数据格式」。
- §14.4 成本口径末句：**「未启用的能力不占用进程、端口、显存或云请求」**——A08 与 A09 共用这一条判据。
- §14.3 部署迁移：**「一致导出导入、权限映射、哈希验证；SQLite 不挂多节点写；实时链路留在原部署，在维护窗口切换」**。
- A11 之外的 §16 原文：A09 要求**「未安装的模块没有后台进程、端口、显存与云请求；结束场次后按 TTL 卸载模型、结束临时 worker、关闭未用预览」**。

值得注意的是这条判据的**方向性**：它约束的是「没在跑的时候占多少」，而不是「跑起来要多少」。这类要求最容易在文档里被写成承诺、在代码里被漏成默认值——一个忘了关的预览轮询进程不会让任何测试变红，只会让用户的机器和账单慢慢变热。因此本轮的目标是**把「不用的东西不占资源」变成可执行、可审计的判据**，而不是再加一段说明文字。

复用基座已经就位：`nai-upgrade-core` 已有清单解析、`verify_package` 全链校验、`RevocationList`、`preflight` 与 journal 事务，本轮的离线安装与部署迁移直接建立在其上。

## What Changes

- **档位驻留账本（新增 `residency.rs`，A08/A09）**：把「一个能力占据哪些资源」建模成**声明式足迹**（`ResourceClaim { kind, name, amount }`，`kind` 为封闭四族 `Process | Port | Vram | CloudRequest`），并只在 `enable` 时**物化**。于是「禁用即零占用」是账本的**结构性质**而非约定：`held()` 对未启用的能力必然为空，`audit()` 会主动把「禁用却持有资源」报成违规。排他资源（进程名、端口）在 `enable` 时做冲突检查，被别的能力占着即拒绝而非覆盖。

- **空闲回收（`IdlePolicy` + `reclaim`，A09）**：空闲**不是立刻释放**——场次一结束就卸载模型，下一句要付一次完整重载，所以规格要求的是 TTL。每族一个 TTL（临时 worker 最短、模型最长，因为重载最贵），`reclaim(now)` 只释放 TTL 已到的声明，**把还没到期的留在手上并如实报告**（`held_back`），而不是提前清空制造一次冷启动。

- **档位迁移图（`Tier` / `tier_plan`，A08）**：`Lite | Standard | Pro` 的加装/移除集合是**数据**，`tier_plan(from, to)` 由两段差集并成一条路径，因此 Lite→Pro 与 Lite→Standard 用的是同一张图、**不需要先装中间档**；集合中不可能出现内核（不重装内核）。

- **离线安装（新增 `offline.rs`，R11）**：离线安装的**前置**是「每一个声明文件都在本地」。缺任何一个即**显式拒绝**并说明是哪个文件，绝不回退到抓取。真正的保证是结构性的：本 crate **没有网络依赖**，所以「不会偷偷下载」不是靠一个 `offline: bool` 开关维持的。

- **部署迁移（新增 `migration.rs`，R11）**：`Deployment::{Local, Hybrid, Distributed}` 与 `DeploymentState`（权限集合 + 证据版本 + 数据哈希）。`plan_migration` 强制**维护窗口**（有活动会话即拒，对应「实时链路留在原部署」）与**单写者**（Distributed 目标 + SQLite 多写者即拒，对应「SQLite 不挂多节点写」）；权限**无映射即拒绝**，不做静默丢弃。`verify_migration` 复验迁移后的**权限一致、证据版本一致、数据哈希一致**三项。`OutputSwitch` 以 key 幂等：同一次切换重放**只播报一次**（「输出切换不重复播报」）。

- **不越权声明**：本轮证明的是**驻留与迁移的架构判据**，不是「在真实设备上跑过 8 小时不涨内存」（R06 需 24h 长播、`manual`）也不是「真的从 Local 迁到了 Distributed」（需真实轻云实例）。registry 只补证据并如实留痕，`auto` 项 `pending → implemented` 且**不标 `tested`**，`manual` 项（A08/A09）按既有惯例只补证据与说明。

## Capabilities
### New Capabilities
- `deployment-lifecycle`：能力驻留与档位迁移图、按 TTL 的空闲回收与驻留审计、离线安装的拒绝语义、部署形态迁移的窗口/写者/权限/版本/哈希一致性与输出切换幂等。覆盖 registry **A08 / A09 / R11**。

### Modified Capabilities
- 无。既有 `upgrade-transaction`（升级包校验、事务、门控、成本）**不改**：本变更在其确认「包装得上」之后，回答「装完占多少、闲置怎么放、离线和迁部署怎么办」。

## Impact

- 代码：`crates/nai-upgrade-core/src/` 新增 `residency.rs`、`offline.rs`、`migration.rs`；`tests/` 新增 `lifecycle.rs`、`migration.rs`；`lib.rs` 增导出。**不新增 crate，不改 workspace 依赖**（维持 18 包与依赖方向）。
- 规格：新增 capability `deployment-lifecycle`；新增 `docs/deployment.md`、ADR 0027、报告 `docs/reports/<date>-w25-tier-idle-migration.md`。
- 依赖：**零新增**。复用既有 `sha2`/`semver`/`serde_json`/`nai-security::Operation`。
- registry：A08/A09/R11 补证据；A08/A09 的 `automation: manual` 与 R11 的真实轻云实例均未具备，故状态只到 `implemented`（**不标 `tested`**）。
- 明确不做：不接真实 GPU/显存探针（`Vram` 是声明式索赔，真实配额由宿主注入）；不做真实轻云实例与 TLS（§27.4 属独立任务）；不做 R06 的 24 小时长播与 B04 的设备延迟分位（均需设备）；不实现 A10（云断连）与 A11（多会话隔离）——两者是独立主线。

## Non-Goals

- 不用账本断言替代真实测量：本轮证明的是「账本不会给未启用能力留资源、TTL 到期一定释放」这条**结构性判据**，不宣称任何真实设备上的内存曲线。
- 不让闲置回收变成静默降级：`reclaim` 只释放资源、不改能力启用状态、**不删数据**；`disable` 在仍有会话服务时拒绝，与 `upgrade-transaction` 的降档门控取向一致。
- 不做双向迁移图：与 `nai-storage::migration` 的「只向前」一致，档位与部署的**回退**走移除集合/备份恢复，不新增反向迁移脚本。
