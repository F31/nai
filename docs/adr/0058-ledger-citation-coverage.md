# ADR 0058：待裁定台账的引用完整性——扫描域由单一清单派生

- 状态：已接受
- 日期：2026-10-13
- 依据：ADR 0054（待裁定台账）、ADR 0050（基线 ↔ 在册对账）、
  W72（`AJV_SOURCES` 是「有哪些 schema」的单一清单）、W73（假契约：每句「SHALL 被校验」都要能指出执行体）、
  W66（照抄表的常量必须另有把表逐项写死的断言）、W79（错误的措辞会把定位引向错误的方向）
- 影响：`scripts/contracts/pending-decisions.mjs`、`tests/registry.test.mjs`；
  **不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边**

## 背景

W76 的台账校验器有一条断言：**条目里引用的每个 `§NNN` MUST 真实存在于当前基线**。
断言本身是对的，**它的覆盖有两个同形的洞**。

### 洞一（字段级）：扫描域是手写的三个字段

```js
const refs = [
  ...sectionRefs(entry.basis),
  ...sectionRefs(entry.introduced_in),
  ...(Array.isArray(entry.also_in) ? entry.also_in.flatMap(sectionRefs) : []),
];
```

`REQUIRED_FIELD` 里**另外三个同样承载 `§NNN` 的字段**——`recommendation`、`closure`、`note`
——一条都没扫。**实测（W80 探针，只用公开 API）**：

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

既有测试里甚至有一句注释预见了这个形状——「`basis` 之外的引用同样受检，否则**复述处可以随便编**」
——只是它把「之外」当成了 `introduced_in`／`also_in`，**漏了真正承载 17 条引用的那三个字段**。

### 洞二（表级）：校验器自身的两张表可以分家

```js
if (!DISPOSITIONS.includes(entry.disposition)) { … }
const field = REQUIRED_FIELD.get(entry.disposition);
if (field) { … }        // ← 该处置漏在 REQUIRED_FIELD 里时，field 是 undefined ⇒ 整段跳过
```

若某处置进了 `DISPOSITIONS` 却漏在 `REQUIRED_FIELD` 里，它的条目就**没有必填字段**。
**注入实测（W80）**：在临时副本里删掉 `['decidable_in_repo', 'recommendation']`，再把任一条
`decidable_in_repo` 的 `recommendation` 删掉 ⇒ **不再有** `requires a non-empty recommendation` 错误。
**校验对它沉默。**

> 两个洞的病因相同：**一份会漂移的手写清单**。洞一的清单写死在循环里；洞二的清单根本没写。

## 决策

### 决策 1：扫描域取一份**导出的**单一清单

```js
export const CITATION_FIELDS = ['basis', 'introduced_in', 'also_in',
                                'recommendation', 'closure', 'note'];
export const ID_LIST_FIELDS = ['precondition'];   // 承载 id，不承载 §NNN
```

引用扫描**遍历 `CITATION_FIELDS`**，不再手写 `sectionRefs(...)` 调用。导出它有两个理由：
①判据可以直接断言它**逐字**含哪些字段（钉内容，不只是关系）；②它是「哪些字段会被扫」的
**单一回答**——与 `AJV_SOURCES` 对「有哪些 schema」是同一形状（W72）。

**`note` 必须在里面**：`note` 不是 `out_of_scope` 的专属字段——实测 5 条 `note` 里有 4 条挂在
**别的**处置上（PD-23／PD-26／PD-27／PD-30 是注解）。⇒ **扫描按「字段是否存在」触发，
不按「处置是不是 out_of_scope」触发**——后者会漏掉注解型的 `note`。

### 决策 2：错误具名到字段

原措辞 `${id}: references §${ref}` 在同一个坏章节号出现在多个字段时无法定位。
改为**条目 + 字段 + 章节号**三件齐：`PD-24: recommendation references §99.9, which does not exist
in the baseline`。这不是「更精确」而已——W79 已立过同型裁定：**错误的措辞会把定位引向错误的方向**。

### 决策 3：校验器自身的常量表 MUST 互相一致

新 `shapeErrors()`（导出，并被 `validatePendingDecisions` **无条件**并入返回值）：

1. `REQUIRED_FIELD` 的键集 MUST **等于** `DISPOSITIONS`（双向：少了会「无必填字段」，
   多了会「为一个不存在的处置定义字段」）；
2. `CITATION_FIELDS ∩ ID_LIST_FIELDS` MUST 为空；
3. 每个处置的必填字段 MUST ∈ `CITATION_FIELDS ∪ ID_LIST_FIELDS`
   ⇒ **新增处置的字段不可能在两个检查之间落空**。

**这不是「自检的装饰」**：它挡的不是「常量表写错」（一次性错误），而是**未来某次合法编辑**
（加一个处置）导致的**静默失去覆盖**——W73 假契约、W66「删字段照样绿」的同一族形状：
**断言还在，覆盖没了，且没有任何信号。**

### 决策 4：判据 MUST 在**每个**字段上开火，并钉住清单内容

- 对 `recommendation`／`closure`／`note` **各注入一次**，且要求错误**指名该字段**；
- 表级两条**真的把清单改坏**来验证：把 `CITATION_FIELDS` 换回旧的三字段，`shapeErrors()`
  必须**指名** `note` 失去覆盖；删掉一个处置的必填字段项，必须**点名**该处置。
- 直接钉内容：`CITATION_FIELDS` MUST 逐字含那三个字段。**只钉「关系」不够**——
  三者一起被拿掉时，字段集同步变窄、关系断言仍会全绿。

## 后果

- **台账数据一字未改**：本轮是「让既有 17 条引用**开始被核**」，不是改它们。
  实测 17 条**全部已可解析** ⇒ 基线**保持全绿**（这正是「本轮不引入红」的判据）。
- 新增 4 类具名失败：字段级坏引用（含字段名）、处置无必填字段、必填字段无覆盖、字段同时属两域。
- **不改基线正文、不改任何 Rust、不新增依赖边**（`pending-decisions.mjs` 无 import，
  注入验证用的临时副本因此可独立 import）。
- **不与 `baseline.mjs` 的引用检查合并**：那边的域是 §48.5／§51.3／§57.3 的**表格**，
  这边是台账的**字段**；两者共享的只有 `sectionRefs` 这个正则本身（已复用）。

## 实施

```
node --test tests/registry.test.mjs                 # 27 → 28
node .w80_inject.mjs                                # F0 + F1–F3（字段级）+ F4–F5（表级）
cargo xtask check --profile full
```

## 遗留

1. **仍不判语义**：脚本只证「引用的章节号存在」，**不证该章节回答了该问题**（与 ADR 0054 决策 2 一致）。
2. **`note` 的处置归属不校验**：一条 `note` 可以挂在任何处置上（实测如此），本轮不要求它与处置相称。
3. **`CITATION_FIELDS` 之外的字段**（`question`／`id`／`disposition` 等）不承载 `§NNN`；
   若将来某个新字段承载了，它 MUST 一并进 `CITATION_FIELDS`——**清单是单一来源，改一处即可**。

   > **勘误段（W81 / ADR 0059 追加）**：上面把「清单是单一来源，改一处即可」当作**已经够用**，
   > 而这句话把**维护者的记性**当成了机制。W81 实测：**`CITATION_FIELDS` 是清单，而清单的补集是盲区**
   > ——给某条加一个未知字段 `rationale: "见 §99.9"`，校验返回**空错误**。⇒ 本条判断被**收紧**：
   > 字段集 MUST 是**闭世界**的（`LEDGER_FIELDS`，ADR 0059）——新字段**不**进清单会被**具名拒绝**；
   > 并且本条把 `question` 列为「不承载 `§NNN`」的那份**假设**，已被**换成断言**：
   > `question` 现在**在** `CITATION_FIELDS` 里（实测当时 0 命中，故不引入红）。原文保留，细节见 ADR 0059。
