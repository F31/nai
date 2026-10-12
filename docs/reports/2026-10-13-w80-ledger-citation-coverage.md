# W80 · 待裁定台账的引用完整性（扫描域必须覆盖所有承载 `§NNN` 的字段）

- 日期：2026-10-13
- 基线：V3.10（`docs/直播数字人 + 智能体系统技术方案 V3.10.md`）
- 裁定：[ADR 0058](../adr/0058-ledger-citation-coverage.md)
- 变更：`openspec/changes/archive/2026-10-13-w80-ledger-citation-coverage/`

## 一、判断与范围

W76 立的待裁定台账有一条断言：**条目里引用的每个 `§NNN` MUST 真实存在于当前基线**。
本轮核实它的**覆盖面**——断言是对的，但它有**两个同形的洞**，病因都是「一份会漂移的手写清单」。

**范围**：只改 `scripts/contracts/pending-decisions.mjs` 与 `tests/registry.test.mjs`；
**不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边**。

## 二、审实

### 2.1 洞一（字段级）：扫描域是手写的三个字段

```js
const refs = [
  ...sectionRefs(entry.basis),
  ...sectionRefs(entry.introduced_in),
  ...(Array.isArray(entry.also_in) ? entry.also_in.flatMap(sectionRefs) : []),
];
```

`REQUIRED_FIELD` 里**另外三个同样承载 `§NNN` 的字段**（`recommendation`／`closure`／`note`）
一条都没扫。**探针实测（只用公开 API，不改被测文件）**：

```
[已扫] basis           引用数=4
[已扫] introduced_in   引用数=34
[已扫] also_in         引用数=28
[未扫] recommendation  引用数=4      ← §40.4 / §5.5 / §14.1 / §54.2
[未扫] closure         引用数=9      ← §52.1 / §38.8 / §38.4 / §10.2 / §8.2 / §46.2 …
[未扫] note            引用数=4      ← §46.3 / §43.4 / §47.2 / §45.3
```

**注入实测**：把 `PD-24.recommendation` 里 `§40.4` 改成不存在的 `§99.9`
⇒ `validatePendingDecisions` 返回**空错误**。**17 条引用无人核。**

> 既有测试里有一句注释预见了这个形状——「`basis` 之外的引用同样受检（`introduced_in` / `also_in`），
> 否则**复述处可以随便编**」——只是它把「之外」当成了那两个字段，**漏了真正承载 17 条引用的三个**。

### 2.2 洞二（表级）：校验器自身的两张表可以分家

```js
const field = REQUIRED_FIELD.get(entry.disposition);
if (field) { … }        // ← 该处置漏在 REQUIRED_FIELD 里时 field 是 undefined ⇒ 整段跳过
```

若某处置进了 `DISPOSITIONS` 却漏在 `REQUIRED_FIELD` 里，它的条目就**没有必填字段**。
**注入实测**：在临时副本里删掉 `['decidable_in_repo', 'recommendation']`，再删掉任一条
`decidable_in_repo` 的 `recommendation` ⇒ **不再有** `requires a non-empty recommendation`。
**校验对它沉默。**

### 2.3 两个洞是同一个病因

**一份会漂移的手写清单**：洞一的清单写死在循环里，洞二的清单根本没写。
所以修法不是「再补三个 `sectionRefs(...)`」，而是**把清单变成单一来源 + 让清单本身被断言**。

## 三、裁定

见 [ADR 0058](../adr/0058-ledger-citation-coverage.md) 决策 1–4，要点：

1. 扫描域取**导出的**单一清单 `CITATION_FIELDS`（六字段）+ `ID_LIST_FIELDS`（`precondition`）；
   引用扫描**遍历清单**，不再手写。
2. 错误措辞改为**条目 + 字段 + 章节号**三件齐（同一个坏章节号可能在多个字段里，只说条目无法定位）。
3. `shapeErrors()`：`REQUIRED_FIELD` 键集 == `DISPOSITIONS`、两清单不相交、每个必填字段必有覆盖；
   并入 `validatePendingDecisions` 的返回值。
4. 判据 MUST 在**每个**字段上开火、且钉住清单内容（**只钉「关系」不够**——三者一起被拿掉时，
   字段集同步变窄、关系断言仍全绿）。

**`note` 为什么必须在清单里**：它不是 `out_of_scope` 的专属字段——实测 5 条 `note` 里有 **4 条**
挂在**别的**处置上（注解）。**扫描按「字段是否存在」触发，不按处置触发**，否则注解型的 `note` 会被漏掉。

## 四、实现

| 文件 | 改动 |
| --- | --- |
| `scripts/contracts/pending-decisions.mjs` | +`CITATION_FIELDS`、+`ID_LIST_FIELDS`、+`shapeErrors()`（导出）；引用扫描改为遍历清单；错误措辞加字段名；`validatePendingDecisions` 无条件并入 `shapeErrors()` |
| `tests/registry.test.mjs` | 扩展「引用必须可解析」用例（+3 字段级注入）；新增 1 用例（表级一致性 + 逐字钉清单）；**27 → 28** |

**接线零改动**：断言并入既有 Node 测试文件 ⇒ `xtask/src/lib.rs::engineering_checks` 的逐个显式列举**不需改**。
**台账数据一字未改**——本轮是「让既有 17 条引用**开始被核**」，实测 17 条**全部已可解析** ⇒ 基线**保持全绿**。

## 五、非空洞注入（F0 基线 + F1–F3 字段级 + F4–F5 表级）

以 `.w80_inject.mjs` 驱动（**锚点不唯一即拒绝动手**），每次要求**红色 + 具名**：

| 注入 | 期望 |
| --- | --- |
| **F0** 基线（未注入） | 对账 `errors = []`、`shapeErrors() = []` ⇒ **先证「本来就绿」** |
| **F1** `recommendation` 里 `§99.9` | `PD-09: recommendation references §99.9 …` ⇒ **指名字段** |
| **F2** `closure` 里 `§99.9` | `PD-01: closure references §99.9 …` |
| **F3** `note` 里 `§99.9` | `PD-20: note references §99.9 …` |
| **F4** 把 `CITATION_FIELDS` 换回旧三字段（模块副本） | `required field note for out_of_scope is covered by no check` ⇒ **覆盖断言会因清单变窄而开火** |
| **F5** 删掉 `['decidable_in_repo','recommendation']`（模块副本） | `disposition decidable_in_repo has no required field` |
| **F5b** F5 前提下删掉某条的 `recommendation` | `requires a non-empty recommendation` **不出现** ⇒ **证明洞二真实存在**（不只是理论） |

F1–F3 覆盖此前未扫的**每一个**字段（**每条来源 MUST 有独有覆盖**）；F4/F5 是**表级**注入——
把校验器自身的清单改坏，验证「不是装饰」。**F5b 单独证明洞是真的**：表坏时，那条条目的
必填字段确实静默消失了。

> 表级注入用**临时模块副本**（`pending-decisions.mjs` 无 import ⇒ 副本可独立 import）；
> 每次动手前先断言锚点**恰好出现一次**，出现 0 次或 2 次即**拒绝动手**（W79 的同一纪律）。

## 六、验证

### 6.1 定向测试

```
node --test tests/registry.test.mjs            →  28/28 pass   （27 → 28）
node --test tests/contracts/schema.test.mjs    →  17/17 pass
node .w80_inject.mjs                           →  F0 + F1–F5b 共 7 项，失败 0
```

### 6.2 门控

```
cargo xtask check --profile full   →  EXIT=0   （33s；首次即绿）
```

实测 **91 个测试目标（73 `Running` + 18 `Doc-tests`）、860 passed / 0 failed**。
口径以 `grep -c '^test result:' <门控日志>` 定准（**每个目标恰好一行，没有自由度**）；
`^     Running ` 74 行 = **1 包装行**（cargo 自己打印的 `Running \`…nai-xtask.exe check --profile full\``，
**不是测试目标**）+ 73 真实目标。本轮**零 Rust 改动**，`passed` 与 W71–W79 逐项相同 ⇒「数字不变」
正是「未误改别处」的旁证。日志里可见本轮两条用例在门控内执行：
`ok 16 - a pending decision cannot cite a section the baseline does not have`、
`ok 17 - the ledger checker cannot lose coverage through its own tables`（Node 用例**不**计入 91/860，
那组数只来自 `cargo test`）。

### 6.3 两条 validate

```
openspec validate --all --strict              →  43 passed / 0 failed
openspec validate --all --strict --archived   →  78 passed / 0 failed   （77 → 78）
```

另对**活动变更**单独跑 `openspec validate w80-ledger-citation-coverage --strict` ⇒ `valid`
（同时验证新增 Requirement 的正文长度未超 500 字符——该限制只在 `specs/` 里报，归档前看不出来，W56 踩过）。

## 七、CI 复核

**run `38319692372`（`head_sha = e5f50a44…`，与本轮推送 SHA 一致）= `success`**（run_number **90**，双平台绿）。

观测方式沿用既有纪律：**单次低频查询**（**不轮询**），每次带**唯一查询参数**破坏代理缓存、
`Cache-Control: no-cache`，并**回读 `head_sha` 与本次推送比对**；`curl` 带 `--tlsv1.2 --tls-max 1.2`
（schannel 与 TLS 1.3 握手失败，W77 实测）。本轮共 3 次查询：`queued`（推送后）→ `in_progress`（+10m）
→ `success`（+15m 读到，updated `23:10:55Z`）。同一列表查询顺带证实上一轮 run `38276175603`
（`head_sha = b4aca0b`，W79 回填）也是 `success`。

**按观测轮终止约定**：本条回填提交会触发一个新的 run，**该 run 不再回填**——否则无限递推。

## 八、遗留

1. **仍不判语义**：脚本只证「引用的章节号存在」，**不证该章节回答了该问题**（ADR 0054 决策 2 一致）。
2. **`note` 的处置归属不校验**：一条 `note` 可挂在任何处置上（实测如此），本轮不要求它与处置相称。
3. **`CITATION_FIELDS` 之外的字段**不承载 `§NNN`；将来若有新字段承载，MUST 一并进清单
   ——**清单是单一来源，改一处即可**（这正是本轮引入的纪律）。
4. **§56.2 归档边界仍未写死**（W78 §八第 8 / W79 §八第 6 未变）：「上一轮已发布的归档」与
   「本轮刚归档、尚未入库」是否同一处置，仍需一次升基线轮次写死。
5. `CAPABILITY_PACK_IDS` 仍是手写数组（W78/W79 遗留未变）。
6. harness 轨道 **H18 与 B07** 仍 `pending`；W76 台账的 5 条 `decidable_in_repo`
   （`PD-09/22/24/31/34`）仍只登记了推荐——**本轮让其中三条（PD-09/24/31）的 `recommendation`
   引用开始被核**，但推荐的**落地**仍未发生。
7. `cargo xtask schema --check` 本机仍跑不了（宿主对 node 派生任何子进程的 `EBUSY`）。
   本轮未改任何 schema 与生成器输入 ⇒ 零漂移由结构性理由 + CI 独立步骤兜底，**未写成「已通过」**。

## 九、方法论教训

1. **断言的「存在」不等于断言的「覆盖」。** W76 立了正确的一条断言，但它的域是手写的三个字段
   ——**漏掉三个字段里的 17 条引用，与没有这条断言是同一件事**。凡是「扫一份清单」的守卫，
   MUST 问：**这份清单有几处会写？**
2. **「复述处可以随便编」这条推理，适用面比当时想的更宽。** 既有注释用它对 `introduced_in`／
   `also_in` 立了断言，却没有把同样的推理走完——`recommendation`／`closure`／`note` 是同一类
   「复述处」。**一条推理走到一半，比不走更容易让人以为已经走完。**
3. **守卫自身的常量表也要有自己的守卫。** `REQUIRED_FIELD` 与 `DISPOSITIONS` 分家时，
   校验**沉默**——这是 W73 假契约 / W66「删字段照样绿」的同一族：**断言还在，覆盖没了，没有信号。**
4. **反向注入要有，但还要有「证明洞是真的」那一项。** F4/F5 证明「清单被改坏时会开火」；
   **F5b 证明「清单确实会坏」**（表坏时那条条目真的没有必填字段）。少 F5b，F5 只是在测一段
   没被证明有意义的分支。
5. **钉内容与钉关系缺一不可。** `shapeErrors()` 的关系断言（必填字段要有覆盖）在清单**整体变窄**时
   仍会全绿；只有**逐字钉**`CITATION_FIELDS` 才挡得住「三者一起被拿掉」。
