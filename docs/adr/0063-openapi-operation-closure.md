# ADR 0063：OpenAPI 描述的操作集闭世界——判据 MUST 遍历文档自己的操作键

- 状态：已接受
- 日期：2026-10-13
- 依据：ADR 0058/0059/0060/0061/0062（同一族：「按清单做」的盲区是清单的补集）、
  W75（**「已声明」≠「已执行」**；声明 MUST 指回对象）、
  W82（闭世界 MUST 在**每个层级**上成立；相邻层 MUST 不相交）、
  W83（**触发条件 MUST 只由「字段存在」决定，MUST NOT 由处置/手写清单决定**）、
  W84（非空洞：判据 MUST 至少看到一个对象）
- 影响：新增 `scripts/contracts/openapi-ops.mjs`；重构 `scripts/contracts/validation.mjs` 的
  `apiDocument()`；`tests/contracts/schema.test.mjs` 增补判据；
  **不改 `schemas/openapi/control.yaml`、不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边**

## 背景

`apiDocument()` 的两道逐操作检查——**权限声明**（`x-required-permission`）与**实现状态**
（`x-implementation-status`）——都由一张**手写的方法清单**决定「哪些操作要被查」：

```js
const implemented = new Set(['get /health', 'get /runtime/status', …]);   // 八条，手写
for (const [path, item] of Object.entries(document.paths)) {
  for (const method of ['get', 'post', 'put', 'delete']) {                // 方法清单，手写
    if (item[method] && !item[method]['x-required-permission']) throw …
    const expected = implemented.has(`${method} ${path}`) ? 'implemented-loopback' : undefined;
    if (item[method]['x-implementation-status'] !== expected) throw …
  }
}
```

**手写清单的盲区恰好是它的补集**：`patch`／`head`／`options`／`trace` 都是 OpenAPI 3.0 的合法
方法键，全部落在 `['get','post','put','delete']` 之外。

**实测（W85 探针，非推断）**：往 `paths` 里加一个操作、用 `SwaggerParser` 校验、再跑上面那段循环的
**逐字逻辑**：

| 注入 | SwaggerParser | 手写循环 |
| --- | --- | --- |
| `patch` 操作（**无** `x-required-permission`、**无** `x-implementation-status`） | 通过（没拦） | **0 报错 ⇒ 静默通过** |
| 同位置换成 `get` 操作 | 通过（没拦） | 1 报错（`missing permission: get /w85-probe`） |

⇒ 一个 HTTP 方法只要落在清单之外，它的权限声明与实现状态**都不再被问**。这不是「漏检一个字段」，
而是**一整类操作可以从契约门口走进来而无人过问**——对一份承载鉴权语义的契约文档而言，这是安全方向
上的洞。

同族第二处（**潜伏**，当前计数为零）：`implemented` 里的八条**只被用来反查文档**，从未被要求
**指回文档里真实存在的操作**。把其中一条改成不存在的路径，循环照旧全绿——一条**只有声明、
没有对象**的契约。实测当前孤儿数 = 0，但**没有任何守卫**会拦住第一个孤儿。

同族登记（W80–W84 修的是台账三层与门控接线，本条修的是**契约文档的操作层**）：

| 轮次 | 「清单」 | 「清单的补集」 |
|---|---|---|
| W80 | 三个 `sectionRefs(...)` 手写字段 | `recommendation`/`closure`/`note`（17 条引用漏扫） |
| W81 | `CITATION_FIELDS` | 清单**之外**的新字段 |
| W82 | 条目的键集 | 信**封**（顶层）的键集 |
| W83 | `§` 一侧的触发规则 | id 一侧仍是**按处置**触发 |
| W84 | 逐条列举 + 两个 glob | 没有任何站点指向的测试文件 |
| **W85** | **`['get','post','put','delete']` 手写方法清单** | **`patch` 等清单外的方法（权限与状态都不查）** |

## 决策

1. **判据遍历文档自己的操作键，不遍历手写子集**（W83 的应用）。`operationErrors` 对每个
   path item 沿 `HTTP_METHODS` **逐个键**取操作——于是「新方法」与「老方法」走同一条路径。
   这是本轮洞的**主要补法**。

2. **`HTTP_METHODS` 是闭集且逐字写死**（八个：`get`/`put`/`post`/`delete`/`options`/`head`/
   `patch`/`trace`，OpenAPI 3.0 允许的全部方法键）。它 MUST NOT 由「文档里现有方法」派生——
   那样「现有方法 == 清单」是**恒真式**，闭世界退化成复述事实。仓库此刻只用到四个，
   但清单 MUST NOT 随之收缩：收缩后 `patch` 就重新落进补集（本 ADR 的 T1 注入正是这条）。

3. **`PATH_ITEM_FIELDS` 亦逐字写死**（五个：`$ref`/`summary`/`description`/`servers`/`parameters`），
   path item 的**每个键** MUST ∈ `HTTP_METHODS` ∪ `PATH_ITEM_FIELDS`（`x-` 扩展前缀按规范放行）。
   这一条同时解决两件事：① 闭集之外的方法键不再被静默跳过；② `gets:` 这类笔误被**具名拒绝**。

4. **相邻层 MUST 不相交**（W82）：`HTTP_METHODS` ∩ `PATH_ITEM_FIELDS` MUST 为空，由
   `operationShapeErrors()` 断言——一个键不能既被当作「要逐项检查的操作」又被当作「无需检查的字段」。

5. **`IMPLEMENTED_OPERATIONS` 逐字写死并 MUST 无孤儿**（W75）。该表从 `validation.mjs`
   **移到** `openapi-ops.mjs` 成为单一来源，并被 `operationErrors` **反查**：每一项 MUST 指回文档里
   真实存在、且 `x-implementation-status === IMPLEMENTED_STATUS` 的操作。理由是 W75 的老话——
   **「已声明」≠「存在」**；一条改名或删端点的改动否则会留下一条零信号的假契约。

6. **非空洞**：文档 MUST 至少暴露一个操作，否则「逐操作检查」全部为真但**空转**
   （与 W84 的「覆盖集非空」同形）。

7. **`x-` 扩展前缀有意放行**：规范规定规范扩展以 `x-` 开头。放行它不重开洞——扩展**不是操作**，
   不参与「逐操作检查」的语义；而一个想冒充操作的名字（`gets`／`patch2`）不以 `x-` 开头，仍被拒。

8. **判据是纯函数、入参是文档对象**：`operationErrors(document)` 便于注入验证，且
   `apiDocument()` 只负责「读文档 → 调判据 → 有错即抛」。判据与 I/O 分离（与
   `reachabilityErrors`／`reconcile` 同纪律）。

## 后果

- **正面**：`patch` 等清单外的方法从「静默跳过」变成「与别的方法同等检查」；孤儿声明从
  「零守卫」变成「具名拒绝」；方法键集的补集有了对立面（键集闭世界）。
- **代价**：`HTTP_METHODS`／`PATH_ITEM_FIELDS` 是**需要维护的手写声明**。这是**想要的**摩擦——
  将来 OpenAPI 引入新方法（如 `query`）必须显式改这份清单，而不是让它静静落进补集。
  与 `CAPABILITY_PACK_IDS`（ADR 0056 遗留 4）、`TEST_REGISTRATION_SITES`（ADR 0062 遗留 7）同类。
- **中性**：`schemas/openapi/control.yaml` **一字未改**。实测其 path item 键全部落在闭集内、
  42 个操作全部带权限声明、状态全部与声明一致、八条 `implemented` **零孤儿** ⇒ 收紧后
  **报错集合不变**，基线保持全绿。
- **假阳性方向已证**：一个**合法**的 `patch` 操作（带权限声明、不带实现状态）MUST NOT 红
  （反向注入 F5）——证明补的是「**检查**新方法」，不是「**拒绝**新方法」。

## 实施

- 模块：`scripts/contracts/openapi-ops.mjs`（`HTTP_METHODS`／`PATH_ITEM_FIELDS`／
  `EXTENSION_PREFIX`／`IMPLEMENTED_OPERATIONS`／`IMPLEMENTED_STATUS`／`operations()`／
  `operationShapeErrors()`／`operationErrors()`）。
- 接线：`validation.mjs` 的 `apiDocument()` 删掉手写的 `implemented` 集与方法清单，改为
  `operationErrors(document)` 非空即抛。**无新增接线点**（`apiDocument` 已由
  `tests/contracts/schema.test.mjs` 消费，该文件已在 `engineering_checks` 登记）。
- 判据：`tests/contracts/schema.test.mjs` **17 → 24**（新增 7 条用例：基线＋5 组注入＋
  2 组近似反向）。
- 注入证据：表级 F0／T1–T8（**11 / 11**，含两条反向注入）用**临时模块副本**（该文件无 import，
  副本可独立加载）逐项开火并还原；数据级由测试自身承载。

## 遗留

1. **判据只到「文档声明的操作键」**：它不检查「实现体是否真的实现了该操作」。`x-implementation-status`
   是**声明**，其真伪不在本 ADR 的范围；需要那层证据时应在运行时（而非契约文本）取证。
2. **`HTTP_METHODS`／`PATH_ITEM_FIELDS`／`IMPLEMENTED_OPERATIONS` 三张手写表的维护摩擦是有意的**
   （决策 2／3／5）。若将来 OpenAPI 版次引入新方法或新 path-item 字段，必须显式更新；
   更新前它们会被具名拒绝（安全方向），而不是被静默接受。
3. **`JSON Schema` 侧的 `control.yaml` 结构合法性仍由 `SwaggerParser` 负责**，本条只补**覆盖闭世界**；
   两者的分工 MUST NOT 互相替代。
4. §56.2 归档边界的正式裁定仍未落地（已被 W78 起连续七轮登记）。
5. harness 轨道仍 `pending` 的 **H18 与 B07**、W76 台账 5 条 `decidable_in_repo`
   （`PD-09/22/24/31/34`）的推荐落地，均未动手。
