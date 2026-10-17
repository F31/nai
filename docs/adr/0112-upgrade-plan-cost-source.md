# ADR 0112：O05 的成本来源——把「升级成本显示」接上控制面

- 状态：已接受（W137，2026-10-17）
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`（§1330 O05、§30.1 第 4 条、§900 数据根）
- 相关：ADR 0063（W85 OpenAPI 操作闭世界）、ADR 0086（W108 `schema --check` 工具链前提）、
  W49（呈现层）、W51/W54（控制面与桌面壳）、`docs/control.md`、`docs/ui.md`、`docs/upgrade.md`

## 背景

`O05`（「升级成本显示：云资源、下载、存储与预估运行增量准确列示」）自 W15/W49 起就是 `pending`，
notes 自己写明卡在哪：**控制面尚未暴露 `nai-upgrade-core` 的 plan 与成本来源** ⇒ 呈现层写好了、
没有任何东西喂它。开工前逐环现测：

| 环 | 现测结果 |
| --- | --- |
| ① 判定面/模型 | `CostEstimate`（`crates/nai-upgrade-core/src/cost.rs`），四分量 `Known`/`Unknown`，`total()` 给 `Complete`/`Partial{floor,unknown}`，`tests/gates.rs` 有覆盖 |
| ② 执行体 | 控制面 `match` 里对 `/upgrades/plan` **0 命中**；契约 `schemas/openapi/control.yaml` 声明了三条 upgrade 操作，均未标 `implemented-loopback` |
| ③ 调用方 | `apps/desktop/src/upgrade.ts` 有 `parseCostEstimate`/`costRows`/`totalPresentation`（19 项），`control.ts` 的客户端**没有**任何升级方法 |
| ④ 真实产生者 | 包清单（含 `cost`）来自厂商发布的升级包；仓库内只有结构校验（`UpgradePackage::parse` + `schemas/config/upgrade-package.schema.json`） |

于是缺口的形状很具体：**②和③之间少一条线**，而且①的答案本来就在仓库里。

## 决定

1. **`plan` 是同步纯读，不是作业。** 契约原先给 `/upgrades/plan` 复用了 `*jobResponses`（202 + `job_id`）。
   这一轮把它改成 200 + `UpgradePlanView`。理由是可判的：本路径**不下载、不暂存、不迁移**，
   没有任何可以轮询的进行中状态；发一个 `job_id` 却永远查不到东西，是**伪造一个可观测面**。
   `/upgrades/execute` 与 `/upgrades/{id}/rollback` 保持作业语义不变（它们真的会做多步事务）。

2. **清单是「被判对象」，不是「判定输入」。** 请求体是 `{"manifest": {…}}`——调用方给的是**要估的商品**，
   不是价格。成本由 `CostEstimate::from_package` 算，控制面与前端都**不**另算一份。
   `additionalProperties: false` 让「顺手递一个覆盖价」这类字段直接被拒。

3. **清单结构不在 OpenAPI 里重述。** `UpgradePlanRequest.manifest` 只是 `type: object`；
   真正的结构真相是 `schemas/config/upgrade-package.schema.json`，由 `UpgradePackage::parse` 读**同一份文件**。
   在契约里再抄一遍字段，两处迟早漂移，而「哪一份是真的」不可判。

4. **表示性闸门放在控制面，且一律选拒绝。** 清单 schema 比呈现口径宽，于是两处不肯合并的差异
   在这里被具名拒绝，而不是发一个自己契约不合格的 200：
   - 币种不是三字母大写码（清单允许 1–8 字符，`CostView.currency` 是 `^[A-Z]{3}$`）；
   - 金额超过 `2^53-1`（消费端 `Number.isSafeInteger` 会悄悄取整）；
   - 清单不合规（具名 `package manifest rejected: <stable token>`，**不**退化成「全未知」或「全 0」）。
   注意这**不是**放宽或收紧呈现口径：`parseCostEstimate` 的十条拒绝用例一条没动。

5. **`known: 0` 与「未知」在协议层也是两件事。** `CostLine` 是 `oneOf[{known},{unknown}]`，
   `additionalProperties: false` ⇒ 同时给两者或多给字段都不合法；缺分项由 `CostLine` 的必填保证不可能被省略。

6. **本路径不验签，并且必须说出来。** 请求里只有清单、没有包体，验签属于 execute 侧
   （`UpgradeTransaction`）。契约描述与 ADR 之外，呈现层新增 `COST_DECLARED_BASIS_NOTE`
   （「金额来自包清单声明，未验签；执行前由升级事务校验签名」），判据是对它做**内容锚点**断言——
   只判「非空字符串」等于没判。

7. **体上限从清单上限派生，不另立数字。** `PLAN_BODY_LIMIT = MANIFEST_LIMIT + 1024`；
   照抄命令那条 4096 会让一个合法清单变成静默的 413。同一次改动里把「拒绝后直接断开」改成
   **有界排空**：带着未读数据关连接会给对端 RST，`413` 于是**不可观测**——「拒绝了」成了一个
   只有服务端知道的状态。排空上限 `body_limit × 4`，超了就放弃（接受对端失败），这是有界的。

## 后果

- `O05` **仍保持 `pending`**：W137 补的是**来源**。`apps/desktop` 里没有真实渲染的升级面板
  （也没有 DOM 测试环境），"显示"这个动词还没有主语。与 O02 同惯例：界面出现不等于验收达标。
- §30.1 第 4 条（升级与回滚**提示**）从「同一缺失前置」变成**只缺一半**：`plan` 有了，
  journal/执行状态仍没有 ⇒ 「升级进行到哪一步、能否回滚」依然不可呈现。`docs/ui.md` 已就地订正。
- 契约生成物（`crates/nai-contracts/src/generated/control_http.rs`、`packages/contracts-ts/src/generated/http.ts`）
  本轮**由本机重新生成**：W136 结尾装好的 `protoc 3.21.12` 让 `cargo xtask schema --check` 第一次能在本机跑绿，
  这条改动从「只能手工同步 + 靠 CI 兜底」的高风险路径变成普通轮次。5 处「本机缺 protoc」的过期表述
  按「已发布记录只追加勘误」处置（dev-plan W137 小节与 `docs/knowledge.md` 的勘误段）。

## 未做（具名）

- 下载/暂存/执行/回滚三条路径未实现（本 ADR 只动 `plan`）。
- 计划视图不含 preflight 事实（磁盘、平台、依赖闭包）——那需要 `HostFacts`/`Probe` 的真实来源，仓库内没有。
- 成本总额不在服务端算：`Total` 已有实现，但呈现口径的合计与下界在桌面端（`totalPresentation`），
  本轮不新增第四个实现。
- 无 UI 渲染：`main.tsx` 没有升级面板，`tests/ui` 也不带 DOM。
