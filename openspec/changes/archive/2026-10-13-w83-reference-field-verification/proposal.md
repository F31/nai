# 提案（w83 引用字段的验证范围）

## Why

W80 给待裁定台账确立了**一条**纪律：

> **扫描按「字段是否存在」触发，MUST NOT 按处置触发**——否则「注解型的字段」会被漏掉。
> （当时的具体发现：`note` 不是 `out_of_scope` 的专属字段，实测 5 条里 4 条挂在别的处置上。）

**但 W80 只把这条纪律应用到了 `CITATION_FIELDS`（承载 `§NNN` 的那些字段）这一侧。**
台账里还有**另一类引用**——登记 id 引用（`precondition` 的元素，指向基线作业项或注册表验收 id）。
它的校验是**按处置触发的**：

```js
if (entry.disposition === 'needs_precondition' && Array.isArray(entry.precondition)) {
  for (const item of entry.precondition) { /* 校验每个 id */ }
}
```

`precondition` 是 `ID_LIST_FIELDS` 的**唯一**成员；类别表声明它「**按 id 校验**」。
而实测：把 `precondition: ["W99"]` 挂到一条 **非 `needs_precondition`** 的条目上，
**校验完全静默**——那条前置声称「等 W99 完成」，而 **W99 在基线里不存在**。

**实测（本轮探针 v2，用真实注册表，只用公开 API，不改被测文件）**：

```
F0 基线错误数 = 0                        ← 基线为绿

=== P1 PD-01 (needs_external) 加 precondition=['W99'] ===
  (无新增错误 —— 静默接受)

=== P1b PD-01 (needs_external) 加 precondition=['H99'] ===
  (无新增错误 —— 静默接受)

=== P2 PD-25 (answered_by_baseline) 加 precondition=['不是 id 的字符串'] ===
  (无新增错误 —— 静默接受)

=== P2b PD-09 (decidable_in_repo) 加 precondition=['W99'] ===
  (无新增错误 —— 静默接受)

=== P3 PD-09 (decidable_in_repo) 加 closure='§99.9' ===   ← 对照：§ 侧
  + PD-09: closure references §99.9, which does not exist in the baseline
```

**P3 是对照组**：同在一条 `decidable_in_repo` 条目上，`closure` 里的坏 `§` 被抓、`precondition`
里的坏 id 不被抓——**同一个洞的两半，只有一半被 W80 补上**。

⇒ 一个字段由谁解析它的引用，是**字段的属性**，与「这条数据当前是什么处置」无关。
把「谁负责」写成 `disposition === 'X' && …` 的合取，等于让「这个字段有主」变成**条件性的**：
换个处置，同一个字段就**没人管**——而 `ID_LIST_FIELDS` 的存在恰恰是为了说「它有主」。

## What Changes

1. **声明引用字段集并使其封闭**（`scripts/contracts/pending-decisions.mjs`）：
   - 新增并导出 **`REFERENCE_FIELDS`**（`CITATION_FIELDS` 的七个 ＋ `ID_LIST_FIELDS` 的
     `precondition`，**逐字写死**的八个），作为「承载必须可解析的引用」的**单一来源**；
   - `validatePendingDecisions` 的条目循环把「§ 扫描」与「precondition id 校验」合并为**一个**
     按 `REFERENCE_FIELDS` 遍历、**只由字段存在触发**的循环：`CITATION_FIELDS` 的成员解析 `§NNN`，
     `ID_LIST_FIELDS` 的成员解析登记 id；
   - 触发的判断里**不再出现 `disposition`**——`disposition` 只决定「哪个字段是必填的」（预设断言），
     不决定「哪个字段的引用要解析」。
2. **扩展 `shapeErrors()`**：`REFERENCE_FIELDS` 与 `CITATION_FIELDS ∪ ID_LIST_FIELDS`
   MUST **双向相等**——两个方向都具名。① 声明了却无类别认领 ⇒ 它的引用**没人解析**；
   ② 被类别认领却不在声明集里 ⇒ 引用扫描会**跳过它**。
3. **判据**（并入既有 `tests/registry.test.mjs`）：
   - **泛化到清单**的行为用例：对 `ID_LIST_FIELDS` 的**每个**成员，在一个**处置不等于其必填处置**
     的条目上注入坏 id（不存在的作业项 / 注册表没有的验收 id / 非 id 字符串）⇒ MUST 失败并
     **指名条目 + 字段 + 值**；
   - **反向注入**（不该红时不红）：在**非专属处置**的条目上放一个**合法** id ⇒ MUST NOT 红
     （本轮拒绝的是「**不可解析**的引用」，不是「**放错位置**的引用」）；
   - 逐字钉 `REFERENCE_FIELDS` 的内容 + 非空洞（每个成员都真的在台账里出现过）。

## 范围

- **不改基线正文**（W72–W82 的 `diff` 三件套保持不变）；**不改 `tests/registry.yaml`、
  不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边。**
  实测 `precondition` **只**出现在 4 条 `needs_precondition` 条目上 ⇒ 触发条件改宽后
  报错集合**不变**，基线**保持全绿**。
- **只改校验器与其判据**：`scripts/contracts/pending-decisions.mjs` + `tests/registry.test.mjs`。
  断言并入既有 Node 测试文件 ⇒ `xtask/src/lib.rs::engineering_checks` 的逐个显式列举**零改动**。
- **不重排 W 编号**：本版为 **W83**。
