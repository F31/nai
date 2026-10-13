# W82 · 台账信封的闭世界（顶层键集 MUST 封闭且与条目字段集不相交）

- 日期：2026-10-13
- 基线：V3.10（`docs/直播数字人 + 智能体系统技术方案 V3.10.md`）
- 裁定：[ADR 0060](../adr/0060-ledger-envelope-closure.md)
- 变更：`openspec/changes/archive/2026-10-13-w82-ledger-envelope-closure/`

## 一、判断与范围

W81 给待裁定台账加了一条**闭世界**断言（条目里出现的每个键 MUST ∈ `LEDGER_FIELDS`）。这条**是对的**，
但它把**层级**当成了背景——台账有**两个**层级，而闭世界只在一个层级上成立。

本轮核实的就是**另一个层级**：**信封**（台账顶层）。三个已知信封键（`schema_version`／`baseline`／
`entries`）**各有**具名检查 ⇒ 这不是「检查漏写」，而是**键集不封闭**：
**已知键各有人管 ≠ 键集封闭，新增的第一个键就落进盲区。**

**范围**：只改 `scripts/contracts/pending-decisions.mjs` 与 `tests/registry.test.mjs`；
**不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边**。

## 二、审实

### 2.1 洞五（数据级）：顶层键没有任何「不认识就拒」的断言

**探针实测（只用公开 API，不改被测文件）**：

```
=== 注入 UB1：顶层未知字段 revision，内含 §99.9 ===
errors = []                                  ← 完全静默

=== 注入 UB2：未知字段 revision = "见 §40.4"（合法章节） ===
errors = []

=== 注入 UB3：顶层放一个「条目字段名」 question: "§99.9" ===
errors = []                                  ← 两个闭世界都不认它

=== 三个已知信封键的具名检查（改前） ===
schema_version=2 → ["schema_version must be 1"]
baseline=V3.99   → ["ledger declares baseline "V3.99" but the current baseline is V3.10"]
entries=[]       → ["entries must be a non-empty list"]
```

⇒ 顶层加一个字段，它的引用就永久不受检——与 W80／W81 修掉的是**同一件事**，只是换了一层。

### 2.2 UB3 是最尖锐的一条：它暴露了「层级」

把一个**条目字段名**（`question`）写到顶层时，**条目闭世界看不见它**（它在顶层），
而**信封闭世界还不存在** ⇒ **没有任何人管它**。这一条同时说明：**闭世界的完整性不只要求
每个层级封闭，还要求两个层级的字段名集不相交**。

### 2.3 三层是同一个形状

| 层 | 洞 | 挡得住 | 挡不住 |
| --- | --- | --- | --- |
| W80 修掉 | 清单**内**字段漏扫 | 清单内字段被编造 | —— |
| W81 修掉 | 清单**外**的条目字段 | —— | 条目的新增字段 |
| **W82 本轮** | **信封层的键集** | —— | **顶层的新增键** |

⇒ 「有断言」→「断言有覆盖」（W80）→「覆盖的边界谁保证」（W81）→
**「边界在几个层级上成立」**（本轮）。**只在一个层级封闭，等于把盲区搬到了上一层**——
而**上一层通常是「更少人看」的那一层**。

## 三、裁定

见 [ADR 0060](../adr/0060-ledger-envelope-closure.md) 决策 1–5，要点：

1. `TOP_LEVEL_FIELDS`（**逐字写死**，三个键）导出；派生写法会让闭世界成为**恒真式**。
2. **信封闭世界断言只看键名、不看值**：`unknown envelope field revision — a top-level field the
   checker does not know can carry §NNN unchecked`。⇒ 注入 MUST 含一次**值干净**的顶层未知键。
3. **信封字段必需**：`envelope field baseline is missing — a declared envelope field with no value
   is a field nobody reads` ⇒ 声明一个字段就必须在数据里有它（错加幽灵字段会**响得早**）。
4. **两个层级的字段名集 MUST 不相交**（`shapeErrors()`，具名）——UB3 的洞唯一的守卫。
5. **两层各自保留措辞**（一层报**条目 + 字段名**、一层报**键名**），MUST NOT 合并。

## 四、实现

| 文件 | 改动 |
| --- | --- |
| `scripts/contracts/pending-decisions.mjs` | +`TOP_LEVEL_FIELDS`（导出）；`shapeErrors()` 增「信封字段集 ∩ `LEDGER_FIELDS` == 空」；`validatePendingDecisions` 增**信封闭世界**（`Object.keys(ledger)` ⊆ `TOP_LEVEL_FIELDS`，报键名）+ **信封必需**（`Object.hasOwn`，报字段名） |
| `tests/registry.test.mjs` | 新增 2 用例（顶层未知键注入含**值干净**一次 + 条目字段名放顶层 + 信封字段逐个删除；逐字钉 `TOP_LEVEL_FIELDS` + 两层不相交 + **非空洞**）；import 增 `TOP_LEVEL_FIELDS`；**30 → 32** |

**接线零改动**：断言并入既有 Node 测试文件 ⇒ `xtask/src/lib.rs::engineering_checks` 的逐个显式列举**不需改**。
**台账数据一字未改**——实测顶层键**恰好**等于 `TOP_LEVEL_FIELDS`（三个）⇒ 基线**保持全绿**。

## 五、非空洞注入（F0 基线 + F1–F4 数据级 + F5–F9 表级）

以 `.w82_inject.mjs` 驱动（**锚点不唯一即拒绝动手**），每次要求**红色 + 具名**：

| 注入 | 期望 |
| --- | --- |
| **F0** 基线（未注入） | `errors = []`、`shapeErrors() = []` ⇒ **先证「本来就绿」** |
| **F1** 顶层未知键 `revision: "见 §99.9"` | `unknown envelope field revision …` ⇒ **具名**，且**只有这一条** |
| **F2** 顶层未知键 `revision: "见 §40.4"`（**值干净**） | **同样失败** ⇒ 挡的是**键**，不是它的值 |
| **F3** 条目字段名 `question: "§99.9"` 放到顶层 | `unknown envelope field question …` ⇒ 两闭世界之间的缝已被填 |
| **F4** 逐个删除信封字段（3 个子例） | 各报 `envelope field <name> is missing` ⇒ 声明即必需 |
| **F5** 表级：`baseline` 移出 `TOP_LEVEL_FIELDS`（模块副本） | `unknown envelope field baseline …` |
| **F6** 表级：声明集掺幽灵字段 `ghost` | `envelope field ghost is missing` ⇒ **声明即必须在数据里有** |
| **F7** 表级：把条目字段 `note` 塞进 `TOP_LEVEL_FIELDS` | `field note is declared as both an envelope field and an entry field` ⇒ 不相交断言开火 |
| **F8** 表级（反向）：`entries` 移出声明集 | 报**未知键** `entries`（**不**报必需缺失） |
| **F9** 表级（反向）：声明集**顺序打乱** | **不红** ⇒ 顺序不是语义 |

**F2 是这轮最有价值的一项**：F1 只证明「顶层未知键里的坏引用会被抓」——若实现在此收手，
判据测的就只是「未知键的值坏不坏」。**F2 证明挡的是「键本身不在声明集里」**，与它的值无关。
**F8／F9 是「不该红时不红」**：只有 F1–F7 全红也可能来自**过紧**的实现（如把任何未声明的键
都报两次、或对顺序敏感）。**F6 证明「声明集被写坏」是响得早的**，而不是静静地什么都不检查。

> 表级注入用**临时模块副本**（`pending-decisions.mjs` 无 import ⇒ 副本可独立 import）；
> 每次动手前先断言锚点**恰好出现一次**，出现 0 次或 2 次即**拒绝动手**（W79 的同一纪律）。

## 六、验证

### 6.1 定向测试

```
node --test tests/registry.test.mjs            →  32/32 pass   （30 → 32）
node --test tests/contracts/schema.test.mjs    →  17/17 pass
node .w82_inject.mjs                           →  F0 + F1–F9 共 10 项，失败 0
```

### 6.2 门控

```
cargo xtask check --profile full   →  EXIT=0   （首次即绿；热产物）
```

实测 **91 个测试目标（73 `Running` + 18 `Doc-tests`）、860 passed / 0 failed**。
口径以 `grep -c '^test result:' <门控日志>` 定准（**每个目标恰好一行，没有自由度**）；
`^     Running ` 74 行 = **1 包装行**（cargo 自己打印的 `Running \`…nai-xtask.exe check --profile full\``，
**不是测试目标**）+ 73 真实目标。本轮**零 Rust 改动**，`passed` 与 W71–W81 逐项相同 ⇒「数字不变」
正是「未误改别处」的旁证。日志里可见本轮两条用例在门控内执行：
`ok 20 - a top-level field the checker does not know cannot hide from the scan`、
`ok 21 - the envelope field set is closed and disjoint from the entry field set`
（Node 用例**不**计入 91/860，那组数只来自 `cargo test`）。

### 6.3 两条 validate

```
openspec validate --all --strict              →  43 passed / 0 failed
openspec validate --all --strict --archived   →  80 passed / 0 failed   （79 → 80）
```

另对**活动变更**单独跑 `openspec validate w82-ledger-envelope-closure --strict` ⇒ `valid`
（MODIFIED Requirement 的正文实测 **395** 字符，未超 500——该限制只在 `specs/` 里报，归档前看不出来）。

> **一处流程自纠（如实登记）**：本轮归档时 `tasks.md` 的九项**尚未勾选**，
> `validate --archived` 当场以 `✗ 9 incomplete tasks (0/9 completed)` **失败**（`archive --yes` 只给
> 非阻塞警告）。这正是 `KNOWLEDGE.md §6.3` 已登记的纪律（**归档前勾 tasks**），本轮**执行晚了**。
> 处置：九项均已真实交付 ⇒ 就地勾选（该变更**本轮创建、本轮归档、同在一次未提交工作区内**，
> 按 W78 的判定可作就地订正），并**顺带订正**第 6 项与实现不符的一处表述（写作 F5–F7，
> 实际为 F5–F9 含两条反向注入）。**订正后两条 validate 全绿（43 / 80）。**


## 七、CI 复核

**run `38328899767`（`head_sha = 33a9283340…`，与本轮推送 SHA 一致）= `success`**（run_number **94**，双平台绿）。

观测方式沿用既有纪律：**单次低频查询**（**不轮询**），每次带**唯一查询参数**破坏代理缓存、
`Cache-Control: no-cache`，并**回读 `head_sha` 与本次推送比对**；`curl` 带 `--tlsv1.2 --tls-max 1.2`
（schannel 与 TLS 1.3 握手失败，W77 实测）。本轮共 3 次查询：`in_progress`（推送后）→ `in_progress`（+8m）
→ `success`（+15m 读到，updated `00:57:22Z`）。同一列表查询顺带证实上一轮 run `38323669462`
（`head_sha = 47377f9`，W81 回填）也是 `success`。

**按观测轮终止约定**：本条回填提交会触发一个新的 run，**该 run 不再回填**——否则无限递推。

## 八、遗留

1. **仍不判语义**：脚本只证「引用的章节号存在」，**不证该章节回答了该问题**（沿用 ADR 0054 决策 2）。
2. **`note` 的处置归属不校验**（沿用 ADR 0058 遗留 2 / 0059 遗留 2）。
3. **信封只覆盖「顶层键集」**：三个信封字段的**值**由各自的具名检查判定；本轮的闭世界只保证
   **它们的名字被认识、且必须出现**。⇒ 新增信封字段 MUST **同时**补一条值检查（ADR 0060 遗留 3）。
4. **`question` 从此必须「引用可解析」**（沿用 ADR 0059 遗留 4）。
5. **§56.2 归档边界仍未写死**（W78 §八第 8 / W79 §八第 6 / W80 §八第 4 / W81 §八第 5 未变）：
   「上一轮已发布的归档」与「本轮刚归档、尚未入库」是否同一处置，仍需一次升基线轮次写死。
6. **`CAPABILITY_PACK_IDS` 仍是手写数组**（W78–W81 遗留未变；ADR 0056 遗留 4 已给理由）。
7. harness 轨道 **H18 与 B07** 仍 `pending`；W76 台账的 5 条 `decidable_in_repo`
   （`PD-09/22/24/31/34`）仍只登记了推荐——**推荐的落地仍未发生**。
8. `cargo xtask schema --check` 本机仍跑不了（宿主对 node 派生任何子进程的 `EBUSY`）。
   本轮未改任何 schema 与生成器输入 ⇒ 零漂移由结构性理由 + CI 独立步骤兜底，**未写成「已通过」**。

## 九、方法论教训

1. **闭世界 MUST 在「每个层级」上成立。** 只在一个层级封闭，等于把盲区搬到上一层——而上一层
   通常是**更少人看**的那一层。凡「某个集合必须封闭」的断言，MUST 先问：**它有几种层级？**
2. **「已知项各有人管」≠「集合封闭」。** 三个信封键各有具名检查，看上去很完整；而**第四个键**
   没有任何人管。**完整性不是「每一项都被处理」，而是「每一项之外的东西被拒绝」。**
3. **相邻层级的字段名集 MUST 不相交。** 把一个条目字段名写到顶层时，**两个闭世界同时在缝里放行**
   ——这不是「两个检查都通过」，而是「两个检查都不覆盖」。
4. **闭世界断言 MUST 只看名字、不看值**（W81 结论的第二次适用）：若写成「未知键**且**值含 `§NNN`」
   才报，判据的输入（值）就**不等于**被断言的对象（键集）。**F2 就是为这条而设。**
5. **「声明即必需」把常量表绑到数据上。** F6 证明：往 `TOP_LEVEL_FIELDS` 里错加一个幽灵字段会
   **立刻红**（每个真实台账都缺它），而不是「静静地什么都不检查」——**鸣笛比沉默好，早鸣笛更好。**
6. **「不该红时不红」与「证明洞是真的」要成对**（W79–W82）：F9（顺序不敏感）、F8（移出声明集只报
   未知键、不误报必需缺失）、F0（基线绿）——都在证明**边界没收过紧 / 本轮不引入红**。
7. **流程纪律「知道」≠「执行」**（本轮自纠）：`归档前勾 tasks` 是 `KNOWLEDGE.md §6.3` 里**早已登记**
   的一条，本轮仍在归档后才发现（`validate --archived` 报 `9 incomplete tasks`）。
   ⇒ **一条纪律记在文档里不等于它在流程里被执行**——它与 W81「『改一处即可』是承诺不是机制」同型：
   需要把「知道」换成一个**会在错的时候响的**断言（这里就是 `validate --archived` 本身，
   所以可靠的是**在归档后立刻跑它**，而不是「记得先勾」）。
