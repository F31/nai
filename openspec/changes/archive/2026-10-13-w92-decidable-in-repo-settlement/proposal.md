# w92：清空 `decidable_in_repo` —— 两条 A2A 首版边界落到规格，一条画像项订正为 `needs_external`

## Why

`tests/pending-decisions.yaml` 的 **34 条**里，有 **3 条**停在 `decidable_in_repo`（"可在仓库内决定"）。
该态出生在**五态**设计里，只表达「**何处**能闭合」，表达不了「**已**闭合在哪」——W88 为此立了第六态
`decided_in_repo`（首例 PD-09），但**没有回头处理剩余 3 条**，于是它们一直悬在"建议怎么做"上。

逐条核实这 3 条后，发现**两件不同的事**：

- **PD-22 / PD-24 的落点是同一段活规格**：`openspec/specs/agent-harness/spec.md` 的
  `A2A inbound is an external request; outbound delegation is bounded`。该条正文写「委派链深度
  MUST 有上限（**初值 1**）」——"初值"一词恰好**暗示可调**，而 PD-24 要裁的正是"是否可配置"。
  ⇒ 决定 MUST 落到规格里：只写在台账上，它读起来仍然只是"建议"。
- **PD-34 的登记与事实不符**：它要求"三个画像各做**真实使用者访谈**并产出记录"，而基线 §54.2
  明写"本节**未经用户访谈验证**"、§57.5 明写"MUST 在 R1 实现期以**真实使用者验证**后才可作为档位依据"。
  闭合条件（真人访谈记录）**必然在仓外** ⇒ 把它登记为"可在仓库内决定"是**不实分类**
  （`decidable_in_repo` 读起来像"仓库能闭合它"，而仓库**不能**）。

## What Changes

- **PD-22 落规格**：A2A 入站补「**R1 首版入站只响应本机与本地信任配置中已登记的受信对端**；
  对第三方开放 MUST 作为一次**显式的版本决策**，MUST NOT 因请求自称可信或自称已获授权而放行」。
- **PD-24 落规格**：把「深度上限（初值 1）」在 **R1 首版**具体化为「上限 **MUST 为常量、
  MUST NOT 提供配置面**」——与基线 §46.3 的**一般描述**分层（基线说"初值"，规格说"首版是常量"）。
- **PD-34 订正处置**：`decidable_in_repo` → `needs_external`，`closure` = 三个画像的真实使用者访谈记录。
- **台账三态重排**：`decidable_in_repo` **3 → 0**、`decided_in_repo` 2 → 4、`needs_external` 20 → 21。

## Impact

- `tests/pending-decisions.yaml`（3 条）+ `scripts/contracts/pending-decisions.mjs`（`DISPOSITION_COUNTS`）。
- `tests/registry.test.mjs`：**清空一个态暴露了两处注入用例对该态存在的隐含依赖**——它们用
  `find(entry => entry.disposition === X)` 定位样本，态一空即 `assert.ok(entry)` 失败（或更坏：被
  顺手改成"找不到就跳过"而静默失去覆盖）。⇒ 样本定位改为**按字段存在**（`Object.hasOwn`），
  并新增「`decidable_in_repo` 计数为 0」的显式断言。
- `openspec/specs/agent-harness/spec.md` 一条 Requirement 被 **MODIFIED** 替换。
- **不新增能力、不改任何 `status`**：H13/H14 仍是 `capability_pack` + `pending`（A2A 未实现，
  `delegation_depth` 在 Rust 里**零命中**）。落地是**规范层**的（决定钉进契约），不是实现层的
  ——MUST NOT 被读作"A2A 已可用"。
