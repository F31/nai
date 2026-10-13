# ADR 0070：清空 `decidable_in_repo` —— A2A 首版边界落规格、画像验证订正为 `needs_external`

- 状态：接受
- 日期：2026-10-13
- 相关：[ADR 0054](0054-pending-decisions-ledger.md)（台账六态的原始设计与其遗留 3 的**收尾**）、
  [ADR 0066](0066-decided-in-repo-landing.md)（第六态与落地点；本 ADR 兑现其遗留 1）、
  [ADR 0067](0067-viewer-memory-single-answering-point.md)（其遗留 4 登记的「余下 3 条」）、
  [ADR 0061](0061-reference-field-verification.md)、
  `tests/pending-decisions.yaml`、`scripts/contracts/pending-decisions.mjs`、`tests/registry.test.mjs`、
  `openspec/specs/agent-harness/spec.md`、
  `docs/reports/2026-10-13-w92-decidable-in-repo-settlement.md`、
  `openspec/changes/archive/2026-10-13-w92-decidable-in-repo-settlement/`

## 背景

`tests/pending-decisions.yaml` 的 34 条待裁定项里，`decidable_in_repo` 一态在 W89 之后还剩 **3 条**：
PD-22／PD-24（A2A 首版边界）与 PD-34（用户画像的验证方式）。ADR 0066（W88）立第六态解决 PD-09 时
**没有回头处理它们**；ADR 0066 遗留 1 与 ADR 0067 遗留 4 把这件事逐代传了下来。

`decidable_in_repo` 的必填字段是 `recommendation`——**"建议"**。一条停在这里的条目，判据只钉住
「有一个建议」，**不钉**「这个建议落地了没有」。⇒ 该态是**过渡态**：一条建议要么被采纳并落地
（→ `decided_in_repo`），要么被证明闭合在仓外（→ `needs_external`）。

## 证据（本轮实测）

### 1. PD-22 / PD-24 的落点是同一段活规格

`openspec/specs/agent-harness/spec.md` 的 `A2A inbound is an external request; outbound delegation is bounded`：

- 该条正文写「委派链深度 MUST 有上限（**初值 1**）」——"初值"一词**恰好暗示可调**，而 PD-24 要裁的
  正是"是否可配置"。
- 基线 §46.3（line 2164）：「委派边界：…**委派链深度上限初值 1（不得再委派）**」；§40.4（line 1930）：
  「有界：…委派深度全部有界，**越界具名终止、不静默截断**」。
- `grep`「委派链深度」在 **Rust 零命中**（无 `DELEGATION_DEPTH`／`delegation_depth`／`max_delegation`）
  ⇒ 深度上限**没有实现**；H14 是 `capability_pack` + `pending`，与之一致。
- 基线 §48.7（line 2362）把「**A2A**：入站入口是否对第三方开放；…委派深度上限是否可配置」**登记为待决策**
  ⇒ 规格补上是从"待决"变"已决"。

**规格原来漏掉的那半边**：原文只说「对端身份 MUST 由本地信任配置**派生**」——它约束**已到达的请求
如何取身份**，**没有**约束**谁可以到达**。两者是不同断言：前者是"身份从哪来"，后者是**可达集**。
只写前者，实现完全可以"接受任意第三方入站、再查它的身份、查不到就当匿名外部请求处理"——**规格允许这种读法**。

### 2. PD-34 的登记与事实不符

- 基线 §54.2 标题即「用户画像（**初版，未经验证**）」；§57.5（line 2647）：「**用户画像的验证方式**：
  §54.2 初版未经访谈，**MUST 在 R1 实现期以真实使用者验证后才可作为档位依据**」。
- ⇒ **验证方式已被基线答出**（"以真实使用者验证"），未闭合的是**验证的执行**（真人访谈）——
  它必然在**仓外**。
- ⇒ 把它登记为 `decidable_in_repo`（"可在仓库内决定"）**是不实分类**：它读起来像"仓库能闭合它"，
  而仓库**不能**。

## 决定

### 决定 1：PD-22 落到规格（入站来源限定）

采纳建议并落到 `agent-harness` 那条 A2A Requirement 的**新正文**：

> **R1 首版入站只响应本机与本地信任配置中已登记的受信对端**；对第三方开放 MUST 作为一次**显式的版本决策**，
> MUST NOT 因请求自称可信或自称已获授权而放行。

并新增 Scenario `A third party asks to be admitted`：未登记对端发起入站 ⇒ **拒绝并具名**（MUST NOT 以
「无响应」代替拒绝——沿用 §40.4「不静默截断」与 §44 的既有纪律）。

### 决定 2：PD-24 落到规格（深度不配置化）

> **R1 首版委派链深度上限 MUST 为常量、MUST NOT 提供配置面**。

并新增 Scenario `Delegation depth is asked to be configured`。

**为什么不把"配置化前置条件"也写进规格**：那两句（协议验证之后 + 引入时的两项要求）是**前瞻条件**，
不是 R1 可判定的规范；写进规格会让正文膨胀且**不可判定**（"协议验证之后"没有仓库内判据）。
它们留在台账的 `recommendation`（注解性留存）与本 ADR 里。

### 决定 3：PD-34 订正为 `needs_external`

`closure` = 「三个画像各产出一份**真实使用者访谈**记录（「画像-档位映射已验证」）；在此之前 §54.2 MUST NOT
被用作「Lite 该多 Lite」的论据」。

**排除另两个候选**：

- **不是 `decided_in_repo`**：该态要求「决定已作出**且已落地**」。PD-34 的闭合条件（访谈记录）在仓库里
  **不存在** ⇒ "已落地"不成立。
- **不是 `answered_by_baseline`**：§57.5 没给出 recommendation 的颗粒度（三个画像**各自**验证）；
  更关键，§54.2 的自我声明恰恰是**"未经验证"**——把一条"基线说它没验证"的项记成"基线回答了它"，
  是**不实声明**（W88 对 PD-09 排除了同一个陷阱）。

`recommendation` **原文保留**（先例 PD-09：它是**被采纳的那份建议**，删掉就丢了"决定了什么"）。

## 台账计数

| 处置 | 改前 | 改后 |
| --- | --- | --- |
| `answered_by_baseline` | 4 | 4 |
| `decidable_in_repo` | 3 | **0** |
| `decided_in_repo` | 2 | **4** |
| `needs_precondition` | 4 | 4 |
| `needs_external` | 20 | **21** |
| `out_of_scope` | 1 | 1 |
| 合计 | 34 | 34 |

**`decidable_in_repo` 归零是一次断言，不是"没有断言"**：它断言「没有任何条目还停在『可在仓库内决定』
的门口」。`DISPOSITION_COUNTS` 逐项写死（W66 教训：「总数对『被改类别』是瞎的」）。

## 不越权：基线的"一般描述"与规格的"R1 具体化"分层

| 载体 | 句子 | 对象 |
| --- | --- | --- |
| 基线 §46.3 | 委派链深度**上限初值 1**（不得再委派） | 该协议的**一般描述**（"初值"= 起点值） |
| 规格（本轮） | **R1 首版**委派链深度上限 **MUST 为常量、MUST NOT 提供配置面** | **R1 首版**的具体化 |

两者**不矛盾**：后者是前者在**首版范围内的取值决定**，正是 PD-24 要裁的那个自由度。
⇒ **基线正文本轮一字不动**；若要进正文，走升版收录（与 W65／W87 节奏一致）。

## 「清空一个态」暴露的守卫隐含依赖（本轮的技术教训）

把 `decidable_in_repo` 的计数从 3 改成 0 之后，**两处测试用例会失去其作用对象**——它们按**处置**
定位样本：

```js
const item = stray.entries.find(entry => entry.disposition === disposition);
```

实测：改成 0 后 `node --test tests/registry.test.mjs` 立刻 **2 条红**（#16、#22）。

**危险的修法**（MUST NOT 采用）：`if (!item) continue;`——它把「该态没有实例」与「该字段的引用不再被扫」
**混为一谈**，是 W66「删字段照样绿」的同族形状。

**采用的修法**：样本定位改为**按字段存在**（`Object.hasOwn(entry, field)`），与 W80／W83／W88 三次确立的
「触发条件 MUST 只由**字段是否存在**决定」同源。理由：用例断言的是"**这个字段**里的 §NNN 被扫"，
样本就该按"谁有这个字段"选——按处置选会让**一次合法的数据变更**（清空某态）**改变判据的覆盖面**。

空态**被显式断言**（计数 0 ⇒ 数据里 MUST 确实没有该态条目），并新增本轮成果断言
（`DISPOSITION_COUNTS.get('decidable_in_repo') === 0` 且数据侧无该态条目）。

## 后果

- **正面**：A2A 首版边界在实现之前被钉进契约（H13／H14 仍是 `capability_pack` + `pending`）；
  台账六态不再有"过渡态"堆积；测试对"态为空"不再有隐含依赖。
- **代价**：`decidable_in_repo` 计数为 0 ⇒ 该态的**数据侧注入**暂时无实例（由 `shapeErrors()` 与常量表
  断言在**表格层**继续看守，与数据无关）。
- **限制**：落地是**规范层**的——MUST NOT 被读作"A2A 已可用"。`delegation_depth` 在 Rust 仍零命中。

## 遗留

1. **基线收录**：PD-22／PD-24 的裁定若要进基线正文，走下次升版（§46.3 的"初值 1"与规格的"首版常量"分层）。
2. **A2A 实现**：H13／H14 属 `capability_pack`（INV-33），R1 不装；实现时 MUST 兑现本条规格的
   「具名拒绝」与「上限为常量」。
3. **PD-34 的外部闭合**：三个画像的真实使用者访谈记录仍缺（`needs_external`）。
4. **W90 §八遗留 1／3／4／5** 与 W91 的跨语言键级消费**未动**。
5. ADR 0066 遗留 1 与 ADR 0067 遗留 4 的「余下 N 条 `decidable_in_repo`」**在本轮结清**（本条即其收尾）。
