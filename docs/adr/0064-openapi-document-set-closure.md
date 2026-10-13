# ADR 0064：OpenAPI 文档集的闭世界——操作集判据 MUST 覆盖仓库里每一份描述

- 状态：已接受
- 日期：2026-10-13
- 依据：ADR 0063（操作集闭世界）、ADR 0058/0059/0060/0061/0062（同一族：「按清单做」的盲区是清单的补集）、
  W75（**判据的输入 MUST 等于它断言的对象**；「已声明」≠「已存在」）、
  W82（闭世界 MUST 在**每个层级**上成立）、
  W83（**入参 MUST 显式注入，缺即失败**，MUST NOT 静默降级）、
  W84（**覆盖集由已跟踪路径派生，MUST NOT 手写**；非空洞 ≠「数量 > 0」）
- 影响：重构 `scripts/contracts/openapi-ops.mjs`（新增文档集与逐文档声明）；
  `scripts/contracts/validation.mjs` 的 `apiDocument()` → `apiDocuments()`；
  `scripts/contracts/generate.mjs` 改为**调用**判据而不复述它；
  `tests/contracts/schema.test.mjs` 增补判据；
  **不改 `schemas/openapi/control.yaml`、不改 `schemas/openapi/remote-knowledge.yaml`、不改基线正文、
  不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边**

## 背景

W85 把「**哪些操作**被逐项校验」从一张手写方法清单改成**遍历文档自己的操作键**。但
「**哪些文档**被校验」仍然是一张手写清单——`apiDocument()` 把描述路径**硬编码**：

```js
const document = await SwaggerParser.validate('schemas/openapi/control.yaml');   // ← 硬编码
const errors = operationErrors(document);
```

`schemas/openapi/` 下还有一份 `remote-knowledge.yaml`，它是这份清单的补集：

1. **本地门控从不读它**。`tests/contracts/schema.test.mjs` 一次都不提它；唯一读它的
   `scripts/contracts/generate.mjs` 只在 **CI-only** 的 `cargo xtask schema --check` 里运行，
   而本机对该步 `EBUSY`（宿主禁止 node 派生任何子进程）⇒ 本机对它**零校验**。
2. **CI 也只做结构校验**。`SwaggerParser` 对「操作不声明权限」**放行**——`x-` 是 OpenAPI 规范
   允许的扩展前缀，结构上完全合法；而 vendor 扩展的**存在性**不在 JSON Schema 的语义里。
3. **同族第二处定义**。生成器（W86 改动前位于 `generate.mjs:125`）另写了一份
   `if (!operation.operationId || !operation['x-required-permission'])` —— 与 `openapi-ops.mjs` 的判据
   **措辞不同、覆盖不同**（无状态检查、无键集闭世界、无孤儿检查），且**各自会独立漂移**。

**实测（W86 探针，非推断）**：往 `remote-knowledge.yaml` 加一个**无** `x-required-permission`
的 `patch` 操作，`SwaggerParser.validate` 通过（没拦），现有本地判据**根本不接触这份文档**
⇒ **本机门控全绿**。对照：同位置换成 `get` 同样不被任何本地判据看见。

| 注入（加到 `remote-knowledge.yaml`） | SwaggerParser | 现有本地判据 |
| --- | --- | --- |
| 一个**无** `x-required-permission` 的 `patch` 操作 | 通过（没拦） | **无人过问 —— 该文档不走进判据** |
| 同位置换 `get` | 通过（没拦） | 同上 |

⇒ 一份**承载鉴权语义**的契约文档，可以在本地门控与 CI 结构校验**双绿**的情况下新增一个
**不声明权限**的操作。

同族登记（W80–W85 已覆盖台账三层、门控接线与**文档内的操作层**，本条修的是**文档集本身**）：

| 轮次 | 「清单」 | 「清单的补集」 |
|---|---|---|
| W80 | 三个 `sectionRefs(...)` 手写字段 | `recommendation`/`closure`/`note`（17 条引用漏扫） |
| W81 | `CITATION_FIELDS` | 清单**之外**的新字段 |
| W82 | 条目的键集 | 信**封**（顶层）的键集 |
| W83 | `§` 一侧的触发规则 | id 一侧仍是**按处置**触发 |
| W84 | 逐条列举 + 两个 glob | 没有任何站点指向的测试文件 |
| W85 | `['get','post','put','delete']` 手写方法清单 | `patch` 等清单外的方法 |
| **W86** | **`apiDocument()` 硬编码的一份描述路径** | **同目录的第二份描述（权限与状态都不查）** |

## 决策

1. **文档集 MUST 由已跟踪路径派生**（W75／W84）。`openapiDocumentPaths(tracked)` 取
   `OPENAPI_DIR` 下扩展名 ∈ `OPENAPI_EXTENSIONS` 的在册路径。用 `.git/index` 而不是 `readdir`：
   「仓库里有哪些契约」这句话的输入就必须是**仓库**，工作区里未跟踪的散落文件既不参与、
   也不能掩盖覆盖关系。**闭世界只看名字**（目录 + 扩展名），不看内容。

2. **`OPENAPI_DOCUMENTS` 是逐文档声明的单一来源，逐字写死**。键 = 仓库相对路径；
   值 = `{status, implemented}`。`status` 是顶层 `x-implementation-status` 的期望值，
   `implemented` 是**那份文档自己**的已实现操作表（`remote-knowledge.yaml` 自述 `client-only`，
   它的表为空——把 `control.yaml` 的八条套上去只会得到八条孤儿声明）。原先硬编码在
   `apiDocument()` 里的字符串与全局的 `IMPLEMENTED_OPERATIONS` 都**搬进声明**，不再散落在执行体。

3. **文档集闭世界是双向的**，两条各自报具名：在册描述 MUST ⊆ 声明集（新增一份文档而忘了声明
   ⇒ 它无人校验）；声明集 MUST ⊆ 在册描述（文档改名/删除而声明留着 ⇒ 一条没有对象的覆盖）。
   单方向各挡一半，只有两条一起才等于「集合相等」。

4. **判据逐文档参数化，`declared` 是必需入参**。`operationErrors(document, declared)`：缺参或
   类型不对 ⇒ **具名失败**，MUST NOT 退回某个模块级默认值。理由（W75）：静默退回**另一份文档**的
   声明，会让**判据的输入不再等于被断言的对象**——而那条断言看上去仍然在跑。实测：用
   `control.yaml` 的八条去判 `remote-knowledge.yaml`，得到的是**八条孤儿声明**。

5. **权限规则只有一处定义**。`generate.mjs` 改为**调用** `operationErrors`，删掉复述的
   `x-required-permission` 判断，只保留它自己**真正需要**的约束（`operationId`——生成
   `*_PATH`／`*_METHOD` 常量要名字）。与仓库既有的 `TS_GENERATED_SOURCES` 处置同形
   （原注释：*A second copy here would be a list that can disagree with the first one in silence*）
   ⇒ **一处定义、两处使用**。

6. **非空洞**：文档集非空；每份文档至少暴露一个操作；且**至少一份**文档声明了非空 `implemented`
   ——否则「声明无孤儿」这一半在每份文档上**空转**（W84 的「空台账使两集不相交退化成恒真」同形）。

7. **不给两份描述加改动**。实测两份文档各自通过闭世界（键集闭合、逐操作带权限、状态与声明一致、
   各自零孤儿）⇒ 收紧后**报错集合不变**，基线保持全绿。

## 后果

- **正面**：`remote-knowledge.yaml` 从「本机无人读、CI 只做结构校验」变成「与 `control.yaml`
  走同一条判据」；「哪些文档被校验」的补集有了对立面（双向闭世界）；生成器与判据不再是两份会
  各自漂移的定义。
- **代价**：`OPENAPI_DOCUMENTS` 与 `OPENAPI_EXTENSIONS` 是**需要维护的手写声明**。这是**想要的**
  摩擦——新增一份描述或一种扩展名会被**具名拒绝**（安全方向），而不是被静默忽略。
  与 `HTTP_METHODS`／`PATH_ITEM_FIELDS`（ADR 0063）、`CAPABILITY_PACK_IDS`（ADR 0056 遗留 4）、
  `TEST_REGISTRATION_SITES`（ADR 0062）同类。
- **中性**：两份描述**一字未改**；`tests/contracts/schema.test.mjs` **24 → 29**。
- **假阳性方向已证**：一份声明完整的新描述、一份新增的**合法**操作（带权限声明、不在已实现表里）
  MUST NOT 红（反向注入 F8）——证明补的是「**覆盖**文档集」，不是「**只允许**一份描述」。

## 实施

- 模块：`scripts/contracts/openapi-ops.mjs`（新增 `OPENAPI_DIR`／`OPENAPI_EXTENSIONS`／
  `OPENAPI_DOCUMENTS`／`openapiDocumentPaths()`／`documentSetErrors()`；
  `operationErrors(document, declared)` 参数化；`operationShapeErrors()` 收缩为只管两张键集表——
  逐文档的**声明形状**搬进 `documentSetErrors`，那里才有「这是哪一份文档」这个上下文）。
- 接线：`validation.mjs` 的 `apiDocuments()` 遍历声明集逐份校验（顶层状态 + 操作集闭世界）；
  `apiDocument()` 保留为取 `control.yaml` 的薄封装。**无新增接线点**
  （`tests/contracts/schema.test.mjs` 已在 `engineering_checks` 登记）。
- 生成器：`generate.mjs` 在 `bundle` 之后调用 `operationErrors(remote, OPENAPI_DOCUMENTS[...].implemented)`；
  原先那一行复述权限规则的判断改为只判 `operationId`。
- 注入证据：`F0` + `F1`–`F10` + `T1`–`T2`，共 **21 / 21**（含 F7 缺参、F8／F9 两条反向、
  F10 扩展名层、T2 三条文本守卫）用**临时脚本**逐项开火并还原。

## 遗留

1. **判据只到「文档声明的操作键」**（延续 ADR 0063 遗留 1）：`x-implementation-status` 是**声明**，
   其真伪不在本 ADR 的范围。
2. **`OPENAPI_DOCUMENTS`／`OPENAPI_EXTENSIONS` 的维护摩擦是有意的**（决策 1／2）。新增描述或
   新扩展名必须显式更新；更新前会被**具名拒绝**（安全方向）。
3. **`SwaggerParser` 仍负责结构合法性**，本条只补**覆盖闭世界**；两者分工 MUST NOT 互相替代。
4. §56.2 归档边界的正式裁定仍未落地（已被 W78 起连续**八轮**登记）。
5. harness 轨道仍 `pending` 的 **H18 与 B07**、W76 台账 5 条 `decidable_in_repo`
   （`PD-09/22/24/31/34`）的推荐落地，均未动手。
