# ADR 0060：台账信封的闭世界——闭世界 MUST 在**每个层级**上成立

- 状态：已接受
- 日期：2026-10-13
- 依据：ADR 0059（条目字段集的闭世界；遗留 3 把「顶层键由各自的具名检查判定」登记为边界）、
  W81（「按清单扫」的盲区恰是清单的补集）、W80（断言的「存在」≠ 断言的「覆盖」）、
  W79（搜索域 MUST 封闭）、W66（照抄表的常量要另有逐项写死的断言）、
  W73（假契约：每句「SHALL 被校验」都要能指出执行体）
- 影响：`scripts/contracts/pending-decisions.mjs`、`tests/registry.test.mjs`；
  **不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边**

## 背景

W81 给待裁定台账加了**闭世界**断言：条目里出现的每个键 MUST ∈ `LEDGER_FIELDS`。这条是**对的**，
但它把**层级**当成了背景——台账有**两个**层级：

| 层级 | 键 | 谁在管（改前） |
| --- | --- | --- |
| 信封（顶层） | `schema_version` | `schema_version must be 1` |
| 信封（顶层） | `baseline` | `ledger declares baseline … but the current baseline is …` |
| 信封（顶层） | `entries` | `entries must be a non-empty list` |
| 信封（顶层） | **（第四个键）** | **没有人** |
| 条目 | 十个字段 | W81 的闭世界断言 |

三个已知信封键**各有**具名检查 ⇒ 这不是「检查漏写」，而是**键集不封闭**：

> **已知键各有人管 ≠ 键集封闭。** 新增的第一个键就落进盲区。

**实测（W82 探针，只用公开 API，不改被测文件）**：

```
=== 注入 UB1：顶层未知字段 revision = "见 §99.9" ===
errors = []          ← 完全静默

=== 注入 UB2：顶层未知字段 revision = "见 §40.4"（值干净） ===
errors = []

=== 注入 UB3：顶层放一个「条目字段名」 question: "§99.9" ===
errors = []          ← 两个闭世界都不认它，于是谁都不管
```

**UB3 是最尖锐的一条**：它同时暴露了「层级」这件事——**闭世界的完整性不只要求每个层级封闭，
还要求两个层级的字段名集不相交**。把一个条目字段名写到顶层时，条目闭世界看不见它（它在顶层），
信封闭世界还不存在 ⇒ 没有任何信号。

## 决策

### 决策 1：`TOP_LEVEL_FIELDS` 逐字写死

```js
export const TOP_LEVEL_FIELDS = ['schema_version', 'baseline', 'entries'];
```

与 `LEDGER_FIELDS` 同理：必须是**独立的一份**声明，MUST NOT 由「现有检查读了哪些键」派生
——派生出的闭世界是**恒真**的，一个字都没说（W80 §九结论 5：只钉「关系」不够，要逐字钉内容）。

### 决策 2：信封闭世界断言**只看键名、不看值**

```
unknown envelope field revision — a top-level field the checker does not know can carry §NNN unchecked
```

与 W81 决策 2 同型：若写成「未知键**且**其值含 `§NNN`」才报，判据的输入（值）就**不等于**
被断言的对象（键集）。⇒ 注入 MUST 含一次**值干净**的顶层未知键——**它同样必须失败**。

### 决策 3：信封字段 MUST **必需**

```
envelope field baseline is missing — a declared envelope field with no value is a field nobody reads
```

闭世界只挡「**不认识的**键」，它对「**认识的**键没人读」是瞎的。把「必需」也写成断言，等于把
`TOP_LEVEL_FIELDS` 的每个成员绑到**数据上**：**声明一个字段就必须在台账里有它**，否则基线立刻红
（往声明集里错加幽灵字段会**响得早**，而不是「静静地什么都不检查」）。

### 决策 4：两个层级的字段名集 MUST 不相交

`shapeErrors()` 新增：`TOP_LEVEL_FIELDS` ∩ `LEDGER_FIELDS` MUST 为空 ⇒ 具名报出那个字段名。

**为什么这不是装饰性断言**：UB3 的形状（把条目字段名写到顶层）**两个闭世界都不管**，所以一旦某次
编辑顺手把 `question` 也加进 `TOP_LEVEL_FIELDS`（意图是「让那个顶层 `question` 合法」），UB3 的洞就
**原样重开、且完全无声**。这条断言是那个洞**唯一**的守卫。

**方向**：这里选**互斥**（拒绝），不是「两边都允许」——理由同 W81 决策 3：一个字段名只属于一个层级
这件事本身可判定，而「既属信封又属条目」会让「这个字段由谁读」不再是单一回答。

### 决策 5：两层各自保留自己的措辞

一层报**条目 id + 字段名**（`PD-01: unknown field rationale …`），一层报**键名**
（`unknown envelope field revision …`）。**MUST NOT 合并**——合并会让「错在哪一层」在措辞上消失。

## 后果

- **台账数据一字未改**：实测顶层键**恰好**等于 `TOP_LEVEL_FIELDS`（三个）⇒ 基线**保持全绿**
  （「本轮不引入红」的判据）。
- 新增 4 类具名失败：顶层未知键（报键名）、信封字段缺失（报字段名）、字段名同属两层（报字段名）。
- **不改基线正文、不改任何 Rust、不新增依赖边**（`pending-decisions.mjs` 无 import，
  表级注入用的临时副本因此可独立 import）。
- **它把 W81 的结论推到一般形式**：**闭世界 MUST 在每个层级上成立**，且**相邻层级的字段名集
  MUST 不相交**。只在一个层级封闭，等于把盲区搬到了上一层——而**上一层是「更少人看」的那一层**。

## 实施

```
node --test tests/registry.test.mjs            # 30 → 32
node .w82_inject.mjs                           # F0 + F1–F4（数据级）+ F5–F9（表级，含两条反向）
cargo xtask check --profile full
```

## 遗留

1. **仍不判语义**：脚本只证「引用的章节号存在」，**不证该章节回答了该问题**（沿用 ADR 0054 决策 2）。
2. **`note` 的处置归属不校验**（沿用 ADR 0058 遗留 2 / 0059 遗留 2）。
3. **信封只覆盖「顶层键集」**：`schema_version`／`baseline`／`entries` 的**值**由各自的具名检查判定，
   本轮的闭世界只保证**它们的名字被认识、且必须出现**；新增信封字段 MUST 同时补一条值检查。
4. **`question` 从此必须「引用可解析」**（沿用 ADR 0059 遗留 4）。
5. **`CAPABILITY_PACK_IDS` 仍手写**（沿用 ADR 0056 遗留 4）。
6. **§56.2 归档边界仍未写死**（W78 §八第 8 / W79 §八第 6 / W80 §八第 4 / W81 §八第 5 未变）：
   需一次升基线轮次写死。
7. `cargo xtask schema --check` 本机仍跑不了（宿主对 node 派生任何子进程 `EBUSY`）；本轮未改任何
   schema 与生成器输入 ⇒ 零漂移由结构性理由 + CI 独立步骤兜底，**未写成「已通过」**。
