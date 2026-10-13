# w86 设计：文档集闭世界

## 问题形状

W85 修好了「**哪些操作**被逐项校验」，但「**哪些文档**被校验」还是一张手写清单：

```js
const document = await SwaggerParser.validate('schemas/openapi/control.yaml');  // ← 硬编码
const errors = operationErrors(document);
```

手写清单能挡住「清单里那份文档被改坏」，挡不住「**清单外的文档被新增**」——后者的补集恰恰是
清单不覆盖的那一块。`schemas/openapi/` 下已有第二份描述 `remote-knowledge.yaml`，实测：

- **本地门控从不读它**：`tests/contracts/schema.test.mjs` 一次都不提它；唯一读它的 `generate.mjs`
  只在 **CI-only** 的 `cargo xtask schema --check` 里运行，而本机对该步 `EBUSY`（宿主禁 node
  派生子进程）⇒ 本机对它**零校验**。
- **CI 也只做结构校验**：生成器（W86 改动前位于 `generate.mjs:125`）另写了一份「remote 操作必须有 `operationId` 与
  `x-required-permission`」的循环——这是**同一件事的第二处定义**（措辞、覆盖都不同），
  且 `SwaggerParser` 对「操作不声明权限」**放行**（`x-` 是规范允许的扩展前缀，结构合法）。

⇒ 一个**承载鉴权语义**的操作可以在本机绿 + CI 绿的情况下**不声明权限**。同族第七处。

## 决策

1. **文档集 MUST 由已跟踪路径派生，MUST NOT 手写**（W75／W84）。
   `openapiDocumentPaths(tracked)` 取 `OPENAPI_DIR` 下扩展名 ∈ `OPENAPI_EXTENSIONS` 的**在册**路径。
   与 W84 同一条理由：用 `.git/index` 而不是 `readdir` ⇒ 工作区里未跟踪的散落文件
   不参与覆盖集，「仓库里有哪些契约」这句话的输入就是**仓库**。

2. **逐文档声明 `OPENAPI_DOCUMENTS` 是单一来源，逐字写死**。
   键 = 仓库相对路径；值 = `{status, implemented}`。`status` 是顶层 `x-implementation-status`
   的**期望值**（`control.yaml` = `partial-loopback-engineering`、`remote-knowledge.yaml` = `client-only`），
   `implemented` 是那份文档自己的已实现操作表（remote 为**空**——它自述 `client-only`）。
   ⇒ 原先硬编码在 `apiDocument()` 里的字符串 `'partial-loopback-engineering'` 与
   `IMPLEMENTED_OPERATIONS` 都变成**声明**，不再散落在执行体里。

3. **判据逐文档参数化，且 `declared` 是必需入参**。
   `operationErrors(document, declared)`：`declared` 缺失／不是数组 ⇒ **具名失败**，
   MUST NOT 退回某个模块级默认值。理由与 `resolveSite`／`reconcile`／`validatePendingDecisions`
   同纪律：一条会因「忘记传参」而恒真（用错文档的声明）的断言，比没有断言更坏。

4. **文档集闭世界是双向的**：在册描述 MUST ⊆ 声明集，声明集 MUST ⊆ 在册描述。
   单方向的「在册 ⊆ 声明」挡不住**陈旧声明**（指向已删除的文档）；单方向的「声明 ⊆ 在册」
   挡不住**逃逸描述**（新增文档无人覆盖）。两条一起才等于「集合相等」，且各报具名。

5. **权限规则的唯一家是 `openapi-ops.mjs`**。
   `generate.mjs` 改为**调用** `operationErrors`，删掉复述的 `x-required-permission` 判断，
   只保留它**真正需要**的约束（`operationId`——生成 `*_PATH`／`*_METHOD` 常量要名字）。
   这与仓库既有的 `TS_GENERATED_SOURCES` 处置同形（注释原文：*A second copy here would be a
   list that can disagree with the first one in silence*）⇒ 判据留**一处定义、两处使用**。

6. **非空洞**：文档集非空；每份文档至少暴露一个操作；且**至少一份**文档声明了非空 `implemented`
   ——否则「声明无孤儿」这一半在所有文档上**空转**（W84 的「空台账使两集不相交退化成恒真」同形）。

7. **不给两份 OpenAPI 文档加改动**。实测两份文档各自通过闭世界（键集闭合、逐操作带权限、
   状态与声明一致、声明零孤儿）⇒ 收紧后**报错集合不变**，基线保持全绿。

## 假阳性方向（必须证明「不该红时不红」）

- `F0`：两份真实文档 ⇒ **空错误**（不引入红）。
- **反向注入**：一份**新的**描述（声明完整、操作带权限、无已实现声明）⇒ **MUST NOT 红**。
  这是本轮的关键反向证据：补的是「**覆盖**文档集」，不是「**只允许**一份文档」。
- 反向：把声明表里的键**顺序打乱** ⇒ 不红（顺序不承载语义）。
- 反向：还原后基线（**仍绿**），源文件未被改动。

## 不做什么

- **不**新增「声明式 OpenAPI 校验器」（那会把判据变成第二份会漂移的清单）。
- **不**动 `SwaggerParser` 的角色（它管结构合法性，本条管**覆盖闭世界**，两者互补）。
- **不**把 `remote-knowledge.yaml` 的六条操作硬塞进 `control.yaml` 的声明表——每份文档的
  已实现集合是**那份文档**的属性。
