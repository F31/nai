# w85 设计：操作集闭世界

## 问题形状

`apiDocument()` 里的两道检查都是**逐操作**的，但「哪些操作被逐」由一张**手写方法清单**决定：

```js
for (const method of ['get', 'post', 'put', 'delete']) { … }
```

一张手写清单能挡住「清单内的方法被错写」，挡不住「**清单外的方法被新增**」——后者的补集
恰恰是清单本身不覆盖的那一块。`patch`／`head`／`options`／`trace` 都是 OpenAPI 3.0 的合法方法键，
全部落在清单之外。实测注入一个 `patch` 操作 ⇒ 判定返回**空错误**。

## 决策

1. **遍历文档自己的操作键，不遍历手写子集**（W83 的「触发条件 MUST 只由字段存在决定」）。
   `operationErrors` 对每个 path item 沿 `HTTP_METHODS` **逐个键**取操作——于是「新方法」与
   「老方法」走的是同一条路径。这是本轮洞的**主要补法**。

2. **`HTTP_METHODS` 是闭集且逐字写死**：`['get','put','post','delete','options','head','patch','trace']`
   （OpenAPI 3.0 允许的全部方法键）。它 MUST NOT 由「文档里现有方法」派生——派生出的
   「现有方法 == 清单」是恒真式（W80：手写的那一份会漂移，但**派生自事实的那一份不会报错**）。

3. **`PATH_ITEM_FIELDS` 亦逐字写死**（`['$ref','summary','description','servers','parameters']`），
   path item 的**每个键** MUST ∈ `HTTP_METHODS` ∪ `PATH_ITEM_FIELDS`（或 `x-` 扩展前缀）。
   这一条把两件事一起解决：① 闭集之外的**方法**键不再被静默跳过（`patch` 会被查）；
   ② 既不是方法、也不是标准字段的键（`gets:` 这类笔误）被**具名拒绝**。
   ⇒ **相邻层不相交**（W82）：`HTTP_METHODS` ∩ `PATH_ITEM_FIELDS` MUST 为空，由
   `operationShapeErrors()` 断言。

4. **`IMPLEMENTED_OPERATIONS` 逐字写死，且 MUST 无孤儿**（W75 的「已声明 ≠ 存在」）。
   该表 MOVE 到 `openapi-ops.mjs` 成为**单一来源**，并被 `operationErrors` **反查**：
   每一项 MUST 指回文档里真实存在、且 `x-implementation-status === IMPLEMENTED_STATUS` 的操作。
   没有这一条，表里的一条**改名/删端点**会变成一条**只有声明、没有对象**的契约，且零信号。

5. **`x-` 扩展前缀是有意放行的**：OpenAPI 规定「规范扩展」以 `x-` 开头。
   放行它不会重开洞——扩展**不是操作**，因此不参与「逐操作检查」的语义；
   而一个想冒充操作的名字（`gets`／`patch2`）不以 `x-` 开头，仍被具名拒绝。

6. **非空洞**：文档 MUST 至少暴露一个操作，否则「逐操作检查」全部为真但**空转**。
   与 W84 的「覆盖集非空」同形（那里的反面是「两集不相交」在空台账下退化成恒真）。

7. **只把 `operationErrors` 接进 `apiDocument()`，不给 `schemas/openapi/control.yaml` 加改动**。
   实测：文档的 path item 键全部 ∈ `{get,post,put,delete,parameters}`，方法均为 `get/post/put/delete`，
   42 个操作全部带权限声明、状态全部与声明一致、八条 `implemented` **零孤儿**
   ⇒ 收紧后**报错集合不变**，基线保持全绿。

## 假阳性方向（必须证明「不该红时不红」）

- `F0`：真实文档 ⇒ **空错误**（不引入红）。
- 反向注入 `F5`：一个**合法**的 `patch` 操作（带 `x-required-permission`，不带
  `x-implementation-status`）⇒ **MUST NOT 红**。这是本轮的**关键反向证据**：
  它证明第 1 条补的是「**检查**新方法」，而不是「**拒绝**新方法」。
- 反向注入 `F6`：把 `IMPLEMENTED_OPERATIONS` 的顺序打乱 ⇒ 不红（顺序不是语义）。

## 不做什么

- **不**把 `schemas/openapi/control.yaml` 的 42 个操作改写成另一种形状。
- **不**新增「声明式 OpenAPI 校验器」（那会把判据变成第二份会漂移的清单）。
- **不**动 `SwaggerParser` 的角色（它管结构合法性，本条管**覆盖闭世界**，两者互补）。
