# 提案（w81 台账字段集的封闭性）

## Why

W80 把待裁定台账的引用扫描域收成**一份导出的单一清单** `CITATION_FIELDS`，并在
`validatePendingDecisions` 里**遍历这个清单**。这解决了「清单内字段整片漏扫」，但留下一个更根本的盲区：

**「按清单扫」的守卫，其盲区恰好是清单的补集。** 清单挡得住「清单上的字段被编造」，挡不住
**不在清单上的字段被新增**。台账里加一个字段、而这个字段的引用**没有任何人核**——与 W80 修掉的那个洞
是同一件事，只是换了一层。

**实测（本轮探针，只用公开 API，不改被测文件）**：

```
=== 注入 UA：给 PD-01 加未知字段 rationale，内含 §99.9 ===
errors = []          ← 完全静默

=== 注入 UB：未知字段 rationale = "见 §40.4"（合法章节） ===
errors = []

=== 字段覆盖关系 ===
未覆盖字段 = id, question, disposition
```

同一探针还标出：`question`（34 条都有）**只有「非空」检查、不扫引用**。它是**字面意义上的复述字段**
——它复述基线的提问——而 W80 立的推理「复述处可以随便编」对它**最直接适用**，却唯独漏了它。

## What Changes

1. **声明字段集并使其封闭**（`scripts/contracts/pending-decisions.mjs`）：
   - 新增并导出 **`STRUCTURAL_FIELDS`**（`id`／`disposition`，由格式或枚举专属判定）；
   - 新增并导出 **`LEDGER_FIELDS`**——台账**允许出现**的全部字段，**逐字写死**（不由三类派生：
     派生出的「并集 == LEDGER_FIELDS」是恒真的，而 W80 的教训正是**只钉关系不够**）；
   - `validatePendingDecisions` 新增**闭世界**断言：条目里出现的每个键 MUST ∈ `LEDGER_FIELDS`，
     否则**具名失败**（报条目 id 与那个字段名），**不论该字段的值是否含 `§NNN`**。
2. **把 `question` 纳入引用扫描**：`CITATION_FIELDS` 增加 `question`——复述处的引用 MUST 可解析
   （实测当前 0 命中 ⇒ 不引入红；方向为安全侧）。
3. **扩展 `shapeErrors()`**：三类 MUST **两两不相交**，且其并集 MUST **恰等于** `LEDGER_FIELDS`
   ——某字段进了一处声明却漏在另一处时**指名**它。
4. **判据**（并入既有 `tests/registry.test.mjs`）：
   - 未知字段注入（**含值干净的一次**：证明挡的是**字段**、不是它的内容）；
   - `question` 引用注入（不存在的章节号 MUST 被抓、存在的 MUST 放行）；
   - 逐字钉 `LEDGER_FIELDS` 与三类划分；表级注入（把清单改坏 ⇒ `shapeErrors()` 具名）。

## 范围

- **不改基线正文**（W72–W80 的 `diff` 三件套保持不变）；**不改 `tests/registry.yaml`、
  不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边。**
- **只改校验器与其判据**：`scripts/contracts/pending-decisions.mjs` + `tests/registry.test.mjs`。
  断言并入既有 Node 测试文件 ⇒ `xtask/src/lib.rs::engineering_checks` 的逐个显式列举**零改动**。
- **不重排 W 编号**：本版为 **W81**。
