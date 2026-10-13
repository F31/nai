# w86：OpenAPI 文档集的闭世界

## Why

W85 把「**哪些操作**被逐项校验」从一张手写方法清单改成**遍历文档自己的操作键**。但
「**哪些文档**被校验」仍然是手写清单——`validation.mjs` 的 `apiDocument()` 把文档路径硬编码：

```js
const document = await SwaggerParser.validate('schemas/openapi/control.yaml');
const errors = operationErrors(document);
```

`schemas/openapi/` 下还有一份 **`remote-knowledge.yaml`**，它是这份清单的补集：

- **本地门控从不读它**（`tests/contracts/schema.test.mjs` 一次都不提它；唯一读它的 `generate.mjs`
  只在 **CI-only** 的 `cargo xtask schema --check` 里跑，而本机对该步 `EBUSY`）；
- 它自己的生成器（W86 改动前位于 `generate.mjs:125`）又给「remote 的每个操作必须有 `operationId` 与
  `x-required-permission`」**另写了一份定义**——措辞、覆盖范围都与 `openapi-ops.mjs` 不同，
  是**同一件事的第二处定义**。

W86 探针实测（两份真实文档 + SwaggerParser）：

| 注入（加到 `remote-knowledge.yaml`） | SwaggerParser | 现有本地判据 |
| --- | --- | --- |
| 一个**无** `x-required-permission` 的 `patch` 操作 | **通过（没拦）** | **无人过问 —— 该文档根本不走进判据** |
| 同位置换 `get` | 通过（没拦） | 同上 |

⇒ 一份**承载鉴权语义**的契约文档，可以在本地门控与 CI 结构校验**双绿**的情况下，
新增一个**不声明权限**的操作。这与 W80–W85 是同一族洞的**第七处**（清单的载体从
数据字段／条目键／信封键／触发条件／门控接线／HTTP 方法，变成**契约文档集合本身**）。

## What Changes

- `scripts/contracts/openapi-ops.mjs`：
  - 新增 `OPENAPI_DIR` / `OPENAPI_EXTENSIONS`，文档集由**已跟踪路径**（`.git/index`）**派生**（W75／W84），
    MUST NOT 手写一份「有哪些文档」；
  - 新增 **`OPENAPI_DOCUMENTS`**：**逐文档声明**（`status` + `implemented`）的**单一来源**，
    逐字写死（`control.yaml` / `remote-knowledge.yaml` 各一条）；
  - 新增 `documentSetErrors(tracked, documents)`：文档集**闭世界**（在册描述 == 声明集，
    双向具名）+ 声明形状 + 非空洞（至少一条文档声明了非空 `implemented`）；
  - `operationErrors(document, declared)`：**逐文档参数化**，`declared` 是**必需入参**
    （缺即具名失败，MUST NOT 静默退回某个文档的默认值）。
- `scripts/contracts/validation.mjs`：`apiDocuments()` 遍历**声明集**逐份校验
  （SwaggerParser + 顶层状态 + `operationErrors`）；`apiDocument()` 保留为取 control 文档的薄封装。
- `scripts/contracts/generate.mjs`：删掉复述权限规则的那半句，改为**调用**同一判据
  （权限规则的**唯一家**是 `openapi-ops.mjs`），只保留生成器自己**真正需要**的 `operationId`。
- `tests/contracts/schema.test.mjs`：更新既有用例（改名／签名），新增文档集闭世界判据
  （含**反向注入**：一份新文档、声明完整，MUST NOT 红），并加一条文本守卫防回退。

不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边；
**两份 OpenAPI 文档一字未改**（实测均通过闭世界 ⇒ 报错集合不变）。

## Impact

- capability 差量：`versioned-contracts`（一条 ADDED Requirement —— 该能力本就承载
  「Validated configuration and API description」）。
- 新增 ADR 0064；ADR 0063 追加进展段（`IMPLEMENTED_OPERATIONS` 已重构为逐文档声明）。
- 新增活文档小节（`AGENTS.md`、`docs/development-plan.md`）。
- 基线保持全绿。
