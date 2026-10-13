# 提案（w82 台账信封的闭世界）

## Why

W81 给待裁定台账加了一条**闭世界**断言：条目里出现的每个键 MUST ∈ `LEDGER_FIELDS`。这条是对的，
但它只覆盖**一个层级**——**台账的顶层（信封）仍是开世界**。台账顶层只认三个键
（`schema_version`／`baseline`／`entries`），三个**各有**具名检查，而**第四个键没有任何人管**。

**实测（本轮探针，只用公开 API，不改被测文件）**：

```
=== 注入 UB1：顶层未知字段 revision = "见 §99.9" ===
errors = []          ← 完全静默

=== 注入 UB2：顶层未知字段 revision = "见 §40.4"（值干净） ===
errors = []

=== 注入 UB3：顶层放一个「条目字段名」 question: "§99.9" ===
errors = []          ← 两个闭世界都不认它，于是谁都不管
```

⇒ 本轮是 W81 的**直接下一层**，形状完全相同：**闭世界只在一个层级上成立时，另一个层级就是盲区**。

## What Changes

1. **声明信封字段集并使其封闭**（`scripts/contracts/pending-decisions.mjs`）：
   - 新增并导出 **`TOP_LEVEL_FIELDS`**（`schema_version`／`baseline`／`entries`，**逐字写死**）；
   - `validatePendingDecisions` 新增**信封闭世界**断言：台账顶层出现的每个键 MUST ∈
     `TOP_LEVEL_FIELDS`，否则**具名失败**（报那个键名），**不论该键的值是否含 `§NNN`**；
   - 同时断言信封字段**必需**：声明的每个顶层字段 MUST 出现在台账上——声明了却没有值，
     说明它是「声明了但没人读」的幽灵字段（**具名**）。
2. **扩展 `shapeErrors()`**：`TOP_LEVEL_FIELDS` ∩ `LEDGER_FIELDS` MUST 为空——一个字段名
   **只属于一个层级**；两边都声明 ⇒ 具名失败（否则把某个层级写错，两个闭世界**都放行**，
   而没有任何人读它，正是 UB3 的形状）。
3. **判据**（并入既有 `tests/registry.test.mjs`）：
   - 顶层未知键注入（**含值干净的一次**：证明挡的是**键**、不是它的内容）；
   - **条目字段名放到顶层**的注入（两个闭世界之间的缝）；
   - 逐字钉 `TOP_LEVEL_FIELDS`；信封字段逐个删除 ⇒ 各自具名必需失败；
   - 表级注入（把 `TOP_LEVEL_FIELDS` 改坏 ⇒ `shapeErrors()` 具名）。

## 范围

- **不改基线正文**（W72–W81 的 `diff` 三件套保持不变）；**不改 `tests/registry.yaml`、
  不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边。**
- **只改校验器与其判据**：`scripts/contracts/pending-decisions.mjs` + `tests/registry.test.mjs`。
  断言并入既有 Node 测试文件 ⇒ `xtask/src/lib.rs::engineering_checks` 的逐个显式列举**零改动**。
- **不重排 W 编号**：本版为 **W82**。
