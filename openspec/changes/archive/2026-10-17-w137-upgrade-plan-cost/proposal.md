# Proposal: W137 — O05 的成本来源：把「升级成本显示」接上控制面

## Why

`O05`「升级成本显示」登记 `pending`，`notes` 自己写明理由：**控制面尚未暴露 `nai-upgrade-core`
的 plan 与成本来源**，呈现层（W49 的 `upgrade.ts`，19 项）写好了却没东西喂它。
开工前逐环现测：

| 环 | 状态（现测） |
| --- | --- |
| ① 模型 | **已有**：`CostEstimate`（`crates/nai-upgrade-core/src/cost.rs`），四分量 `Known`/`Unknown`，`total()` 给 `Complete`/`Partial` |
| ② 执行体 | **没有**：控制面对 `/upgrades/plan` 0 命中；契约声明了三条 upgrade 操作，均未标 `implemented-loopback` |
| ③ 调用方 | **一半**：`apps/desktop/src/upgrade.ts` 有呈现口径，`control.ts` 的客户端**没有**升级方法 |
| ④ 真实产生者 | 包清单来自厂商发布的升级包；仓库内只有结构校验（`UpgradePackage::parse`） |

缺口的形状是**②与③之间少一条线**——而且①的答案本来就在仓库里，所以这条线是「接线」，
不是「造模型」。

勘定时还发现两件必须一起处理的事：

- 契约给 `/upgrades/plan` 复用了 `*jobResponses`（202 + `job_id`），但 plan **不下载、不暂存**，
  没有可轮询的进行中状态 ⇒ 发一个查不到东西的 `job_id` 是伪造可观测面。
- 请求体上限是 4096（命令口径），而包清单允许到 65536 ⇒ 一个合法清单会变成静默的 413。
  同一次改动还要把「拒绝后直接断开」改成**有界排空**：带未读数据关连接会给对端 RST，
  于是 413 **不可观测**——「拒绝了」成了只有服务端知道的状态（本轮实测到，不是推测）。

## What changes

- `schemas/openapi/control.yaml`：`/upgrades/plan` 改为 200 + `UpgradePlanView`，新增
  `UpgradePlanRequest`/`UpgradePlanView`/`CostView`/`CostComponents`/`CostLine` 五个 schema，
  打 `x-implementation-status: implemented-loopback`；生成物随 `cargo xtask schema` 再生。
- `crates/nai-control-api`：新增依赖 `nai-upgrade-core`，`upgrade_plan` 执行体 + 权限
  `runtime.upgrade` + 从 `MANIFEST_LIMIT` 派生的体上限 + 三道表示性闸门（币种 / 金额上界 / 清单不合规）。
- `apps/desktop/src/control.ts`：`parseUpgradePlan`（只认三个键）+ `ControlClient.planUpgrade`；
  `apps/desktop/src/upgrade.ts`：`COST_DECLARED_BASIS_NOTE`（未验签边界）。
- 判据：控制面 9 条（含「响应必须满足自己声明的 schema」）、`tests/ui/upgrade-plan.test.mjs` 10 条。
- 文档：ADR 0112、报告、`docs/control.md` 路由表、`docs/ui.md` 状态、dev-plan、knowledge，
  以及 5 处「本机缺 protoc」过期表述的**勘误段**（已发布记录不就地改）。

## Impact

- 新增依赖边 `nai-control-api → nai-upgrade-core`（shared → shared，`xtask check-deps` 允许）。
  `nai-upgrade-core` 只多导出一个 `MANIFEST_LIMIT` 常量（单一来源，避免控制面另立数字）。
- 契约生成物两处变更（Rust `SCHEMAS_JSON`、TS `http.ts`）。
- `O05` 仍 `pending`：本轮补的是**来源**；真实渲染的面板与生产接线未做，`tests/ui` 也不带 DOM。

## Alternatives considered

- **在 runtime 里开一个 `UpgradeSurface` 接缝、由装配根注入**（照 `MemorySurface` 的样子）：
  否决。`MemorySurface` 需要接缝是因为*状态*在 runtime；成本只依赖清单，控制面直接调
  `CostEstimate` 就够了。多一层接缝会引入「接缝声明了但没人实现」的常见故障——那正是本轮要避免的形状。
- **让控制面读某个目录里的清单**：否决。基线 §900 的数据根没有这样一个目录，
  凭空加一个等于发明约定；而请求里给清单不需要发明任何东西。
- **放宽 `parseCostEstimate` 以接受任意币种/大数**：否决。那会削弱一条已有 19 项覆盖的呈现判据，
  换取的是「偶尔能多显示一个数」。选择是**拒绝那个包**，方向取安全侧。
