# W76 实测报告：待裁定项的登记与寿命

- 日期：2026-10-12
- 基点：V3.10（ADR 0049）
- 变更：`openspec/changes/archive/2026-10-12-w76-pending-decisions-ledger/`
- ADR：[0054](../adr/0054-pending-decisions-ledger.md)

## 一、判断与范围

出发点是一个**盘点问题**——「离方案设计和规划任务还有哪些未执行」。全仓核实后得到：

- 主干线未达标 **18 条**（r1 pending 12 + r1 blocked 3 + r2 pending 1 + r2 blocked 2）；
- 能力包 **29 条**全部 `pending`（**设计上可选加装，不构成欠账**）；
- 产品验收矩阵 **T01–T16 / E01–E10 / U01–U12 / G0–G3 全部未执行**；
- D 表遗留 **D16 / D18 / D19** 未开始；
- 仓库内可控的剩余缺口（`nai-control-api` 的 deadline→panic、S08 的签名链与依赖审计）。

⇒ **未达标项与阻塞项都已登记在案。唯一一类「有载体而无台账」的东西，是基线的四章「待决策与待确认」。**
它们不决定「做什么」，但决定「哪些事还悬着」——而**悬着的东西没有寿命**。

**范围**：给这四章的条目建一份机器可读台账 + 它的校验，并把核实结论（哪些其实已有答案）
固定下来。**不改基线正文、不改任何 `acceptance`、不改 schema、不改 Rust、不新增 xtask 步骤、不新增契约名。**

## 二、核实

### 2.1 四章是一条累积链

| 章 | 措辞 | 新增条数 |
| --- | --- | --- |
| §18.2 | 「待确认」（首次） | **10** |
| §39.7 | 「沿用 §18.2 的全部事项，并新增」 | **11** |
| §48.7 | 「沿用 §39.7 的全部事项，并新增」 | **10** |
| §57.5 | 「沿用 §48.7 全部事项，并新增」 | **3** |

⇒ **去重后的唯一集合 = 34 条**。若不识别这条链，会数出 10+18+23+3 = 54 条（含重复）。

### 2.2 四条已被基线自身回答（本轮最关键的核实结论）

| 条目 | 基线已有的答案 | 位置 |
| --- | --- | --- |
| MCP `sampling` 是否存在可接受的启用场景 | **存在，但默认禁用**；启用须①本机管理员授权②计入预算③逐次记决策日志 | §46.2 |
| 上下文利用率观测线 | **本轮不设告警阈值**，只登记为观测项；外部观察值 MUST NOT 外推 | §41.4 |
| `a2a` 相关契约名是否设立 | **不新增** `mcp.tool@1`／`a2a.*` 之类契约名（三条理由）；H13/H14 的 `contract` 留空 | §46.5 |
| 能力包的打包粒度 | **已是三行独立包**：市场客户端 M01–M18、MCP 适配 H11/H12、A2A H13/H14 | §51.3 |

⇒ 这四条的处置是**已答性核实并关闭**，不是「新裁定」。与 W67 把「『通过 §47.2 的回归』是全六类还是
相关类」核实为**不是偏差**同型。

### 2.3 处置分布

| 处置 | 条数 | 例子 |
| --- | --- | --- |
| `answered_by_baseline` | **4** | 见 2.2 |
| `decidable_in_repo` | **5** | PD-09 静态数据加密的密钥机制、PD-22 A2A 入站开放性、PD-24 委派深度可否配置、PD-31 观众长期记忆字段集、PD-34 画像验证方式 |
| `needs_precondition` | **4** | PD-23（H13/H14）、PD-26（H05/H06/H16）、PD-27（H07/H08/H16）、PD-30（W61） |
| `needs_external` | **20** | §52.1 三条命题、市场项目 6 项、写实 2 项、平台/供应商资料 |
| `out_of_scope` | **1** | PD-20 技能定价与结算（基线自述范围外） |

### 2.4 与既有台账的关系（**不合并**）

`tests/registry.yaml` 的 `baseline_deviations` **现为空**（W71 已全部收录）。它是**已发生的偏离**台账，
本台账是**未决的问题**——**维度不同**。`baseline.mjs` 的对账管「基线 §57.3 ↔ registry」，
本台账管「基线待裁定章 ↔ 处置」，同构而对象不同。

## 三、裁定

见 [ADR 0054](../adr/0054-pending-decisions-ledger.md)。要点：

1. **五态封闭枚举，每态一个必填字段** —— 把「这条有没有人管」换成可判定的「那个字段在不在」；
2. **依据 MUST 指回真实存在的章节** —— 脚本只证「章节存在」，不证「该章节回答了该问题」（后一半留给人读）；
3. **外部条件 MUST 写成「取到什么算闭合」**；
4. **前置 MUST 落在真实存在的东西上**（registry 验收 id 或基线工作项 `WNN`）；
5. **覆盖性与分布逐章、逐类写死**；
6. **与 `baseline_deviations` 不合并**。

## 四、实现

| 产物 | 说明 |
| --- | --- |
| `tests/pending-decisions.yaml` | 34 条；每条 `introduced_in`／`also_in` + 一个处置 + 对应必填字段 |
| `scripts/contracts/pending-decisions.mjs` | `DISPOSITIONS`／`REQUIRED_FIELD`／`CHAPTER_COUNTS`／`DISPOSITION_COUNTS`／`validatePendingDecisions(ledger, deps)`；**deps 显式注入，缺任一项即报错** |
| `tests/registry.test.mjs` | **+5 个测试**（14 → 19），断言**并入既有文件** ⇒ 随 `check --registry` 执行，`engineering_checks` **零改动** |

**入参显式注入**的四个键：`baselineVersion`、`baselineText`、`acceptanceIds`、`workItemIds`。
`ledger.baseline` MUST 等于当前基线版本 —— 升基线后忘更新会红（一份对着旧基线的台账，
其「§NNN 存在」的结论对新基线不成立）。

## 五、非空洞注入

### 5.1 文件级（真篡改台账文件 → 跑门控 → 还原）

| 组 | 注入 | 实测结果 |
| --- | --- | --- |
| **F1** | 把 PD-25 的 `basis` 由 `§46.2：sampling` 改为 `§99：sampling` | **红**：`PD-25: references §99, which does not exist in the baseline` |
| **F2** | 把 PD-32 的 `introduced_in` 由 `§57.5` 改为 `§18.2` | **红**（两条）：`chapter §18.2 has 11 entries, the baseline introduces 10` + `chapter §57.5 has 2 entries…` |
| **F3** | 把首条 `disposition: needs_external` 改为 `disposition: later` | **红**（两条）：`PD-01: invalid disposition "later"` + `disposition needs_external covers 19 entries, the reviewed set has 20` |

每组跑完立即还原；**还原后复跑 19/19 全绿**（确认注入本身没留下痕迹）。

### 5.2 测试内覆盖（五组）

- **五态必填字段**：逐态删掉/清空其必填字段，各自被指名报出；
- **前置不存在**：`H99`（形状像验收 id 但注册表没有）→ `is not a registry acceptance id`；
  `W99`（基线工作项表没有）→ `is not a work item named by the baseline`；
  `以后再说`（根本不是 id）→ `neither a registry acceptance id nor a baseline work item`
  —— **三种错分开报**；
- **删条目**：按计数报出（`33 entries` vs 34 与章计数）；
- **id 重复**、**陈旧基线**（`V3.9`）、**引用越出 `basis`**（改 `also_in` 为 `§77` 也被抓）、
  **answered 条目不给 `§NNN`**（`at least one §NNN`）；
- **入参缺失**：四个键逐个删掉，各自必须报 `inputs are required`——**不做「缺了就跳过」的降级**。

**一条自查发现并修掉的问题**：首版断言写的是 `H99` 应报 `not a work item`，实际走的是
`neither a registry acceptance id nor a baseline work item` ⇒ 我据此把校验器的诊断**分细**
（「形状是 id 但不存在」与「根本不是 id」是两种错），而不是把断言改宽去迁就实现。

## 六、验证

### 6.1 台账

```
node --test tests/registry.test.mjs   →  19/19（14 → 19）
```

- **不新增测试目标**：断言并入既有文件 ⇒ 门控的 `Running` 目标数**不变**；
- **零新增依赖**（校验器只用 `node:fs/promises` 与既有的 `yaml`）；
- **不改基线正文**：§48.7／§57.5 属 §56.2 情形③的**版本专属陈述**，保留原文；关闭已答项只发生在
  台账与 ADR 里，进入基线要等一次升基线轮次（与 W68 的 B07、W67 的「影子在 R1 无落点」同一处置）。

### 6.2 附带修复：测试夹具的 bring-up 预算（阻塞本轮门控的既有缺陷）

第一次跑门控 **EXIT=1**，失败在 `crates/nai-runtime/tests/common/mod.rs` 的**准备阶段**
（`set_permission(...).unwrap()` → `StorageError { code: DeadlineExceeded, … }`）。
核实与修法见变更目录的 `design.md §7`。

| 观察 | 结论 |
| --- | --- |
| 本轮**零 `.rs` 改动**、且 W75 时本机门控绿 | **不是本轮引入**（同一份 Rust 代码一次绿一次红 ⇒ 间歇性） |
| 失败在夹具的 `set_permission(...).unwrap()`，非断言失败 | 在**准备阶段**，不在被测路径上 |
| 单独跑该包**也红**，但失败的是**另一个**测试（`:99` 而非 `:139`） | 同夹具不同行 ⇒ **并发争用**的 signature |
| `-- --test-threads=1` ⇒ **12/12 绿** | 争用确认（但 W70 §3.2 已否决「靠串行」） |

**根因**：夹具自造的 `Operation::new(Duration::from_secs(5))` **描述错了工作量**——
`open_store`／`provision` 做的是「全新数据根 + **全链迁移**（六个 `BEGIN IMMEDIATE`／`COMMIT`）
+ 建 workspace + 批量授权 + 存 profile」，那是 **bring-up** 的工作量。
`crates/nai-runtime/src/composition.rs` 的注释**逐字禁止**了这件事
（*Sized here once so the engine **cannot hold a second, drifting copy***），
且旁证明确：`nai-storage` 自己的夹具（`src/tests.rs:10`、`src/upgrade.rs:183`）用的就是 30 s，
**只有 `nai-runtime` 的夹具是 5 s**。

**修法**：复用 `nai_runtime::BRINGUP_OPERATION`（`lib.rs` 已重导出）；`operation()`（5 s）保留给
`store_operation()`／`verify_chain` 这类**逐次调用**。**不改任何产品代码。**
**为什么不是「放宽断言」**：失败在**测试前置准备**上，产品代码在那一步**不被考察**；
W70 §3.2 否决的三种处置（改阻塞 lock／只在夹具里等／`--test-threads=1`）都不适用。

**判定**：默认 test-threads 连跑 3 轮 → **3 × 12/12 绿**（38.35 / 38.66 / 39.02 s）。

### 6.3 门控

```
cargo xtask check --profile full   →  EXIT=0
  91 个测试目标 = 73 `Running` + 18 `Doc-tests`；858 passed / 0 failed
```

**与 W75 逐项相同**（91 = 73 + 18、858/0）——本轮**不增删任何测试**（断言并入既有文件、附带修复只改
夹具预算），故「目标 +0、项 +0」正是预期；门控日志里能看到新用例 `ok 15`／`ok 16`／`ok 17` 确实执行。

## 七、CI 复核

（待本轮 run 回填。）

## 八、遗留

1. **台账登记「处置」不替外部条件排期**：20 条 `needs_external` 的闭合仍取决于平台、供应商、
   市场项目与目标机——台账让它们**可追踪**，不让它们**可完成**。
2. **「已答」的语义判定仍靠人**：脚本只证「引用的章节存在」，不证「该章节真的回答了该问题」。
   这是**刻意留给人读**的边界（ADR 决策 2）。
3. **5 条 `decidable_in_repo` 的推荐尚未执行**：PD-09 会触碰平台密钥后端、PD-22／PD-24 属 A2A 边界、
   PD-31 属 R3 前置、PD-34 需真实访谈 ⇒ 每条都值得单独一轮，本轮只登记推荐。
4. **4 条 `needs_precondition` 跨到能力包与 W61**：能力包默认不装（INV-33），故它们在 R1 内
   **不可裁定是设计使然**，不是遗漏。
5. **台账自身的形状契约未接电**：本轮校验在**语义面**（Node 测试），`tests/pending-decisions.yaml`
   **没有**对应的 JSON Schema。是否需要，等它出现第二次结构变更再判——**不预造一份无人消费的契约**
   （W73 的教训：没人消费的契约会变假契约）。
6. **台账的数字与基线章节号会随基线漂移**：`ledger.baseline` 与章节引用会在升基线时红，
   这是**刻意的**；但 `CHAPTER_COUNTS`／`DISPOSITION_COUNTS` 是**人工维护的常量**，
   升基线轮次 MUST 一并复核（它们是「照抄基线某张表」的常量，W66 教训要求逐项写死，见 ADR 决策 4）。
7. **`§58.6` 登记的「需专门一轮」两处属偏差门控，已由 W72 关闭**；本报告不重复处理，
   也 MUST NOT 被读作把它们重新打开。

## 九、方法论教训

1. **「待裁定」不等于「无人负责」，但默认读法会滑向后者。** 一个没有字段负担的清单，
   在三次转述之后就会变成注释。给它一个必填字段是最小成本的解药。
2. **核实先于裁定：34 条里有 4 条不需要裁定。** 与 W67、W73「核实之后改了计划」是同一个模式的第三次出现——
   **盘点类任务的正确产出常常是「其中一部分已经解决了」**。
3. **「指向不存在的东西」与「没有指向」是同一件事。** 所以 `basis` 里必须有一个**能被解析的**结构，
   而不是一段听起来很像依据的话。
4. **总数对「被改类别」是瞎的。** 34 条一条不少、但其中两条互换了处置，只校验总数的实现看不见它
   ——这正是 W66 教训的第二次适用，故 `DISPOSITION_COUNTS` 必须与 `CHAPTER_COUNTS` 并存。
5. **断言写错时先问「是我的期望错了，还是实现错了」。** 本轮 `H99` 那处是**实现**的诊断太粗
   （把「形状是 id 但不存在」混进「不是 id」），修实现而不是放宽断言。
