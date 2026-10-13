# W86 · OpenAPI 文档集的闭世界（操作集判据 MUST 覆盖仓库里每一份描述）

- 日期：2026-10-13
- 基线：V3.10（`docs/直播数字人 + 智能体系统技术方案 V3.10.md`）
- 裁定：[ADR 0064](../adr/0064-openapi-document-set-closure.md)
- 变更：`openspec/changes/archive/2026-10-13-w86-openapi-document-set-closure/`

## 一、判断与范围

W85 把「**哪些操作**被逐项校验」从一张手写方法清单改成**遍历文档自己的操作键**。但
「**哪些文档**被校验」仍然是一张手写清单——`validation.mjs` 的 `apiDocument()` 把描述路径
**硬编码**：

```js
const document = await SwaggerParser.validate('schemas/openapi/control.yaml');   // ← 硬编码
const errors = operationErrors(document);
const implemented = new Set([…]);   // 八条，且只属于这一份文档
```

同目录的 `remote-knowledge.yaml` 是这份清单的**补集**，而且三件事叠在一起：

1. **本地门控从不读它**——`tests/contracts/schema.test.mjs` 一次都不提它；唯一读它的
   `generate.mjs` 只在 **CI-only** 的 `cargo xtask schema --check` 里跑，而本机对该步 `EBUSY`
   （宿主禁止 node 派生任何子进程）⇒ **本机对它零校验**；
2. **CI 也只做结构校验**——`SwaggerParser` 对「操作不声明权限」放行（`x-` 是规范允许的扩展前缀，
   结构上完全合法）；
3. **同一件事的第二处定义**——生成器（W86 改动前位于 `generate.mjs:125`）另写了一份
   `if (!operation.operationId || !operation['x-required-permission'])`，措辞、覆盖范围都与
   `openapi-ops.mjs` 的判据不同，**各自会独立漂移**。

这是 W80–W85 同一族洞的**第七处**，而它落在一份**承载鉴权语义**的契约文档上。

**范围**：重构 `scripts/contracts/openapi-ops.mjs`、改 `scripts/contracts/validation.mjs`、
改 `scripts/contracts/generate.mjs`、改 `tests/contracts/schema.test.mjs`；
**不改 `schemas/openapi/control.yaml`、不改 `schemas/openapi/remote-knowledge.yaml`、不改基线正文、
不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边**。

## 二、审实

**审实 1（洞是真的）——探针实测，非推断。** 两份真实文档经 `SwaggerParser` 解析后逐项开火：

| 注入（加到 `remote-knowledge.yaml`） | SwaggerParser | 现有本地判据 |
| --- | --- | --- |
| 一个**无** `x-required-permission` 的 `patch` 操作 | **通过（没拦）** | **无人过问——该文档不走进判据** |
| 同位置换 `get` | 通过（没拦） | 同上 |

⇒ 一份承载鉴权语义的契约文档，可以在**本地门控绿 + CI 结构校验绿**的情况下新增一个
不声明权限的操作。

**审实 2（同族第二处）——两份定义并存。** `generate.mjs` 与 `openapi-ops.mjs` 各有一份
「remote 的操作必须声明权限」的判断，而前者的覆盖更窄（无状态、无键集闭世界、无孤儿检查），
且**只在 CI 跑得到**。仓库既有的处置口径（`TS_GENERATED_SOURCES` 的注释原文）是：
*A second copy here would be a list that can disagree with the first one in silence.*

**审实 3（「用错文档」是有害的，可实测）。** 把 `control.yaml` 的八条已实现声明套到
`remote-knowledge.yaml` 上，得到的是 **八条孤儿声明**（`no such operation`）——「已实现的是哪些
操作」是**每一份文档各自的属性**，不是一张全局表。

**审实 4（同族登记）。** 这是「按清单做」的第七处，清单的载体是**契约文档集合本身**：

| 轮次 | 「清单」 | 「清单的补集」 |
|---|---|---|
| W80 | 三个 `sectionRefs(...)` 手写字段 | `recommendation`/`closure`/`note`（17 条引用漏扫） |
| W81 | `CITATION_FIELDS` | 清单**之外**的新字段 |
| W82 | 条目的键集 | 信**封**（顶层）的键集 |
| W83 | `§` 一侧的触发规则 | id 一侧仍是**按处置**触发 |
| W84 | 逐条列举 + 两个 glob | 没有任何站点指向的测试文件 |
| W85 | `['get','post','put','delete']` 手写方法清单 | `patch` 等清单外的方法 |
| **W86** | **`apiDocument()` 硬编码的一份描述路径** | **同目录的第二份描述（权限与状态都不查）** |

## 三、裁定

1. **文档集由已跟踪路径派生**（W75／W84）：`OPENAPI_DIR` 下扩展名 ∈ `OPENAPI_EXTENSIONS` 的
   **在册**路径；闭世界**只看名字**（目录 + 扩展名），MUST NOT 手写「有哪些文档」。
2. **`OPENAPI_DOCUMENTS` 是逐文档声明的单一来源，逐字写死**（`status` + `implemented`）。
3. **文档集闭世界是双向的**：在册描述 ⊆ 声明集（新增文档忘了声明 ⇒ 无人校验）；
   声明集 ⊆ 在册描述（文档改名/删除而声明留着 ⇒ 没有对象的覆盖）。
4. **判据逐文档参数化，`declared` 是必需入参**：缺参／类型不对 ⇒ 具名失败，MUST NOT 退回
   另一份文档的默认值。
5. **权限规则只有一处定义**：`generate.mjs` 改为**调用**判据，只保留它自己需要的 `operationId`。
6. **非空洞**：文档集非空、每份文档至少一个操作、且**至少一份**文档声明了非空 `implemented`
   ——否则「声明无孤儿」在每份文档上**空转**。
7. **不给两份描述加改动**：实测各自通过闭世界 ⇒ 收紧后**报错集合不变**。

## 四、实现

- `scripts/contracts/openapi-ops.mjs`：
  - 新增 `OPENAPI_DIR`（`schemas/openapi`）／`OPENAPI_EXTENSIONS`（`['.yaml','.yml']`）；
  - 新增 `OPENAPI_DOCUMENTS`（两份文档各一条 `{status, implemented}`，**单一来源**）；
  - 新增 `openapiDocumentPaths(tracked)`（纯函数；入参不是 `Set` 返回 `null`——「没给输入」与
    「输入是空集」是两件事）；
  - 新增 `documentSetErrors(tracked, documents)`（双向闭世界 + 逐文档声明形状 + 非空洞）；
  - `operationErrors(document, declared)` **参数化**，`declared` 必需；
  - `operationShapeErrors(methods, fields)` 收缩为只管两张键集表（逐文档的声明形状搬进
    `documentSetErrors`，那里才有「这是哪一份文档」这个上下文）。
- `scripts/contracts/validation.mjs`：`apiDocuments()` 遍历声明集逐份校验
  （顶层状态与声明比对 + 操作集闭世界）；`apiDocument()` 保留为取 `control.yaml` 的薄封装。
  **无新增接线点**（`tests/contracts/schema.test.mjs` 已在 `engineering_checks` 登记）。
- `scripts/contracts/generate.mjs`：`bundle` 之后调用
  `operationErrors(remote, OPENAPI_DOCUMENTS['schemas/openapi/remote-knowledge.yaml'].implemented)`；
  原先那一行复述权限规则的判断改为只判 `operationId`。

## 五、非空洞注入（F0 + F1–F10 + T1–T2）

用**临时脚本**（`.w86_inject.mjs`，归档前已删除）逐项开火：

| 组 | 注入 | 期望 | 结果 |
| --- | --- | --- | --- |
| F0 | 真实文档集（派生 == 2、`apiDocuments()` 覆盖两份） | 0 错误 | FIRE |
| **F1a** | remote 的操作数 | 6（判据的输入非空） | FIRE |
| **F1b** | remote + 无权限 `patch` | 具名 `patch /w86-probe` | FIRE |
| **F1c** | 对照组：同一注入过 `SwaggerParser` | 放行（没拦） | FIRE |
| F2 | 在册描述**未声明**（删掉 remote 的声明） | 具名 + `nobody declares` | FIRE |
| F3 | **陈旧声明**（指向 `gone.yaml`） | 具名 + `no document` | FIRE |
| F4 | 声明集清空 | 具名 | FIRE |
| F5 | 无任何文档声明已实现 | `vacuously true on every document` | FIRE |
| F6 | 缺 `tracked` 输入 | 具名，MUST NOT 当成「没有文档」 | FIRE |
| **F7a** | 缺 `declared` 入参 | 具名 `own implemented-declaration array` | FIRE |
| **F7b** | `declared` 类型不对 | 同上（不静默降级） | FIRE |
| **F7c** | **用错文档**的声明（control 的八条判 remote） | **八条孤儿** | FIRE |
| **F8** | 反向：新增**合法**操作 | **不红** | FIRE |
| **F9** | 反向：声明顺序打乱 | **不红** | FIRE |
| F10 | 扩展名层（只认 `.yaml`/`.yml` 且限定目录） | 只看名字 | FIRE |
| **T1** | 判据的**输入**：声明集缩回一份 | remote 逃逸被指名 | FIRE |
| **T2a** | 生成器**代码里**（整行注释不算，W74）不再有第二份权限规则 | 无命中 | FIRE |
| T2b | 生成器调用 `operationErrors` | 命中 | FIRE |
| T2c | 判据不再硬编码 `control.yaml` 路径 | 无命中 | FIRE |

**合计 21 / 21**，源文件未被改动（`git status` 除预期改动外干净）。

**T1 与 F7 是本轮最有价值的两项**：T1 打的是**判据的输入**（声明集本身）——一收缩，
第二份文档就重新落进补集，这证明「文档集」不是常量而是判据的入参；F7 证明「忘记传参」
会**具名失败**而不是退回一份默认声明（否则判据的输入不再等于被断言的对象）。

## 六、验证

### 6.1 定向测试

```
node --test tests/contracts/schema.test.mjs   →  29/29 pass   （24 → 29）
node .w86_inject.mjs                          →  F0 + F1–F10 + T1–T2 共 21 项，全 FIRE
```

### 6.2 门控

```
cargo xtask check --profile full   →  EXIT=0   （4m16s）
```

实测 **91 个测试目标（73 `Running` + 18 `Doc-tests`）、860 passed / 0 failed**，
日志内 `error[` / `LNK1104` / `out of memory` 命中 **0** 次。
口径以 `grep -c '^test result:' <门控日志>` 定准（**每个目标恰好一行，没有自由度**）；
`^     Running ` **74** 行 = **1 包装行**（cargo 自己打印的
`Running \`E:/nai-build-target\debug\nai-xtask.exe check --profile full\``，**不是测试目标**）+ 73 真实目标。
本轮**零 Rust 改动**，`passed` 与 W71–W85 逐项相同 ⇒「数字不变」正是「未误改别处」的旁证。
日志里可见本轮的新用例在门控内执行：
`ok 20 - every tracked OpenAPI description is declared, and every declaration has a description`、
`ok 21 - the second description is judged by the same closure as the first`、
`ok 22 - a per-document declaration must be injected, not defaulted`、
`ok 23 - the declaration shape of every document is checked and names the document`、
`ok 24 - the generator reuses the operation closure instead of restating it`
（Node 用例**不**计入 91/860，那组数只来自 `cargo test`）。

### 6.3 两条 validate

```
openspec validate --all --strict              →  43 passed / 0 failed   （规格数不变：新增的是既有能力的 Requirement）
openspec validate --all --strict --archived   →  84 passed / 0 failed   （83 → 84）
```

另对**活动变更**单独跑 `openspec validate w86-openapi-document-set-closure --strict` ⇒ `valid`。
归档时 CLI 对 `proposal.md` 的 `## Why` 段**未**给超长警告：实测 `## Why` 段 **999 字符** <
阈值 1000（W85 是 1133 字符 ⇒ **有**警告）。**该警告按轮而定、MUST 现测、MUST NOT 照抄上一轮。**

## 七、CI 复核

（待回填）

## 八、遗留

1. **判据只到「文档声明的操作键」**（延续 ADR 0063 遗留 1）：`x-implementation-status` 是**声明**，
   其真伪不在本 ADR 的范围。
2. **`OPENAPI_DOCUMENTS`／`OPENAPI_EXTENSIONS` 的维护摩擦是有意的**（ADR 0064 决策 1／2）。
   新增描述或新扩展名会被**具名拒绝**（安全方向），而不是被静默忽略。与 `HTTP_METHODS`／
   `PATH_ITEM_FIELDS`（ADR 0063）、`CAPABILITY_PACK_IDS`（ADR 0056 遗留 4）、
   `TEST_REGISTRATION_SITES`（ADR 0062）同类。
3. **`SwaggerParser` 仍负责结构合法性**，本条只补**覆盖闭世界**；两者分工 MUST NOT 互相替代。
4. **§56.2 归档边界的正式裁定仍未写死**（W78 §八第 8 / W79 §八第 6 / W80 §八第 4 / W81 §八第 5 /
   W82 §八第 5 / W83 §八第 5 / W84 §八第 3 / W85 §八第 4 未变，**连八轮**）：需一次**升基线轮次**
   把它写进基线正文。
5. harness 轨道 **H18 与 B07** 仍 `pending`；W76 台账的 5 条 `decidable_in_repo`
   （`PD-09/22/24/31/34`）仍只登记了推荐——**推荐的落地仍未发生**。
6. `cargo xtask schema --check` 本机仍跑不了（宿主对 node 派生任何子进程的 `EBUSY`）。
   本轮改了 `generate.mjs`（它只在该步与 CI 里跑）⇒ 该改动**本机无法直接执行验证**，
   只做了 `node --check` 语法检查 + 判据侧的功能等价验证；**结构合法性由 CI 的
   `Check generated contracts` 步骤独立复核**，**未写成「本机已通过」**。

## 九、方法论教训

- **「清单挡得住清单内的错，挡不住清单外的存在」——七轮同一句话，载体越来越"外层"。**
  W80–W82 是**数据**（字段、条目键、信封键）、W83 是**触发条件**、W84 是**门控接线**、
  W85 是**文档内的操作**，这一轮是**文档集合本身**。⇒ 洞的形状不依赖于它藏在哪种工件里，
  而**每修一层，边界只是移到新的一层**（W81 的「覆盖有三层」已经说了这件事）。
- **「同一件事的第二处定义」比「没有定义」更坏一点。** 因为它让「已经检查过了」这句话
  在**两个地方各成立一次**，而两处会以不同速度漂移。⇒ 找到第二处时，处置不是「两边都加强」，
  而是**让它调用第一处**（与 `TS_GENERATED_SOURCES` 同形：一处定义、两处使用）。
- **「没给输入」与「输入是空集」MUST 分开。** `openapiDocumentPaths(non-Set)` 返回 `null`
  而不是 `[]`：后者会让「仓库里没有文档」与「忘记传参」长得一样，而前者是**恒真**。
  （与 W70 的「没看到」vs「出错了」在类型上分开同源。）
- **反向注入要与正向**成对**：F8（合法新操作不红）证明补的是「覆盖」不是「只许一份」；
  F9（顺序无关）证明判据没把排序当成语义。**只有正向注入时，「太紧」与「正确」分不开。**
- **注释里的字面量 MUST NOT 算作代码**（W74 的第二次适用）：T2a 的守卫一开始就被我自己写的
  注释绊倒——那条注释里**逐字引用了**旧的那半句判断。守卫必须先按语言去掉整行注释，
  否则它惩罚的是**解释缺陷**这个行为本身。
