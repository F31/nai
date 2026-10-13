# w85：OpenAPI 描述的操作集闭世界

## Why

`apiDocument()` 在**两道**逐操作检查（权限声明、实现状态）上都用了**手写的方法清单**：

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

**手写清单的补集就是盲区**——这与 W80–W84 是同族洞。

W85 探针实测（往 `paths` 里加一个操作、用 SwaggerParser 校验、再跑上面那段循环的逐字逻辑）：

| 注入 | SwaggerParser | 手写循环 |
| --- | --- | --- |
| `patch` 操作（**无** `x-required-permission`、**无** `x-implementation-status`） | 通过（没拦） | **0 报错 ⇒ 静默通过** |
| 同位置换成 `get` 操作 | 通过（没拦） | 1 报错（`missing permission: get /w85-probe`） |

⇒ 一个 HTTP 方法只要落在 `['get','post','put','delete']` 之外，它的**权限声明与实现状态都不再被检查**。
这不只是「漏检一个字段」，而是**一整类操作可以从契约门口走进来而无人过问**。

同族第二处（**潜伏**，当前计数为零）：`implemented` 里的八条**只被用来反查文档**，从未被要求
**指回文档里真实存在的操作**。把其中一条改成不存在的路径，循环照旧全绿——一条**只有声明、没有对象**
的契约。当前实测孤儿数 = 0，但**没有任何守卫**会拦住第一个孤儿。

## What Changes

- 新增 `scripts/contracts/openapi-ops.mjs`（**操作集闭世界**，与 `test-reachability.mjs` 同形）：
  - 导出 `HTTP_METHODS`（**八个**，逐字写死：OpenAPI 3.0 允许的全部方法键）、
    `PATH_ITEM_FIELDS`（**五个**，逐字写死）、`IMPLEMENTED_OPERATIONS`（**八条**，逐字写死）、
    `IMPLEMENTED_STATUS`，以及 `operations()` / `operationShapeErrors()` / `operationErrors()`；
  - **触发只由「文档里存在这个方法键」决定**（W83）：遍历 `HTTP_METHODS`，MUST NOT 遍历手写子集；
  - **方法键集闭世界**（W81／W84）：path item 的每个键 MUST ∈ `HTTP_METHODS` ∪ `PATH_ITEM_FIELDS`
    （或 OpenAPI 规定的 `x-` 扩展前缀）⇒ `patch` 从此**被检查**而不是被跳过，`gets:` 这类笔误**被具名拒绝**；
  - **两集不相交**（W82）：`HTTP_METHODS` ∩ `PATH_ITEM_FIELDS` MUST 为空；
  - **声明无孤儿**（W75 的「已声明 ≠ 存在」）：`IMPLEMENTED_OPERATIONS` 的每一项 MUST 指回
    文档里真实存在、且确实带 `IMPLEMENTED_STATUS` 的操作；
  - **非空洞**：文档 MUST 至少暴露一个操作（与 W84 的「覆盖集非空」同形）。
- 重构 `scripts/contracts/validation.mjs` 的 `apiDocument()`：删掉手写的 `implemented` 集与
  方法清单，改为调用 `operationErrors(document)`，非空即抛错。
- `tests/contracts/schema.test.mjs` 增补判据（含**反向注入**：一个合法的 `patch` 操作，
  带权限声明、不带实现状态，MUST NOT 红）。

不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边。

## Impact

- capability 差量：`versioned-contracts`（一条 ADDED Requirement —— 该能力本就承载
  「Validated configuration and API description」）。
- 新增 ADR 0063。
- 新增活文档小节（`AGENTS.md`、`docs/development-plan.md`）。
- `schemas/openapi/control.yaml` **一字未改**（实测其 path item 键全部落在闭集内、无孤儿声明）；
  基线保持全绿。
