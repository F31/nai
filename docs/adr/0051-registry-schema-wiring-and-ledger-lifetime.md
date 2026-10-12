# ADR 0051：注册表形状契约接电，与偏差台账的存活期

- 日期：2026-10-12（W73）
- 状态：已接受
- 相关：ADR 0050（冻结基线与注册表的对账门控）、ADR 0049（V3.10 订正版）、ADR 0042 决策 4（「在册」≠「测得」）、ADR 0047 决策 1、`openspec/specs/admission-registry`、`openspec/specs/spec-driven-delivery`
- 依据章节：V3.10 §16、§48.5、§51.1、§56.2、§57.3

## 背景

W72 的报告在「未做与遗留」第 2 条登记了一项**未裁定**的事：

> **§56.2「立即订正」与「跨轮次收录」的张力**：§56 前言要求「如实登记」之后**紧接着**订正现行规范，
> 而 A12/A13（W54/W55 登记、W65 收录）与 B07（W68 登记、W71 收录）都是**跨轮次**收录。
> 本轮把「待收录」做成可判定状态，但**没有**裁定「多久算太久」。

本轮按建议从这一条开始，先用一轮核实把「历史到底发生了什么」读清楚——**核实改变了计划**。

### 核实一：三条规格要求以「schema 校验」为前提，而那句校验根本没跑

`openspec/specs/admission-registry/spec.md` 有三处把结论挂在 **schema 校验**上：

| 位置 | 原文 |
|---|---|
| `Single machine-readable admission registry` | 「**schema 校验 SHALL 拒绝**重复编号、未知 release_line/status、悬空映射与新增的旧 T/E/U 编号」 |
| `Release lines are declared…` | 场景「**The new line is not registered in the schema**」：`capability_pack` 写进注册表而 schema 不认 ⇒ 「schema 校验失败，MUST NOT 出现『**注册表写得进去、schema 认不出来**』的状态」 |
| `The frozen baseline and the registry reconcile…`（W72 新增） | 台账每条 MUST 有形状（该形状**只写在文档里**，无 schema 可依） |

**实测：`schemas/tests/registry.schema.json` 不被任何执行体消费。**

- 它不在 `scripts/contracts/validation.mjs` 的 `sources`（那里列了 11 份 schema）；
- 全仓无 `include_str!("…/schemas/tests/…")`；
- 全仓无任何 `*.mjs` / `*.rs` 读它。

⇒ 目前 `tests/registry.yaml` 的**形状**只由 `scripts/contracts/registry.mjs` 的手工检查把关。
这不是「重复定义」，而是**一层缺失**：手工解析**对未知键静默忽略**，`additionalProperties: false` 才会拒绝它。
**W72 恰好踩中了这个缺口**——本轮实测（把注册表喂给该 schema）：

```
registry 顶层键：schema_version, release_lines, acceptance, mapping, baseline_deviations
schema 校验通过： false
  - (root) additionalProperties must NOT have additional properties
  错误总数： 1
```

**只差一处**：W72 新增的 `baseline_deviations` 没被 schema 声明。**其余 88 条、六条发布线、映射段全部通过**
——所以该 schema **不是**文档残留：它的 `description` 随 W65/W71 更新过（写着「V3.10 §16 + §48.5 + §51.1 + §57.3 + ADR 0015 errata」），
它只是**从未接电**。一次「合法的往前推进一步」就把一个无人检查的契约变成了**假契约**。

### 核实二：`schemas/` 下的孤儿不止一个

对 `schemas/**/*.schema.json` 逐个查消费点，得到三类：

| schema | 消费点 | 判定 |
|---|---|---|
| `config/{runtime,provider,backup,binding-plan,avatar-2d}`、`plugin/{manifest,model,avatar,ui}`、`catalog/catalog`、`platform-matrix/platforms` | `validation.mjs` 的 Ajv（`platforms` 另有 `scripts/platform/matrix.mjs`） | 已消费 |
| `config/upgrade-package` | `crates/nai-upgrade-core/src/manifest.rs` 的 `include_str!` | 已消费（**只在 Rust 侧**） |
| **`tests/registry`** | **无** | **孤儿**（本轮接电） |
| **`rag-golden/case-set`** | **无**（B01 的 `notes` 引用它作为**结构说明**） | **孤儿**（本轮具名声明，见决策 3） |

⇒ 「有没有人读这份 schema」这件事，**当前没有任何守卫**。它能默默发生两次，就会发生第三次。

### 核实三：「紧接着」在历史上从未按字面发生过，且字面读法不可行

把「偏差出现 → 被基线收录」逐条读出来（判据是**各版本基线正文本身**，不是报告里的话）：

| 实例 | 偏差何时出现 | 何时收录 | 中间是否**跳过**升基线轮次 |
|---|---|---|---|
| A12–A14 | **W56**（ADR 0047 新建 V3.8，其 §48.5 的 A 行写 `A01–A08；A09–A11 补充`，而注册表**此刻已有** A12–A14） | W65（V3.9，A 行补 `**A12–A14**（W54/W55 就地新增）`） | **否**：W56 之后的下一次升基线就是 W65 |
| 爬坡 / H18 的不对称（E4） | W67（登记） | W71（V3.10） | **否**：W67 之后的下一次升基线就是 W71 |
| B07 | W68（登记） | W71（V3.10） | **否**：W68 之后的下一次升基线就是 W71 |

两处必须写清楚的**订正**（它们纠正 W72 报告里的表述）：

1. **A12–A14 不是「未及时收录」，是「从未登记」。** W54/W55 新增 A12/A13/A14 时，
   **活索引 §48.5 还不存在**（§48 是 V3.8 的新章）；当时存在的 `§39.5 验收总表索引` 是
   **V3.7 的版本专属章节**（§56.2 情形③），它的落后是**设计**，不是漂移。
   偏差是**在 W56 出现的**——那份活的跨版本索引**诞生时就落后于注册表**——
   而直到 W65 被收录，中间**九轮**没有任何一条记录写着「这里不一致」。
   ⇒ 它属于「**静默**漂移」，正是 W72 对账门控所覆盖的形状（同样的动作发生在今天，W56 当场变红）。
2. **已登记的偏差：零跳过。** 三条实例**全部**在下一次升基线轮次被收录。

⇒ 因此「紧接着」应当被读成 **「不晚于下一次基线修订轮次」**，而**不是**「同一轮内」，
也**不是**「N 轮之内」——理由不是「历史这样，所以放宽」，而是**字面读法不可行**：

- 基线被 ADR 批准后**不可变**，而就地订正有硬约束 **MUST 1:1 行替换**（§56.2）⇒
  **改 §57.3 只能发生在一次专门的升基线轮次里**。「登记之后紧接着订正」在实现轮次内**没有可执行的落点**；
- **轮数是外部观察值**：本仓库已有「MUST NOT 直接采用外部观察值」的纪律（§48.7 的阈值三件），
  取一个 `N`（3？9？11？）既无依据、又会随实例变化；
- **升基线轮次是仓库内的离散事件**，且是唯一可改 §57.3 的时刻 ⇒ **锚在事件上，不锚在计数上**。

## 决策 1：注册表的**形状契约接电**，形状层与语义层并存且必须一致

`schemas/tests/registry.schema.json` **是活契约**，`tests/registry.yaml` MUST 满足它；该断言 MUST 在 `portable` 与 `full` 两条 profile 内。

**为什么是接线，而不是删除它、也不是「只改 description」**：

- **不能只改 description**：那正是本轮发现的状态——一个看起来在管、实际不管的契约。留着比没有更坏，
  因为它会把「有 schema」当成证据。
- **不能删除**：三条既有规格要求以「schema 校验」为前提（见背景）；删掉它就得连带把规格改成
  「手写检查」——那是**降低**已经声明过的保证。
- **接电不是「同一检查第二处定义」**：手工检查管**语义**（家族完整性、能力包集合、计数、跨文件绑定），
  schema 管**形状**（类型、枚举、未知键）。两者只有 id pattern / 发布线枚举 / 状态枚举三处重叠，
  重叠处加**一致性断言**（见下）而不是各自漂移。

**形状层与语义层的一致性**（三处常量，MUST 逐字相同，否则失败并指名是哪一处）：

| # | 形状层（schema） | 语义层（脚本） |
|---|---|---|
| 1 | `acceptance.items.properties.id.pattern` 与 `mapping.d01_to_d14.patternProperties…targets.items.pattern` | `registry.mjs` 的 `ID_RE` |
| 2 | `release_lines.properties` 的键集合与 `acceptance.items.properties.release_line.enum` | `registry.mjs` 导出的 `RELEASE_LINES` |
| 3 | `acceptance.items.properties.status.enum` 与 `mapping…d01_to_d14…status.enum` | `registry.mjs` 的 `STATUSES` |

> 这一条是「**同一形状的定义不得超过一份**」的可判定形式：重叠**允许**，**不一致**不允许。

## 决策 2：台账条目 MUST NOT 跨基线版本存活（「紧接着」的可判定形式）

`baseline_deviations` 的条目形状扩为：

| 字段 | 必需 | 形状 | 含义 |
|---|---|---|---|
| `id` | 是 | `^(S\|R\|A\|B\|O\|P\|M\|H\|X)[0-9]{2}$` | 偏差涉及的验收编号 |
| `since_baseline` | 是 | `^V[0-9]+\.[0-9]+$` | 该偏差**相对于哪一份基线文本**成立 |
| `registered` | 是 | `^W[0-9]+$` | 登记轮次 |
| `carried_at` | 否 | `^W[0-9]+$`，且 MUST **大于** `registered` | 若该条目跨过一次升基线而**仍然维持**，记下结转轮次 |
| `reason` | 是 | 非空字符串 | 为什么尚未收录 |

**不变式**：**每一条的 `since_baseline` MUST 等于当前基线版本。**

```
在册 == 基线声明 ∪ { 条目的 id | 条目.since_baseline == 当前基线版本 }
```

后果：**升基线时，台账的每一条都必须被处置**，且只有两种处置——

- **收录**（基线声明已追上）⇒ 删条目（决策 3 的「自清理」本来就会因陈旧条目失败）；
- **结转**（本版确实收不了）⇒ 把 `since_baseline` 改成新版本，并记下 `carried_at` 与非空 `reason`。

**为什么必须显式**：结转要**改文件**、要**写轮次**、要**给理由**，三件事都会出现在 diff 里。
⇒「这一版故意不收它」从一个**沉默**变成一条**记录**。这与 ADR 0050 决策 2 的三条性质同构：
把台账的**长度**换成台账的**寿命**——长度可以靠沉默增长，寿命不能。

**为什么不设轮数上限**：见背景「核实三」的三条理由。**锚在事件（升基线）上，不锚在计数上。**

**逃生通道仍然可用**（否则就是死锁）：真实差异 + 相称条目 + 结转记录 ⇒ 一致。台账**当前为空**，
因为 W71 已把三处偏差全部收录——**空台账本身就是「W71 已全部收录」这条断言**，
所以本决策今天的实际成本是**零**，它的作用在**下一次**升基线时才收账。

## 决策 3：`schemas/` 下的每份 schema MUST 要么被执行体消费，要么被具名声明

新增守卫：`schemas/**/*.schema.json` 的全集 MUST 等于

```
被消费的（从三个来源**派生**，不写死）
  ∪ 具名声明的例外清单（每条 MUST 给非空理由）
```

**「被消费」的派生来源**（三条，覆盖本仓库全部现有形态）：

1. `validation.mjs` 导出的 `AJV_SOURCES` 的取值（⇒ `schemas/<值>.schema.json`）；
2. Rust 的 `include_str!("…/schemas/…schema.json")`（正则扫描 `crates/`、`xtask/`、`plugins/`）；
3. `scripts/**` 与 `tests/**` 的 `*.mjs` 里出现的 `schemas/…schema.json` 字面量。

**本轮的具名例外恰一条**：

| schema | 为什么不接线 |
|---|---|
| `schemas/rag-golden/case-set.schema.json` | 它是**载荷格式的文档陈述**（供题集产出方按结构手写 `*.json`）；**执行体**是 `xtask/src/golden.rs` 的 `parse_case_set`，它对每类缺陷产出**稳定 token**（`unknown_field`／`field_missing`／`case_set_schema_unsupported`…），调用方据此定位。再接一份 JSON Schema 会形成**同一形状的第二处判定**，而两份判定的失败**措辞不同**⇒ 接线只会让「为什么被拒」有两个答案 |

⇒ 判据不是「有没有 schema 文件」，而是「**谁在判它**，以及**为什么是它**」。

## 决策 4：本轮明确不改什么

- **不改基线正文**：`docs/直播数字人 + 智能体系统技术方案 V3.10.md` **零改动**。
  决策 2 是**规格与门控**层面的裁定，不动 §56.2 的正文——原因见「遗留 3」。
- **不改 `tests/registry.yaml` 的任何 `acceptance` 条目**：88 条、六条发布线、全部 `status` 原样
  （H18 与 B07 仍 `pending`）；**台账仍为空**。
- **不改 Rust 行为**：不新增 xtask 步骤（`engineering_checks` 已含两个 Node 测试文件）。
- **不改生成物**：能力目录与 `schemas/{config,plugin}` 契约零漂移（`cargo xtask schema --check` 的输入未动）。
- **不删 `rag-golden/case-set.schema.json`**（决策 3 的具名例外）、**不删 `COMPLETE_FAMILIES`**（ADR 0050 决策 3）。

## 后果与实施

- 交付：`schemas/tests/registry.schema.json`（补 `baseline_deviations` 定义）、
  `scripts/contracts/validation.mjs`（`AJV_SOURCES` 导出 + 注册表入列）、
  `scripts/contracts/baseline.mjs`（台账跨版判据）、`scripts/contracts/registry.mjs`（导出 `STATUSES`）、
  `tests/registry.test.mjs`、`tests/contracts/schema.test.mjs`、`tests/registry.yaml`（台账段注释）。
- 门控路径不变：`engineering_checks` → `registry::check`（`tests/registry.test.mjs`）
  与 `schema::node`（`tests/contracts/schema.test.mjs`）⇒ 新守卫自动进入 `portable` 与 `full`，
  **不需要新的 xtask 步骤**。
- **已知代价（如实登记）**：台账的版本判据要求**每次升基线都回头处置台账**。
  这是**有意的**——它的目的就是把「沉默」变成「记录」。代价是升基线轮次多一件必做事，
  而当台账为空时该成本为零。

## 遗留（如实登记，本轮不处理）

1. **语义类漂移仍只能靠人读**（沿用 ADR 0050 遗留 1）：决策 2 判的是**寿命**，不是**内容**——
   「这条理由是否成立」只能读出来。
2. **注册表之外的形状漂移**：本轮只给 `tests/registry.yaml` 接电。`schemas/` 之外的数据文件
   （如 `docs/reports/evidence/*`）没有形状契约，本轮不动。
   **并如实登记消费守卫的一个已知盲点**：`scripts/contracts/generate.mjs` 通过
   `schemas/${source}.schema.json` 这种**模板路径**消费 10 份 schema（它的 `jsonSources` 清单
   是 `AJV_SOURCES` 的子集，故今天不漏判）。模板路径的**具体取值**抓不出来，所以「仅由模板消费」的
   schema 会被守卫**误报为孤儿**——方向是**安全的**（多报一次，逼人做决定），
   但若将来出现这种形态，正确做法是把该清单也做成一条**派生来源**，而不是给它加例外。
3. **`§56.2` 自身的两处张力，均未裁定**：
   ① 它把 `§56` 列入「该版专属的版本章节」（情形③），而 §56.2 的内容是**一般规则**——
   V3.10 的 E5 正是按「一般规则」去改它的，两种读法都能自圆其说；
   ② 前言「紧接着订正」的字面读法不可行（本轮已论证），但**基线正文未改**。
   ⇒ 二者都需要一次基线修订轮次裁定，本轮只把**可判定的一半**（台账寿命）落地。
4. **W72 报告的表述订正**：其遗留第 2 条把 A12/A13 描述成「跨轮次收录」，本轮核实后更正为
   「**W54/W55 未登记、W56 的活索引诞生即落后、W65 收录**」；该报告的处置是**追加勘误段**（历史记录不改原文）。
5. **§57.5 与 §48.7 的待决策项**（能力包打包粒度、MCP/A2A 协议版本与认证、阈值三件等）：沿用 W71/W72 的登记，未动。
