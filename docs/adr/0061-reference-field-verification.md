# ADR 0061：引用字段的验证范围——触发条件 MUST 只由**字段存在**决定

- 状态：已接受
- 日期：2026-10-13
- 依据：ADR 0058（扫描域由单一清单派生；**决策 1 已写下触发规则，但只落到了 `§NNN` 一侧**）、
  ADR 0059（条目字段集的闭世界）、ADR 0060（信封的闭世界）、
  W80（「扫描按字段是否存在触发，MUST NOT 按处置触发」）、
  W73（假契约：每句「SHALL 被校验」都要能指出执行体）、
  W66（不合并不同维度）、W79（错误的措辞会把定位引向错误的方向）
- 影响：`scripts/contracts/pending-decisions.mjs`、`tests/registry.test.mjs`；
  **不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边**

## 背景

ADR 0058 决策 1 写下了台账引用完整性的一般规则：

> **扫描按「字段是否存在」触发，不按「处置是不是 out_of_scope」触发**——后者会漏掉注解型的 `note`。

同一条决策还引入了 `ID_LIST_FIELDS = ['precondition']`（承载登记 id 的字段），并声明它
「按 id 校验」。**但这条规则只落到了 `§NNN` 一侧**——id 一侧的校验是**按处置触发**的：

```js
if (entry.disposition === 'needs_precondition' && Array.isArray(entry.precondition)) {
  for (const item of entry.precondition) { /* 校验每个 id */ }
}
```

**实测（W83 探针 v2，用真实注册表，只用公开 API，不改被测文件）**：

```
F0 基线错误数 = 0

=== P1 PD-01 (needs_external) 加 precondition=['W99'] ===          （无新增错误）
=== P1b PD-01 (needs_external) 加 precondition=['H99'] ===         （无新增错误）
=== P2 PD-25 (answered_by_baseline) 加 precondition=['不是 id'] === （无新增错误）
=== P2b PD-09 (decidable_in_repo) 加 precondition=['W99'] ===      （无新增错误）

=== P3 对照 PD-09 (decidable_in_repo) 加 closure='§99.9' ===
  + PD-09: closure references §99.9, which does not exist in the baseline
```

**P3 是对照组**：同在一条 `decidable_in_repo` 条目上，`closure` 里的坏 `§` 被抓、`precondition`
里的坏 id 不被抓——**同一个洞的两半，只有一半被 W80 补上**。

⇒ 一个字段由谁解析它的引用，是**字段的属性**，与「这条数据当前是什么处置」无关。
把「谁负责」写成 `disposition === 'X' && …` 的合取，等于让「这个字段有主」变成**条件性的**：
换个处置，同一个字段就**没人管**——而 `ID_LIST_FIELDS` 存在的全部意义，就是声明「它有主」。

## 决策

### 决策 1：`REFERENCE_FIELDS` 逐字写死

```js
export const REFERENCE_FIELDS = ['basis', 'introduced_in', 'also_in', 'recommendation',
                                 'closure', 'note', 'question', 'precondition'];
```

与 `LEDGER_FIELDS`／`TOP_LEVEL_FIELDS` 同理：MUST 是**独立的一份**声明，MUST NOT 由
`[...CITATION_FIELDS, ...ID_LIST_FIELDS]` 派生——派生出的「声明集 == 两类别之并」是**恒真式**
（W80 §九结论 5：只钉「关系」不够，要逐字钉内容）。写死之后，`shapeErrors()` 的「双向相等」
才是一条**内容断言**。

### 决策 2：扫描循环由 `REFERENCE_FIELDS` 驱动，触发条件**只看字段存在**

条目循环里唯一的引用扫描循环，其触发条件里**不出现 `disposition`**。这不是风格问题：

- **「必填」**是**预设**断言（这条处置 SHALL 带那个字段）⇒ 按处置触发**正确**；
- **「引用可解析」**是**内容**断言（这个字段的每个引用 SHALL 解析得开）⇒ 按处置触发**错误**。

把两者混在一个 `if` 里，正是本轮修的那个洞（W66：不合并不同维度）。

### 决策 3：错误措辞「条目 + 字段 + 值」三件齐，字段名由 `${field}` 生成

```
PD-01: precondition W99 is not a work item named by the baseline
```

原实现里「precondition」是**写死的字面量**。改由 `${field}` 生成，措辞不变，但**将来再加一个
id 清单字段时自动正确**——写死的字面量在那一刻会**说谎**（W79：错误的措辞会把定位引向错误的方向）。

### 决策 4：只拒「不可解析的引用」，不拒「放错位置的引用」

`precondition` 出现在**非**专属处置上时，只要每个 id 都解析得开，本轮**不报错**。

**方向**：假阳性与假阴性不对称 ⇒ 永远选**安全**方向。一个**可解析**的 `precondition` 描述了一个
真实前置，它奇怪但不虚假；拒绝它会把合法数据判红（**假阳性**）。本轮真正要挡的是「凭空编一个
不存在的 id」。⇒ 判据的输入 MUST 等于被断言的对象：这里断言的是「引用可解析」，不是「字段用对了处置」。

### 决策 5：`shapeErrors()` 的**双向**具名断言

```
reference field X is declared but no check category owns it — its references would be resolved by nobody
field Y is owned by a check category but is not a reference field — the reference scan would skip it
```

方向②正是本轮洞的近亲（`precondition` 曾在类别里，却只在一种处置下被扫）；方向①挡「新加字段进了
扫描域却没排进类别」。

## 后果

- **台账数据一字未改**：实测 `precondition` **只**出现在 4 条 `needs_precondition` 条目上
  （`PD-23`／`PD-26`／`PD-27`／`PD-30`）⇒ 触发条件放宽后**报错集合不变**，基线**保持全绿**。
- 新增 3 类具名失败：id 引用不可解析（作业项 / 验收 id / 两者都不是，措辞三分）——但**只在
  引用字段的每个元素上**，与处置无关。
- **不改基线正文、不改任何 Rust、不新增依赖边**（`pending-decisions.mjs` 无 import）。
- **它把 W80 的结论推到一般形式**：**「谁解析这个字段的引用」是字段的属性 ⇒ 触发条件 MUST 与
  数据无关**。W80 只证明了「域要完整」；本轮证明「**触发条件**也必须与数据无关」——
  一份完整的清单，配上一个条件性的触发，覆盖仍然是条件性的。

## 实施

```
node --test tests/registry.test.mjs            # 32 → 34
node .w83_inject.mjs                           # F0 + F1–F3（数据级）+ F4（反向）+ F5–F6（表级）+ F7–F8（反向）
cargo xtask check --profile full
```

## 遗留

1. **仍不判语义**：脚本只证「引用的 id 存在」，**不证该前置真的是这条裁定的前提**（沿用 ADR 0054 决策 2）。
2. **id 清单目前只有一个成员**：若将来需要「只要 `precondition` 出现在某处置上就禁止」这类**位置**
   约束，那是另一条断言（**预设**，非**内容**），MUST NOT 与本轮的可解析性检查合并。
3. **`note`／`closure` 等的处置归属不校验**（沿用 ADR 0058 遗留 2 / 0059 遗留 2 / 0060 遗留 2）。
4. **`question` 从此必须「引用可解析」**（沿用 ADR 0059 遗留 4）。
5. **`CAPABILITY_PACK_IDS` 仍手写**（沿用 ADR 0056 遗留 4）。
6. **§56.2 归档边界仍未写死**（W78 §八第 8 / W79 §八第 6 / W80 §八第 4 / W81 §八第 5 /
   W82 §八第 5 未变）：需一次升基线轮次写死。
7. `cargo xtask schema --check` 本机仍跑不了（宿主对 node 派生任何子进程 `EBUSY`）；本轮未改任何
   schema 与生成器输入 ⇒ 零漂移由结构性理由 + CI 独立步骤兜底，**未写成「已通过」**。

> **进展段（W84 / ADR 0062 追加）**：本轮（W84）在**同族的另一处**落下了同样的闭世界——
> 台账三层的盲区已由 W80–W83 补齐，W84 修的是**门控接线**这一层的补集（没有任何站点指向的
> Node 测试文件此前**静默不跑**）。上文遗留 6（**§56.2 归档边界**）**仍未写死**，本轮未动。
> 上文遗留 5（`CAPABILITY_PACK_IDS` 仍手写）**与本轮同类**：都是「一份需要维护的手写声明」，
> ADR 0062 决策 4 明确把它当成**有意的摩擦**接受。原文保留。
