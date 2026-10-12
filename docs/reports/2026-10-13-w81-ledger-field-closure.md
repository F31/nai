# W81 · 台账字段集的封闭性（扫描清单之外 MUST 为空）

- 日期：2026-10-13
- 基线：V3.10（`docs/直播数字人 + 智能体系统技术方案 V3.10.md`）
- 裁定：[ADR 0059](../adr/0059-ledger-field-closure.md)
- 变更：`openspec/changes/archive/2026-10-13-w81-ledger-field-closure/`

## 一、判断与范围

W80 把待裁定台账的引用扫描域收成**一份导出的单一清单** `CITATION_FIELDS`，并**遍历它**——
修掉了「清单内字段整片漏扫」。本轮核实这个修法的**边界**：它是一条**开世界扫描**，
而**遍历一份清单的守卫，其盲区恰好是清单的补集**。

**范围**：只改 `scripts/contracts/pending-decisions.mjs` 与 `tests/registry.test.mjs`；
**不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边**。

## 二、审实

### 2.1 洞三（数据级）：清单之外的字段**没有任何人管**

`CITATION_FIELDS` 是清单；`validatePendingDecisions` 只遍历清单。**清单之外的键既不被扫、也不被拒**。
**探针实测（只用公开 API，不改被测文件）**：

```
=== 注入 UA：给 PD-01 加未知字段 rationale，内含 §99.9 ===
errors = []                                  ← 完全静默

=== 注入 UB：未知字段 rationale = "见 §40.4"（合法章节） ===
errors = []

=== 字段覆盖关系 ===
未覆盖字段 = id, question, disposition
```

⇒ **给台账加一个字段，它的引用就永久不受检**——与 W80 修掉的洞是同一件事，只是换了一层。

### 2.2 洞四（同形）：`question` 是复述字段，却只做非空检查

探针标出：`question`（**34 条全部**都带）**只有「非空」检查、不扫引用**。它是**字面意义上的复述字段**
——它复述基线的提问——W80 立的推理「复述处可以随便编」对它**最直接适用**，
而 W80 的清单**唯独漏了它**。

> ADR 0058 的遗留 3 曾把 `question` 明确列在「`CITATION_FIELDS` 之外」，并写成
> 「清单是单一来源，**改一处即可**」。本轮实测这句话把**维护者的记性**当成了机制——
> 已按 §56.2 在 ADR 0058 该条追加**勘误段**（原文保留）。

### 2.3 两个洞是同一个形状的不同层

W80 的结论是「断言的**存在** ≠ 断言的**覆盖**」。本轮是它的下一层：
**覆盖对「新出现的东西」永远需要一条闭世界断言来兜**——清单挡得住清单上的字段被编造，
挡不住清单外的字段被新增。**只要还有「清单之外的字段不受检」这个状态，W80 修掉的洞就还在。**

## 三、裁定

见 [ADR 0059](../adr/0059-ledger-field-closure.md) 决策 1–5，要点：

1. `LEDGER_FIELDS`（**逐字写死**，十个字段）与 `STRUCTURAL_FIELDS`（`id`／`disposition`）导出；
   派生写法会让「并集 == `LEDGER_FIELDS`」成为**恒真式**（W80 §九结论 5：只钉关系不够）。
2. **闭世界断言只看字段名、不看值**：`PD-01: unknown field rationale — a field the checker does not
   know can carry §NNN unchecked`。⇒ 注入 MUST 含一次**值干净**的未知字段。
3. `question` 进 `CITATION_FIELDS`（实测 0 命中 ⇒ 不引入红；方向为安全侧）。
4. 三类 MUST **两两不相交**（一个字段不能有两个主人）。
5. 并集 == `LEDGER_FIELDS`，**两个方向**都具名开火。

## 四、实现

| 文件 | 改动 |
| --- | --- |
| `scripts/contracts/pending-decisions.mjs` | +`STRUCTURAL_FIELDS`、+`LEDGER_FIELDS`（均导出）；`CITATION_FIELDS` 增 `question`；`shapeErrors()` 增「三类两两不相交」与「并集 == `LEDGER_FIELDS`（双向）」；`validatePendingDecisions` 增**闭世界**断言（`Object.keys(entry)` ⊆ `LEDGER_FIELDS`，报条目 + 字段名） |
| `tests/registry.test.mjs` | 新增 2 用例（未知字段注入含**值干净**一次 + `question` 引用注入；逐字钉 `LEDGER_FIELDS` + 三类划分 + **非空洞**）；import 增 `STRUCTURAL_FIELDS`／`LEDGER_FIELDS`；**28 → 30** |

**接线零改动**：断言并入既有 Node 测试文件 ⇒ `xtask/src/lib.rs::engineering_checks` 的逐个显式列举**不需改**。
**台账数据一字未改**——实测当前字段集**恰好**等于 `LEDGER_FIELDS`、`question` **0 命中** `§`
⇒ 基线**保持全绿**。

## 五、非空洞注入（F0 基线 + F1–F4 数据级 + F5–F8 表级）

以 `.w81_inject.mjs` 驱动（**锚点不唯一即拒绝动手**），每次要求**红色 + 具名**：

| 注入 | 期望 |
| --- | --- |
| **F0** 基线（未注入） | 对账 `errors = []`、`shapeErrors() = []` ⇒ **先证「本来就绿」** |
| **F1** 未知字段 `rationale: "见 §99.9"` | `PD-01: unknown field rationale …` ⇒ **指名条目 + 字段** |
| **F2** 未知字段 `rationale: "见 §40.4"`（**值干净**） | **同样失败** ⇒ 挡的是**字段**，不是它的内容 |
| **F3** `question` 含 `§99.9` | `PD-01: question references §99.9 …` ⇒ 复述字段已被扫 |
| **F4** `question` 含合法 `§40.4` | **不红** ⇒ 反向注入：不是「`question` 一提 `§` 就报」 |
| **F5** 把 `question` 移出 `CITATION_FIELDS`（模块副本） | `field question is allowed by LEDGER_FIELDS but no check category claims it` |
| **F6** 把 `note` 移出 `LEDGER_FIELDS`（模块副本） | `field note is classified but not listed in LEDGER_FIELDS` |
| **F7** 把 `id` 也塞进 `CITATION_FIELDS`（模块副本） | `field id is claimed by two check categories (citation, structural)` |
| **F8** 往 `LEDGER_FIELDS` 掺幽灵字段 `ghost`（模块副本） | `field ghost is allowed by LEDGER_FIELDS but no check category claims it` |

**F2 是这轮最有价值的一项**：F1 只证明「未知字段里的坏引用会被抓」——若实现在此收手，判据测的就只是
「未知字段的值坏不坏」。**F2 证明挡的是「字段本身不在声明集里」**，与它的值无关。
**F4 是「不该红时不红」**：只有 F1–F3 全红也可能来自**过紧**的实现（把任何 `question` 里的 `§` 都报）。

> 表级注入用**临时模块副本**（`pending-decisions.mjs` 无 import ⇒ 副本可独立 import）；
> 每次动手前先断言锚点**恰好出现一次**，出现 0 次或 2 次即**拒绝动手**（W79 的同一纪律）。
> 注意 `  'question',` 在两个清单里**各出现一次**——锚点必须取三行块才唯一。

## 六、验证

### 6.1 定向测试

```
node --test tests/registry.test.mjs            →  30/30 pass   （28 → 30）
node --test tests/contracts/schema.test.mjs    →  17/17 pass
node .w81_inject.mjs                           →  F0 + F1–F8 共 9 项，失败 0
```

### 6.2 门控

```
cargo xtask check --profile full   →  EXIT=0   （57s；热产物，首次即绿）
```

实测 **91 个测试目标（73 `Running` + 18 `Doc-tests`）、860 passed / 0 failed**。
口径以 `grep -c '^test result:' <门控日志>` 定准（**每个目标恰好一行，没有自由度**）；
`^     Running ` 74 行 = **1 包装行**（cargo 自己打印的 `Running \`…nai-xtask.exe check --profile full\``，
**不是测试目标**）+ 73 真实目标。本轮**零 Rust 改动**，`passed` 与 W71–W80 逐项相同 ⇒「数字不变」
正是「未误改别处」的旁证。日志里可见本轮两条用例在门控内执行：
`ok 18 - an entry field the checker does not know cannot hide from the scan`、
`ok 19 - the ledger field set is closed and every field has exactly one owner`
（Node 用例**不**计入 91/860，那组数只来自 `cargo test`）。

### 6.3 两条 validate

```
openspec validate --all --strict              →  43 passed / 0 failed
openspec validate --all --strict --archived   →  79 passed / 0 failed   （78 → 79）
```

另对**活动变更**单独跑 `openspec validate w81-ledger-field-closure --strict` ⇒ `valid`
（新增 Requirement 的正文实测 **275** 字符，未超 500——该限制只在 `specs/` 里报，归档前看不出来，W56 踩过）。

## 七、CI 复核

（待回填）

## 八、遗留

1. **仍不判语义**：脚本只证「引用的章节号存在」，**不证该章节回答了该问题**（沿用 ADR 0054 决策 2）。
2. **`note` 的处置归属不校验**：一条 `note` 可挂在任何处置上（实测 5 条里 4 条是注解），本轮不要求相称。
3. **闭世界只对「条目内部」成立**：`entries` 之外的顶层键（`schema_version`／`baseline`／`entries`）
   由各自的具名检查判定，**不**进 `LEDGER_FIELDS`；新增顶层键同样 MUST 被显式处理（ADR 0059 遗留 3）。
4. **`question` 从此必须「引用可解析」**：将来若出现合法的 `§` 指代（如指代非当前基线的版本章节），
   MUST 落在一个真实章节号上，或改由别的字段承载。
5. **§56.2 归档边界仍未写死**（W78 §八第 8 / W79 §八第 6 / W80 §八第 4 未变）：
   「上一轮已发布的归档」与「本轮刚归档、尚未入库」是否同一处置，仍需一次升基线轮次写死。
6. **`CAPABILITY_PACK_IDS` 仍是手写数组**（W78–W80 遗留未变；ADR 0056 遗留 4 已给理由）。
7. harness 轨道 **H18 与 B07** 仍 `pending`；W76 台账的 5 条 `decidable_in_repo`
   （`PD-09/22/24/31/34`）仍只登记了推荐——**推荐的落地仍未发生**。
8. `cargo xtask schema --check` 本机仍跑不了（宿主对 node 派生任何子进程的 `EBUSY`）。
   本轮未改任何 schema 与生成器输入 ⇒ 零漂移由结构性理由 + CI 独立步骤兜底，**未写成「已通过」**。

## 九、方法论教训

1. **「按清单扫」的守卫，盲区恰是清单的补集。** 清单挡得住「清单上的字段被编造」，
   挡不住**清单外的字段被新增**。凡「遍历一份清单」的守卫，MUST 同时问：**这份清单的补集，谁来管？**
   ——若答不上来，就必须补一条**闭世界**断言。
2. **「断言存在」与「断言覆盖」之后是第三层：「覆盖的边界谁保证」。** W80 修掉了「清单内的漏扫」，
   本轮修掉「清单外的漏扫」。**每修一层，边界只是移到了新的地方**——所以要问的是
   「这层边界是什么形状」，而不是「还有没有洞」。
3. **闭世界断言 MUST 只看名字、不看值。** 若写成「未知字段**且**其值含 `§NNN`」才报，
   判据的输入（值）就**不等于**被断言的对象（字段集）。**F2 就是为这条而设**——
   它是唯一能区分「挡字段」与「挡坏值」的注入。
4. **「不该红时不红」与「证明洞是真的」要成对。** F4（合法 `§` 不红）证明边界没被收得过紧；
   F0（基线绿）证明本轮**不引入红**（台账数据一字未改）。
5. **一个字段可以「既被扫、又有专属检查」。** 分类判的是「`§NNN` 由谁负责」，
   不是「这个字段还有没有别的检查」——把 `question` 归入 `CITATION_FIELDS` 的同时，
   它的「非空」检查照旧。
6. **「改一处即可」是一句**承诺**，不是一条**机制**。** ADR 0058 把维护者的记性当成了保证；
   本轮把它换成断言（闭世界 + 具名拒绝）。**凡推理的终点是「记得去改」，终点就还没到。**
