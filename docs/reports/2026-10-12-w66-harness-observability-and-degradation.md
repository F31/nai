# W66 harness 观测与逐层具名降级（V3.9 §47.1、§47.4；验收 H15/H17）

日期：2026-10-12。基线：V3.9（ADR 0048）。工作项：**W63**。验收：H15、H17（均属 `r1`，本轮**机制落地、状态保持 `pending`**）。
变更：`openspec/changes/w66-harness-observability-and-degradation/`。

## 一、判断

V3.9 §51.3 把 harness 的**基础纪律**留在首版门槛（R1）：装配与预算（H01、H02）、记忆读写（H03、H04）、
**观测入决策日志（H15）**、**逐层具名降级（H17）**、模式爬坡（H18）。上一轮收尾时做了一次实测盘点，
结论是 87 条验收里未完结 46 条，其中 R1 剩 14 条；在这 14 条里，**W63 是唯一「留在 R1 却完全未开始」的 harness 工作项**——
W56（H01/H02）与 W57（H03/H04）都已落下判定面，只有它没有。

这不是「缺能力」，而是**已设计但没有可判定的面**。§47.1 写了五类观测记录、§47.4 写了六行具名降级表、
INV-32 写了「不得静默降级为空上下文的自由发挥」，但实测 `grep -rn 'harness' crates/` 在**判定逻辑上零命中**。
后果很具体：今天要回答「这次调用有没有静默降级」，只能靠人读代码。
这正是 W32–W36 以来各轮「判定骨架」反复处理的那类缺口，本轮照同一套做法处理。

## 二、方法

1. **先读口径再动手**：§47.1（五类观测项）、§47.4（六行降级表）、§12.2（降级顺序四级）、
   INV-11（密钥只经引用）、INV-32（每层失败须具名降级）。
2. **先看有没有人做了一半**（W57 的教训）：`crates/nai-kernel/src/` 下已有 `evaluate/{agent,guard,memory,show,skill}.rs`
   与顶层 `bridge.rs`／`device.rs`／`realistic.rs`／`demo.rs`／`golden.rs`。结论是没有 harness 观测或降级的判定面，
   但**有两处近邻必须区分开**：` evaluate/memory.rs` 的头注释已引用 INV-32（「every layer failure is named」），
   `bridge.rs:330` 已有一个 `Degradation` 结构。
3. **决定落点并写下理由**：放 `evaluate/harness.rs`（不是新建顶层模块），**不复用** `bridge::Degradation`——
   后者只服务 §119 的平台中断（`bridge_lost`／`auto_reply_revoked`），与 harness 六层是两件事；
   把两者合并会让「这次降级属于哪一类」重新变得不可判定，而那正是本轮要消除的状态。

## 三、交付

### 3.1 观测判定（H15）

`ObservationLayer` 五类（装配／意图／记忆／工具／缓存）各带 `required_fields()`，**逐项照抄 §47.1 表格，不增不减**。
`evaluate_observation` 回答「这条记录**能不能**进日志、以什么形态进」：

- 必需项齐备（缺项 → `MissingRequiredField`；空值不算已记录 → `EmptyFieldValue`）；
- **值内**无凭据字面量（INV-11）→ 否则 `PlaintextCredential`，并把该值替换为 `[redacted]`；
- 观众原文与昵称已脱敏、且在 §5.8 保留期内 → 未脱敏 `ViewerTextUnmasked`（载荷里掩成 `[masked]`），
  超期 `ViewerTextOverRetention`（**丢弃**，不 merely 标记）。

### 3.2 降级判定（H17）

`FailureLayer` 六类各带互异的 `code()` 与 `preferred_level()`；`evaluate_degradation` 从首选级起沿
§12.2 四级（模板 → 已批准缓存 → 人工 → 静音待机）取**第一个可用**级，静音待机恒可用故必有落点：

| 失败层 | 错误码 | 首选级 | 拒绝调用 | 继续时丢弃 |
|---|---|---|---|---|
| 装配（不可裁剪段缺失） | `HARNESS_ASSEMBLY_FAILED` | 模板 | **是** | — |
| 记忆读失败 | `HARNESS_MEMORY_READ_FAILED` | 模板 | 否 | `memory` |
| 意图不可用／低置信 | `HARNESS_INTENT_UNAVAILABLE` | 人工 | 否 | `intent` |
| 工具失败 | `HARNESS_TOOL_FAILED` | 模板 | 否 | `tool_result` |
| 缓存不可用／失效 | `HARNESS_CACHE_UNAVAILABLE` | 人工 | 否 | `cache_entry` |
| MCP/A2A 对端不可用 | `HARNESS_PEER_UNAVAILABLE` | 人工 | 否 | `peer_capability`（**已有会话不受影响**） |

装配层的 `refuses_call` **不因落点变化而解除**——§47.4 的「拒绝调用」是绝对的，不会因为模板恰好不可用就转而继续生成。
`visible` 恒为真：**不存在不可见的降级**。

### 3.3 INV-32 的后半段

「不得静默降级为空上下文的自由发挥」如果只理解成「要有错误码」，是**空洞的**：一个实现完全可以给出错误码，
然后把空上下文丢给模型让它自由发挥。故 `assert_no_free_form` 做**跨字段**判定：
本次调用存在任一降级，且装配后上下文已失去策略或人设约束 ⇒ 判为具名违规，调用不得继续生成。

## 四、关键取舍

1. **放 `evaluate/` 而非新建顶层模块**——`evaluate/` 的模块契约就是纯逻辑判定，且 `memory.rs` 已引用 INV-32；
   放同处才不会出现两个都声称实现 INV-32 的模块。
2. **不复用 `bridge::Degradation`**——主体不同（平台中断 vs harness 六层），合并会让分类不可判定。
3. **凭据扫值、不扫字段名**——黑名单式实现会被「字段名含 `key` 但值干净」误伤，也被「字段名很干净但值里带 `sk-`」绕过。
4. **降级顺序做成「回退链」而不是「一一映射」**——§47.4 只给了每层的首选动作，没给「首选不可用怎么办」；
   按 §12.2 顺序向下取第一个可用级是唯一不自创规则的解法，且静音待机兜底保证必有落点。
5. **H15/H17 保持 `pending`**——判定面 ≠ 真实路径已接线。沿用 W57 对 H03/H04 的口径。
6. **不新增契约名**（能力目录零漂移），不改 `CapabilityView` 字段。
7. **不新增第二个门控入口**——测试进既有 `check --profile full`。
8. **补一条「钉住内容」的断言**（见第八节第 1 条）——这是本轮自己发现的守卫缺口。

## 五、验收侧

`tests/registry.yaml` 的 H15／H17 追加 `notes`，记录「机制已落地、验收仍 `pending`」。
**不改任何 `status`**：判定的是「记录能不能进日志」「失败落到哪一级」，而
**真正把记录追加进 `nai-storage` 的哈希链、把 `DegradeVerdict` 渲染进 `CapabilityView` 与界面，都属接线**。

⇒ 本轮**不宣称**「已进决策日志」。这不是保守措辞，是事实：哈希链的写入在 `crates/nai-storage/src/decision.rs`，
kernel 侧是纯逻辑、无 I/O，两者之间还没有线。

## 六、验证

### 6.1 门控（`cargo xtask check --profile full`）

**EXIT=0**。

| 项 | 结果 |
|---|---|
| 测试目标 | **88**（70 `Running` + 18 `Doc-tests`）= W65 的 87 **+1** = 新增 `tests/harness.rs` |
| 测试项 | **785 passed / 0 failed** = W65 的 761 **+24** = harness.rs 的 24 项 |
| `check-deps` | `dependency boundaries passed: 19 workspace packages` |
| 注册表 | `admission registry check passed (V3.9 acceptance + D01-D14 mapping)` |
| 前端门控 | 22 项通过 |

两个增量（+1 目标、+24 项）互相自洽，且「+24」恰好等于新测试数 ⇒ 未误改别处。
**数目标时先扣 cargo 的包装行**：按 `^     Running ` 数得 71，其中 1 行是 `Running …nai-xtask.exe check --profile full`
的包装行，真实 `Running` = **70**。

**本机遇到一次已知主机问题**：第 1 次整体门控在 `cargo test --workspace` 步骤报 **LNK1104**
（`nai-budget-core` / `nai-control-api` / `nai-engine`），即**批量链接争用**（历史登记过的同族问题）。
判据与处置沿用既有约定：**逐包能过 ⇒ 别改代码**——实测 `cargo test -p nai-budget-core --lib`、
`-p nai-control-api --lib` 均 EXIT=0（`nai-control-api` 第一次也失败、重跑即过），
故加 `CARGO_BUILD_JOBS=1` 后**第 2 次整体通过**。本轮**零 Rust 逻辑改动**以外的代码。

### 6.2 非空洞自检（四组注入，每组都确认会失败）

| 注入 | 预期 | 实测 |
|---|---|---|
| 凭据扫描改成**字段名黑名单** | 干净值被误伤 + 脏值被放过 | 3 项失败（含 `a_field_named_key_with_a_clean_value_passes`） |
| 所有失败**压到第一级** | 意图/缓存错落模板 | `intent_and_cache_land_on_human…` 失败 |
| 「**命名了就继续生成**」 | INV-32 后半段失效 | `a_degraded_call_without_its_policy_constraint_is_a_violation` 失败 |
| 悄悄**删掉 `crop_reason`** | 内容漂移无人发现 | `required_fields_match_the_baseline_table` + `a_complete_clean_record_is_loggable` 失败 |

第四条是本轮**补出来的**（见第八节第 1 条）：加它之前，删掉一项必需字段**没有任何测试会失败**。

### 6.3 OpenSpec 校验（两条口径都跑，互不替代）

| 口径 | 结果 |
|---|---|
| `validate w66 --strict`（变更层） | 一次通过 |
| `validate --all --strict`（**规格**） | **42 passed** |
| `validate --all --strict --archived`（**已归档变更**） | **64 passed** = W65 的 63 **+1** |

需求正文长度实测 **305 / 274 字符**，远低于 `--strict` 在 `specs/` 上的 500 上限（W56 曾在此翻车）。

### 6.4 契约零漂移（如实）

`cargo xtask schema --check` 在本机**跑不了**（`EBUSY`，主机限制）。本轮**未改动任何生成器输入**
（`schemas/catalog/capabilities.json`、`schemas/{config,plugin}/*.schema.json`）与 `crates/`、`packages/` 之外的
契约来源 ⇒ 结构性理由成立；并由 CI 的独立步骤（「Check generated contracts without modifying consumers」）兜底，
推送后核对。

## 七、未做与遗留

1. **runtime 接线**：把观测记录真正追加进决策日志哈希链；把降级渲染进 `CapabilityView` 与桌面界面。
2. **H18 爬坡门控**仍 `pending`——与 H15/H17 同属 W63 轨道的近邻，本轮未做。
3. **H05–H14、H16 仍属能力包**（意图、缓存、工具循环、MCP、A2A、黄金集），本轮不实现。
4. **三处阈值**（意图置信度、缓存相似度、上下文利用率）仍「待本场景回放实测」，MUST NOT 采外部观察值。
5. **A12 桌面壳宿主边界**、**O05 升级成本显示接入界面**——上一轮盘点里的下一批，本轮未动。
6. **`apps/` 的结构缺口仍在**：`platform-bridge`(C++)、`control-server`、`update-helper`、`remote-web` 均无；
   `crates/conformance` 未建为独立 crate。
7. **`cargo xtask schema --check` 在本机跑不了**（`EBUSY`，主机限制），依赖 CI 兜底。

## 八、方法论教训

1. **「完整性守卫」要问一句：它钉住的是条数还是内容？**
   本轮最初的四组守卫里，「五类齐全」只校验了条数与 `label()` 互异、以及各层字段集互不相同——
   **没有一条钉住字段表的实际内容**。注入测试（悄悄删掉 `crop_reason`）证明：删掉一项，全部测试照样绿。
   修法是新增 `required_fields_match_the_baseline_table`，把五类字段表**逐项写死**。
   ⇒ **只钉条数的守卫，对「内容被改小」是瞎的。**
2. **「有错误码」不等于「没有静默降级」**。INV-32 的后半段必须做成**跨字段**判定：
   降级 + 失去策略/人设约束 ⇒ 违规。只断言错误码的守卫会被「命名了就继续生成」的实现骗过。
3. **非空洞自检要挑「另一种合理实现」来注入**，不是随便改坏。
   本轮选的四个：字段名黑名单（另一种合理的凭据检查写法）、所有失败压到第一级（更简单的映射）、
   命名后继续生成（漏掉 INV-32 后半段）、悄悄删一项（内容漂移）。四组**每组都被抓住**，
   其中第一组被 3 项测试同时抓住。
4. **近邻结构要先区分再决定复用**。`bridge::Degradation` 与 `memory.rs` 的 INV-32 引用都是近邻，
   前者不能复用（主体不同）、后者是同一条不变量的前半段（应当同处）。判断依据是**主体**而不是**名字**。
5. **「机制已交付」与「验收已达成」必须分开记**。本轮与 W57 一致：判定面落地，状态仍 `pending`，
   并在 notes 里写清楚差在哪一段接线。这样后面的人看到 `pending` 不会误以为「还没开始」。
