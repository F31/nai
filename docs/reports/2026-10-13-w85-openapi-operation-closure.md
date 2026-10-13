# W85 · OpenAPI 操作集的闭世界（判据 MUST 遍历文档自己的操作键）

- 日期：2026-10-13
- 基线：V3.10（`docs/直播数字人 + 智能体系统技术方案 V3.10.md`）
- 裁定：[ADR 0063](../adr/0063-openapi-operation-closure.md)
- 变更：`openspec/changes/archive/2026-10-13-w85-openapi-operation-closure/`

## 一、判断与范围

`validation.mjs` 的 `apiDocument()` 对 `schemas/openapi/control.yaml` 做两道**逐操作**检查——
**权限声明**（`x-required-permission`）与**实现状态**（`x-implementation-status`）——但
「哪些操作要被查」由一张**手写的方法清单**决定：

```js
const implemented = new Set(['get /health', 'get /runtime/status', …]);   // 八条，手写
for (const [path, item] of Object.entries(document.paths)) {
  for (const method of ['get', 'post', 'put', 'delete']) {                // ← 方法清单，手写
    if (item[method] && !item[method]['x-required-permission']) throw …
    const expected = implemented.has(`${method} ${path}`) ? 'implemented-loopback' : undefined;
    if (item[method]['x-implementation-status'] !== expected) throw …
  }
}
```

**手写清单的盲区恰好是它的补集**：`patch`／`head`／`options`／`trace` 都是 **OpenAPI 3.0 的合法
方法键**，全部落在 `['get','post','put','delete']` 之外。这是 W80–W84 同一族洞的**第六处**，
而它落在一份**承载鉴权语义**的契约文档上。

**范围**：新增 `scripts/contracts/openapi-ops.mjs`、改 `scripts/contracts/validation.mjs`、
改 `tests/contracts/schema.test.mjs`；
**不改 `schemas/openapi/control.yaml`、不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边**。

## 二、审实

**审实 1（洞是真的）——探针实测，非推断。** 三支探针（`.w85_probe*.mjs`，归档前已删除）：

1. **当前状态**：文档里 **42 个操作**，出现的方法只有 `get`／`post`／`put`／`delete`
   ⇒ 手写循环「会检查的」= 42、「会跳过的」= 0；`implemented` 的八条**零孤儿**（当前无症状）。
   ⇒ 洞是**潜伏**的，不是正在开火的。

2. **注入对照**（往 `paths` 里加一个操作 → `SwaggerParser` 校验 → 再跑那段循环的**逐字逻辑**）：

   | 注入 | SwaggerParser | 手写循环 |
   | --- | --- | --- |
   | `patch` 操作（**无** `x-required-permission`、**无** `x-implementation-status`） | 通过（没拦） | **0 报错 ⇒ 静默通过** |
   | 同位置换成 `get` 操作 | 通过（没拦） | 1 报错（`missing permission: get /w85-probe`） |

   ⇒ 一个 HTTP 方法只要落在清单之外，它的权限声明与实现状态**都不再被问**。
   **`SwaggerParser` 两种情况都放行**——它管结构合法性，不管覆盖，两者不可互相替代。

3. **键集**：文档的 path item 键只有 `{get, post, put, delete, parameters}`，
   path item 层**没有** `x-` 扩展键 ⇒ 收紧后不会对现有文档误报（假阳性方向已预先核实）。

**审实 2（同族第二处，潜伏）。** `implemented` 的八条**只被用来反查文档**，从未被要求**指回文档里
真实存在的操作**。把其中一条改成不存在的路径，循环照旧全绿——一条**只有声明、没有对象**的契约。
当前实测孤儿数 = **0**，但**没有任何守卫**会拦住第一个孤儿（W75 的老话：**「已声明」≠「存在」**）。

**审实 3（同族登记）。** 这是「按清单做」的第六处：

| 轮次 | 「清单」 | 「清单的补集」 |
|---|---|---|
| W80 | 三个 `sectionRefs(...)` 手写字段 | `recommendation`/`closure`/`note`（17 条引用漏扫） |
| W81 | `CITATION_FIELDS` | 清单**之外**的新字段（`rationale: "§99.9"` 静默） |
| W82 | 条目的键集 | 信**封**（顶层）的键集 |
| W83 | `§` 一侧的触发规则 | id 一侧仍是**按处置**触发 |
| W84 | 逐条列举 + 两个 glob | 没有任何站点指向的测试文件 |
| **W85** | **`['get','post','put','delete']` 手写方法清单** | **`patch` 等清单外的方法（权限与状态都不查）** |

## 三、裁定

1. **判据遍历文档自己的操作键**（W83 的应用）：沿 `HTTP_METHODS` 取操作，MUST NOT 遍历手写子集
   ——「新方法」与「老方法」走同一条路径。
2. **`HTTP_METHODS` 逐字写死**（八个，OpenAPI 3.0 全表），MUST NOT 由「文档里现有方法」派生
   （否则「现有方法 == 清单」是**恒真式**）。仓库此刻只用四个，但清单 MUST NOT 随之收缩。
3. **`PATH_ITEM_FIELDS` 逐字写死**（五个），path item 的**每个键** MUST ∈ 两者之一
   （`x-` 扩展前缀按规范放行）⇒ `patch` 被**检查**、`gets:` 这类笔误被**具名拒绝**。
4. **相邻层不相交**（W82）：`HTTP_METHODS ∩ PATH_ITEM_FIELDS == ∅`。
5. **声明无孤儿**（W75）：`IMPLEMENTED_OPERATIONS` 的每一项 MUST 指回文档里真实存在、
   且状态为 `IMPLEMENTED_STATUS` 的操作。
6. **非空洞**：文档 MUST 至少暴露一个操作（同 W84 的「覆盖集非空」）。
7. **只把 `operationErrors` 接进 `apiDocument()`**：判据是纯函数、入参是文档对象，与 I/O 分离。
8. **不给 `control.yaml` 加改动**：实测 42 个操作全部带权限声明、状态全部与声明一致、
   八条 `implemented` 零孤儿 ⇒ 收紧后**报错集合不变**。

## 四、实现

- 新建 `scripts/contracts/openapi-ops.mjs`：
  - `HTTP_METHODS`（八，逐字写死）／`PATH_ITEM_FIELDS`（五，逐字写死）／
    `EXTENSION_PREFIX`／`IMPLEMENTED_OPERATIONS`（八条，逐字写死，**单一来源**）／`IMPLEMENTED_STATUS`；
  - `operations(document)`：文档里被识别的全部操作；
  - `operationShapeErrors(...)`：三张表自洽（两集不相交、各集非空、声明项形状合法且不重复）；
  - `operationErrors(document)`：**主判据**——键集闭世界 + 逐操作权限/状态 + 非空洞 + 声明无孤儿。
- 改 `scripts/contracts/validation.mjs`：`apiDocument()` 删掉手写的 `implemented` 集与方法清单，
  改为 `operationErrors(document)` 非空即抛。**无新增接线点**（`tests/contracts/schema.test.mjs`
  已在 `engineering_checks` 登记）。

## 五、非空洞注入（F0 基线 + T1–T8 表级）

数据级由 `tests/contracts/schema.test.mjs` 自身承载（5 条注入用例）；表级用**临时模块副本**
（`openapi-ops.mjs` **无 import**，副本可独立加载），逐项开火并还原：

| 组 | 注入 | 期望 | 结果 |
| --- | --- | --- | --- |
| F0 | 真实文档 | 0 错误 | FIRE（`errors=0`） |
| F0b | 真实文档 + 探针 `patch` | 具名 `patch /w85-probe` | FIRE |
| **T1** | `HTTP_METHODS` 收缩回旧四方法 | 逐操作检查**不再覆盖** `patch` | **FIRE**（只剩键集闭世界在挡） |
| T2 | `HTTP_METHODS` 删 `get` | `get /health` 变孤儿声明 | FIRE（23 错） |
| T3a | 声明表清空 | 形状层报空 | FIRE |
| T3b | 声明表清空 | 8 条已实现变「未声明却带状态」 | FIRE |
| T4 | 字段集清空 | `parameters` 被具名拒绝 | FIRE |
| T5 | 字段集掺 `get` | 两集重叠被具名 | FIRE |
| T6 | 声明表掺幽灵 | `no such operation` | FIRE |
| **T7** | 声明顺序打乱（**反向**） | **不红** | FIRE |
| **T8** | 还原后基线（**反向**） | **仍绿** | FIRE |

**合计 11 / 11**，源文件未被改动（脚本末尾核对 `restored === original`）。

**T1 是本轮最有价值的一项**：它证明「方法集」是判据的**输入**而不是常量——方法集一收缩，
`patch` 就重新落进补集、逐操作检查失效。**T1 同时也暴露两条规则是互补的**：`patch` 仍被
**键集闭世界**那条独立守卫抓住（报 `neither a recognised HTTP method nor a path-item field`）。
⇒ 「补」不是一条规则，而是**两条腿**：一条保证「每个操作被检查」，一条保证「没有键能躲开检查」。

## 六、验证

### 6.1 定向测试

```
node --test tests/contracts/schema.test.mjs   →  24/24 pass   （17 → 24）
node .w85_inject.mjs                          →  F0 + T1–T8 共 11 项，全 FIRE
```

### 6.2 门控

```
cargo xtask check --profile full   →  EXIT=0   （4m46s）
```

实测 **91 个测试目标（73 `Running` + 18 `Doc-tests`）、860 passed / 0 failed**，
日志内 `error[` / `LNK1104` / `out of memory` 命中 **0** 次。
口径以 `grep -c '^test result:' <门控日志>` 定准（**每个目标恰好一行，没有自由度**）；
`^     Running ` **74** 行 = **1 包装行**（cargo 自己打印的
`Running \`E:/nai-build-target\debug\nai-xtask.exe check --profile full\``，**不是测试目标**）+ 73 真实目标。
本轮**零 Rust 改动**，`passed` 与 W71–W84 逐项相同 ⇒「数字不变」正是「未误改别处」的旁证。
日志里可见本轮的新用例在门控内执行：
`ok 13 - the real API description passes the operation-set closure`、
`ok 14 - an operation under a method nobody listed is judged, not skipped`、
`ok 18 - an empty or unreadable API description is reported, not silently closed`
（Node 用例**不**计入 91/860，那组数只来自 `cargo test`）。

### 6.3 两条 validate

```
openspec validate --all --strict              →  43 passed / 0 failed   （规格数不变：新增的是既有能力的 Requirement）
openspec validate --all --strict --archived   →  83 passed / 0 failed   （82 → 83）
```

另对**活动变更**单独跑 `openspec validate w85-openapi-operation-closure --strict` ⇒ `valid`。
归档时 CLI 对 `proposal.md` 的 `## Why` 段给了一条**非阻塞**警告
（`Why section should not exceed 1000 characters`）——**本轮这条警告是真的**：
实测 `## Why` 段 **1133 字符**（阈值 1000）。归档**后**的 `--all --strict --archived` 仍
**0 failed** ⇒ 该警告不使严格校验失败，如实登记。
（对照：W84 的 Why 段 521 字符，**无**此警告——**两轮性状不同，故逐轮实测、不照抄**。）

## 七、CI 复核

**run `38343617885`（`head_sha = 41bda076e6…`，与本轮推送 SHA 一致）= `success`**（run_number **100**，双平台绿）。

观测方式沿用既有纪律：**单次低频查询**（**不轮询**），每次带**唯一查询参数**破坏代理缓存、
`Cache-Control: no-cache`，并**回读 `head_sha` 与本次推送比对**；`curl` 带 `--tlsv1.2 --tls-max 1.2`
（schannel 与 TLS 1.3 握手失败，W77 实测）。本轮共 4 次查询（配额起点 53/60）：

1. 推送后首次：run 100 `in_progress`，**顺带证实 run 99（`195d5fa`，W84 回填）亦 `success`**；
2. 等待 8 分钟后：仍 `in_progress`（父 run 的 `updated_at` 未变）；
3. 为判断「是否真在推进」查了一次 `/jobs`：**`ubuntu-24.04` 已 `completed success`（25/26 步），
   `windows-latest` `in_progress`（13/25 步，正在 `Scheduling and shared budget ledger`）**
   ⇒ 正常推进，只是 windows 侧更慢（**这是「查一次进度」而不是「轮询」**）；
4. 再等 7 分钟后单次查询 `/jobs` ⇒ **两个 job 均 `completed success`（各 26 步，`failed: []`）**。

「三件齐」满足：① 唯一查询参数 `&cb=<ns>`；② `Cache-Control: no-cache`；
③ 回读 `head_sha = 41bda076e6` == 本次推送 SHA。**本机门控绿在此之上叠加了独立环境的复核**。

## 八、遗留

1. **判据只到「文档声明的操作键」**：它不检查「实现体是否真的实现了该操作」。
   `x-implementation-status` 是**声明**，其真伪不在本 ADR 的范围（ADR 0063 遗留 1）。
2. **`HTTP_METHODS`／`PATH_ITEM_FIELDS`／`IMPLEMENTED_OPERATIONS` 三张手写表的维护摩擦是有意的**
   （ADR 0063 决策 2／3／5）。更新前它们会被**具名拒绝**（安全方向），而不是被静默接受。
3. **`SwaggerParser` 仍负责结构合法性**，本条只补**覆盖闭世界**；两者分工 MUST NOT 互相替代。
4. **§56.2 归档边界的正式裁定仍未写死**（W78 §八第 8 / W79 §八第 6 / W80 §八第 4 / W81 §八第 5 /
   W82 §八第 5 / W83 §八第 5 / W84 §八第 3 未变）：需一次**升基线轮次**把它写进基线正文。
5. harness 轨道 **H18 与 B07** 仍 `pending`；W76 台账的 5 条 `decidable_in_repo`
   （`PD-09/22/24/31/34`）仍只登记了推荐——**推荐的落地仍未发生**。
6. `CAPABILITY_PACK_IDS` 仍手写（ADR 0056 遗留 4）；`TEST_REGISTRATION_SITES`（ADR 0062）
   与本轮新增的两张表**同类**——都是需要维护、且**有意**维护的手写声明。
7. `cargo xtask schema --check` 本机仍跑不了（宿主对 node 派生任何子进程的 `EBUSY`）。
   本轮未改任何 schema 与生成器输入 ⇒ 零漂移由结构性理由 + CI 独立步骤兜底，**未写成「已通过」**。

## 九、方法论教训

- **「清单挡得住清单内的错，挡不住清单外的存在」**——这是六轮同一族的同一句话。
  W85 的新意在于清单的**载体**：前五轮是数据字段、条目键、信封键、触发条件、门控接线，
  这一轮是**一份契约文档里的 HTTP 方法**。⇒ 洞的形状不依赖于它藏在哪种工件里。
- **「两条腿」比「一条腿」稳**：T1 意外证明键集闭世界与逐操作检查是**互补**的——
  收缩方法集会让前者失效，但后者仍在挡。写守卫时与其加固一条，不如问「还有哪条独立的断言
  覆盖同一个对象」。
- **潜伏 ≠ 不存在**：本轮的洞当前**不产生任何错误**（42 个操作全在清单内）。
  「今天不红」不是「明天不会红」的证据——**判据的价值在于它对未来的编辑说什么**。
- **注入要打到判据的输入上**：T1 打的是 `HTTP_METHODS`（判据的输入），不是文档（判据的对象）。
  只按文档构造样本，会漏掉「输入本身被改坏」这一类失效——而这正是本轮洞的成因。
