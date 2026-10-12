# 提案（w80 待裁定台账的引用完整性）

## Why

W76 立的待裁定台账（`tests/pending-decisions.yaml` + `scripts/contracts/pending-decisions.mjs`）
有一条断言：**条目里引用的每个 `§NNN` MUST 真实存在于当前基线**——把「凭印象编一个章节号」排除掉。

但这条断言的**扫描域是手写的三个字段**：

```js
const refs = [
  ...sectionRefs(entry.basis),
  ...sectionRefs(entry.introduced_in),
  ...(Array.isArray(entry.also_in) ? entry.also_in.flatMap(sectionRefs) : []),
];
```

而 `REQUIRED_FIELD` 里**另外三个同样承载 `§NNN` 的字段**——`recommendation`、`closure`、`note`
——**一条都没扫**。实测（本轮探针，只用公开 API）：

```
[已扫] basis           引用数=4
[已扫] introduced_in   引用数=34
[已扫] also_in         引用数=28
[未扫] recommendation  引用数=4      ← PD-24 引 §40.4、PD-31 引 §5.5、PD-34 引 §14.1／§54.2
[未扫] closure         引用数=9      ← §52.1／§38.8／§38.4／§10.2／§8.2／§46.2 …
[未扫] note            引用数=4      ← §46.3／§43.4／§47.2／§45.3
```

**注入实测**：把 `PD-24` 的 `recommendation` 里 `§40.4` 改成不存在的 `§99.9`
⇒ `validatePendingDecisions` 返回 **空错误**。**17 条引用无人核。**

这恰是既有测试里那句注释所担心的形状——「`basis` 之外的引用同样受检，否则**复述处可以随便编**」
——只是它把「之外」当成了 `introduced_in`／`also_in`，**漏了真正承载 17 条引用的那三个字段**。

同一处还有**第二类洞：校验器自身两张常量表可以分家**。

```js
if (!DISPOSITIONS.includes(entry.disposition)) { … }
…
const field = REQUIRED_FIELD.get(entry.disposition);
if (field) { … }        // ← 若该处置漏在 REQUIRED_FIELD 里，field 是 undefined ⇒ 整段跳过
```

若某处置进了 `DISPOSITIONS` 却漏在 `REQUIRED_FIELD` 里，它的条目就**没有必填字段**——
校验对它**沉默**。同理，若某处置的必填文本字段不在扫描清单里，它的引用就**永不被解析**。
两种都是**危险方向的瞎**：断言看起来还在，覆盖已经没了。

## What Changes

1. **扫描域由单一清单派生**（`scripts/contracts/pending-decisions.mjs`）：新增
   **`CITATION_FIELDS`**（`basis`／`introduced_in`／`also_in`／`recommendation`／`closure`／`note`）
   与 **`ID_LIST_FIELDS`**（`precondition`，承载 id 而非 `§NNN`）。引用扫描改为**遍历这个清单**，
   不再手写三个 `sectionRefs(...)` 调用——手写的那一份会漂移。
2. **错误具名到字段**：引用不可解析时报
   `PD-24: recommendation references §99.9, which does not exist in the baseline`
   （**条目 + 字段 + 章节号**三件齐，否则无法定位）。
3. **校验器自身的一致性**（新 `shapeErrors()`，随 `validatePendingDecisions` 一并返回）：
   - `REQUIRED_FIELD` 的键集 MUST == `DISPOSITIONS`（否则新增处置会「无必填字段」地通过）；
   - 每个处置的必填字段 MUST 落在 `CITATION_FIELDS ∪ ID_LIST_FIELDS` 内、且两清单**不相交**
     （否则新增处置的字段会在两个检查之间落空）。
4. **判据**（并入既有 `tests/registry.test.mjs`）：对**每个此前未扫的字段**各注入一次并**指名**抓到；
   另加一条直接断言 `CITATION_FIELDS` 逐字含那三个字段、且 `REQUIRED_FIELD` 的键集与处置集相同。

## 范围

- **不改基线正文**（W72–W79 的 `diff` 三件套保持不变）；**不改 `tests/registry.yaml`、
  不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边。**
- **只改校验器与其判据**：`scripts/contracts/pending-decisions.mjs` + `tests/registry.test.mjs`。
  断言并入既有 Node 测试文件 ⇒ `xtask/src/lib.rs::engineering_checks` 的逐个显式列举**零改动**。
- **不重排 W 编号**：本版为 **W80**。
