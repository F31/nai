# 设计 · W83 引用字段的验证范围

## 1. 问题：W80 的纪律只落地了一半

W80 修的是「扫描域被手写成三个字段、另一批承载 `§NNN` 的字段整片漏扫」。修完之后它留下的
一般结论是：

> **扫描按「字段是否存在」触发，MUST NOT 按处置触发。**

这条结论被**写进了 `CITATION_FIELDS` 的注释**，也确实在 § 侧执行了。但台账里的引用有**两类**：

| 类别 | 字段 | 引用的形态 | 由谁解析 | 触发条件 |
| --- | --- | --- | --- | --- |
| `CITATION_FIELDS` | 七个 | `§NNN` | 基线章节集 | **字段存在**（已对） |
| `ID_LIST_FIELDS` | `precondition` | 登记 id（`W##`／验收 id） | 作业项集 ∪ 验收 id 集 | **`disposition === 'needs_precondition'`**（错） |

第二行的触发条件是**处置**——正是 W80 明令禁止的形状，只是它当时只看了第一行。

**为什么「按处置触发」是错的方向**：`ID_LIST_FIELDS` 这个类别**存在**的意义，就是声明
「这个字段的引用由 id 检查负责」。若那个检查只在一种处置下运行，那么这个声明就是
**条件性的**：`precondition` 一旦出现在别的处置上，类别表说「它有主」，实际**没有任何人管它**。
（W73 的「假契约」同族：一句「SHALL 被校验」指出的执行体，只在某些输入上才执行。）

## 2. 决策

### 决策 1：`REFERENCE_FIELDS` 逐字写死

```js
export const REFERENCE_FIELDS = [
  'basis', 'introduced_in', 'also_in', 'recommendation',
  'closure', 'note', 'question', 'precondition',
];
```

与 `LEDGER_FIELDS`／`TOP_LEVEL_FIELDS` 同理：它 MUST 是**独立的一份**声明，MUST NOT 由
`[...CITATION_FIELDS, ...ID_LIST_FIELDS]` 派生——派生出来的「声明集 == 两类别之并」是
**恒真**的，一个字都没说（W80 §九结论 5：只钉「关系」不够，要**逐字钉内容**）。
写死之后，`shapeErrors()` 的「双向相等」才是一条**内容断言**：新加一个 id 清单字段却忘了
把它放进扫描域，会在 `shapeErrors()` 里被**具名**点名。

### 决策 2：扫描循环由 `REFERENCE_FIELDS` 驱动，触发条件**只看字段存在**

条目循环里唯一的引用扫描循环：

```js
for (const field of REFERENCE_FIELDS) {
  const value = entry[field];
  const values = Array.isArray(value) ? value : typeof value === 'string' ? [value] : [];
  if (citationSet.has(field)) {
    /* 每个 §NNN MUST 存在于当前基线 */
  } else {
    /* 每个 id MUST 解析为基线作业项或注册表验收 id */
  }
}
```

**循环体里没有 `disposition`。** 这不是风格问题：`disposition` 一旦进了引用扫描的触发条件，
「这个字段的引用要不要查」就变成了**数据的属性**而非**字段的属性**——而类别表断言的是后者。

`disposition` 仍然管一件事：**哪个字段是必填的**（`REQUIRED_FIELD`）。这两件事 MUST 分开：

- 「必填」是**预设**断言（这条处置 SHALL 带那个字段）——按处置触发**正确**；
- 「引用可解析」是**内容**断言（这个字段的每个引用 SHALL 解析得开）——按处置触发**错误**。

把两者混在一个 `if` 里，正是本轮修的那个洞。

### 决策 3：错误措辞「条目 + 字段 + 值」三件齐

```
PD-01: precondition W99 is not a work item named by the baseline
PD-01: precondition H99 is not a registry acceptance id
PD-01: precondition 以后再说 is neither a registry acceptance id nor a baseline work item
```

与 § 侧的 `PD-xx: closure references §99.9, …` 对齐：同一个坏引用可能同时出现在多个字段里，
只说条目无法定位（W79：错误的措辞会把定位引向错误的方向）。原来 `precondition` 的错误里
「precondition」是**写死的字面量**；改由 `${field}` 生成，措辞不变，但**将来再加 id 清单字段
时自动正确**——写死的字面量在那一刻会**说谎**。

### 决策 4：只拒「不可解析的引用」，不拒「放错位置的引用」

`precondition` 出现在**非**专属处置上时，只要它的每个 id 都解析得开，本轮**不报错**。

**方向**：假阳性与假阴性不对称 ⇒ 永远选**安全**方向。一个**可解析**的 `precondition` 是
「描述了一个真实前置」，它奇怪但不虚假；拒绝它会把合法数据判红（**假阳性**），而本轮真正要挡的
是「**凭空编一个不存在的 id**」（**假阴性**在安全侧）。⇒ 判据的输入 MUST 等于被断言的对象：
这里断言的是「引用可解析」，不是「字段用对了处置」。

### 决策 5：`shapeErrors()` 的双向断言

```
reference field X is declared but no check category owns it — its references would be resolved by nobody
field Y is owned by a check category but is not a reference field — the reference scan would skip it
```

两个方向都具名。方向①挡「新加字段进了扫描域却没排进类别」；方向②挡「新加了类别成员却没进扫描域」
——后者正是本轮洞的近亲（`precondition` 曾在类别里、却在多数处置下不被扫）。

## 3. 不改什么

- **不改台账数据**：实测 `precondition` **只**出现在 4 条 `needs_precondition` 条目上
  （`PD-23`／`PD-26`／`PD-27`／`PD-30`）⇒ 触发条件放宽后**报错集合不变**，基线**保持全绿**
  （「本轮不引入红」的判据）。
- **不改基线正文、不改任何 Rust、不新增依赖边。**
- **不与条目闭世界、信封闭世界合并**：那两层管「**哪些名字允许出现**」，本轮管「**出现的引用
  能不能解析**」——维度不同（W66：不合并维度）。合并会让「错在哪一层」在措辞上消失。
- **不改 `CHAPTER_COUNTS`／`DISPOSITION_COUNTS`／`TOTAL_ENTRIES`**：那些是覆盖性与分布，
  本轮的边界在**单个引用的可解析性**。
