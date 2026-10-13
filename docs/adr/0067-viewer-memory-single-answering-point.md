# ADR 0067：观众长期记忆的「默认关闭」收成单一来源，并补上写入面的执行体（PD-31 结清）

- 状态：接受
- 日期：2026-10-13
- 相关：[ADR 0054](0054-pending-decisions-ledger.md)（台账的处置枚举与遗留 3；本 ADR 为**第二次**判决
  追加勘误）、[ADR 0066](0066-decided-in-repo-landing.md)（第六态 `decided_in_repo` 与 `landing`；
  本 ADR 是它遗留 1 中 `PD-31` 那半的裁定）、[ADR 0053](0053-repository-scoped-evidence.md)、
  `crates/nai-kernel/src/evaluate/memory.rs`、`crates/nai-kernel/tests/memory.rs`、
  `tests/pending-decisions.yaml`、`scripts/contracts/pending-decisions.mjs`、
  `schemas/config/runtime.schema.json`、`apps/engine/src/pipeline.rs`、
  `docs/reports/2026-10-11-w57-memory-read-write-surface.md`、
  `docs/reports/2026-10-13-w89-viewer-memory-single-answering-point.md`、
  `openspec/changes/archive/2026-10-13-w89-viewer-memory-default-off-single-home/`

## 背景

W88 报告 §八第 2 条登记了一处**张力**：台账 `PD-31`（*观众长期记忆的字段集与保留期*）的推荐写着
「R1 内判为『**空集 + 默认关闭**』」，而 W57 已交付的 `memory.rs` 里
`MemoryPolicy::default().allowed_viewer_fields` 是 **非空**（`["preference", "answered_topic"]`）。
W88 明确写了「**本轮未裁定**（不是 PD-09 的范围），下一轮核实后裁定」。本轮（**W89**）按此核实。

## 证据（本轮实测，先读后判）

### 1. 三处载体，只有一处有执行体

「观众长期记忆默认关闭」在仓库里被写了三遍：

| 载体 | 形态 | 执行体（实测） |
| --- | --- | --- |
| 配置面 | `schemas/config/runtime.schema.json` 的 `session_defaults.viewer_memory_enabled`（`additionalProperties:false`，且 **`required` 明列**） | **0 处** |
| 判定面 | `MemoryPolicy.viewer_memory_enabled` | 读取面 `read_records` ✓；请求准入 `viewer_record_allowed` ✓（**无生产调用方**）；写入面 `admit_write` **✗** |
| 台账 | `PD-31` 的推荐「空集 + 默认关闭」 | 「空集」与白名单**指代不同对象** |

配置面的零执行体是**可核实的**：全仓**非测试**代码里 `composition.config()["session_defaults"]`
**只被读一次**（`apps/engine/src/pipeline.rs:874`），读的是**同层的** `allow_auto_without_platform_grant`。
⇒ 同层字段「一个有主、一个无主」，这个**对照本身**就是判据（「已声明 ≠ 已执行」，W74 起）。

### 2. 写入面确实空着（改前实测）

`admit_write` 的入参只有 `&MemoryCandidate`——**没有策略**，因此它不检查能力开关。
实测（真跑改前那一行）：一条 `kind = "viewer"`、来源为 `approved_played`、作用域/来源标识/时间齐备的候选，
在 `MemoryPolicy::default()`（`viewer_memory_enabled == false`）下 **返回 `Ok(())`**。
⇒ 「读取被拒、写入却被接受」的半覆盖。

### 3. 「空集」指的是另一个对象

推荐的理由是「§5.5 的默认关闭**使字段集没有对象**」——**没有任何记录实例**，因此
**运行时被写入的字段值集合**为空集。而 `allowed_viewer_fields` 是**启用后才生效的允许字段白名单**；
W57 报告 §七明写「**阈值不定夺**：`MemoryPolicy` 的既有默认（内容长度、保留期上限、敏感标记）**沿用，不新造取值**」
⇒ 它是 D10 的**默认配置**，不是 R1 的决定。两者**不是同一对象**，台账**用同一个词指了两样东西**。

## 决策

1. **`viewer_memory_gate(policy)` 是这条开关的唯一判定处。**
   读取面、写入面与请求准入面 MUST 共用它，MUST NOT 各自复写 `if !policy.viewer_memory_enabled`。
   一条开关答两次就会分家（W88 的「同一件事 MUST NOT 两处定义」）。

2. **写入面 MUST 也判它**——这是本轮补的**执行体**，不是新增策略。
   判定是**能力级**的（不依赖条目内容），故它 MUST 在读、写两面都成立。

3. **拒因复用 `viewer_memory_disabled`，MUST NOT 新造名字。**
   读写两面是**同一个拒因**；「同一件事 MUST NOT 有两个名字」。后果是 `MemoryRejection::ALL`
   仍为 **23**，既有的「构造出的拒绝集合**恰为** `ALL`」总账断言**不需要放宽**。

4. **在 `admit_write` 里，能力开关放在「必填项之后、来源判定之前」。**
   与读取面「**结构必填 → 能力开关 → 授权**」同序（`read_records` 先报 `invalid_scope`、
   再报 `viewer_memory_disabled`）。理由：能力未启用**不依赖条目内容**即成立，是能力级判定；
   「来源不可信」是条目级判定。先报能力，调用方才知道该去开开关而不是去换来源。
   **顺序 MUST 有断言**，否则「顺序」只是注释（W57 §四.1 的同一条纪律）。

5. **开关 MUST NOT 塞进 `MemoryCandidate`。**
   让被审对象自带「能力是否启用」，就是 W57 §四.1 已排除的同一反模式（门槛交给被审对象自己填）。
   故 `admit_write` 增加 `policy: &MemoryPolicy` 入参。全仓调用方只有本 crate 与其集成测试。

6. **legacy `viewer_record_allowed` 保留 `EvaluateError` 契约，只做错误类型映射。**
   它是 D10 起的对外形状；本轮把**判定**改为调用 gate，把 gate 的拒绝**映射**成 `Unauthorized`。
   **映射不是第二处定义**——`if !enabled` 这个判定在仓库里只剩一处。既有内联单测
   `viewer_memory_is_off_by_default` 正好覆盖这条映射。

7. **F1（配置面 required 键零消费）本轮只登记、不改，MUST NOT 用「不动」伪装成「已处理」。**
   修它有两条路：给 `viewer_memory_enabled` 接线（改变运行时行为，超出本母题），
   或立一条新类别的断言「配置面的 `required` 键 MUST 有消费者」——
   后者本身是一个**独立母题**（与「判定面收口」不同维），故登记为遗留。

8. **台账的「空集」就地订正为两个具名对象**（活文档就地订正，1:1 行替换）。
   原句不是「说错了」，而是**一个词指了两样东西**；订正后分别写明：运行时字段值集合为空集（默认关闭的直接后果）、
   允许字段白名单沿用 D10 且 **R1 内不生效**（无接线）、R3 的**取值**仍未定。

## 后果与实施

- **代码**：`crates/nai-kernel/src/evaluate/memory.rs` 新增 `viewer_memory_gate`；
  `read_records` 与 `viewer_record_allowed` 改引它；`admit_write` 增加 `policy` 入参并在
  必填项之后加能力判定。`MemoryRejection::ALL` **不变**（23）。
- **测试**：`crates/nai-kernel/tests/memory.rs` **21 → 23**；新增「能力关闭时写入面同样被拒且与读取面同一拒因」
  与「能力判定先于来源判定」两组，含正向（启用后通过、非观众类别不受影响）与顺序断言。
- **台账**：`PD-31` → `decided_in_repo` + `landing: ["crates/nai-kernel/src/evaluate/memory.rs"]`；
  `DISPOSITION_COUNTS` `decidable_in_repo` **4 → 3**、`decided_in_repo` **1 → 2**（总数仍 34）；
  另**补正** W88 漏改的台账头部注释（当时仍只列五态）。
- **不改**：基线正文、`tests/registry.yaml`（H03/H04 仍 `pending`）、运行时（F1 只登记）、
  `allowed_viewer_fields` 的取值与 `max_retention_ms`、依赖边与契约名。

## 遗留

1. **F1：配置面 `session_defaults.viewer_memory_enabled` 是 `required` 键却零消费。**
   值得单独一轮（母题候选：「配置面 `required` 键 MUST 有消费者」，与 W84 的「测试文件可达性闭世界」
   同族——都是**清单之外即盲区**）。本轮**未改**，MUST NOT 读成已处理。
2. **「已接线」这件事本身仍无人证**：`admit_write`／`read_records` 在生产代码里**无调用方**
   （H03／H04 保持 `pending`，W57 报告 §五已说明）。本 ADR 修的是**判定面自足性**，
   **不是**运行时接线 ⇒ MUST NOT 读成「默认关闭已生效于产品」。
3. **`viewer_record_allowed` 的错误类型映射**是刻意的过渡形状：`EvaluateError` 只有 6 个变体
   （W57 §四.4 因此另立 `MemoryRejection`）。若将来统一错误面，这处映射应随之消失。
4. **余下 3 条 `decidable_in_repo` 仍只登记**：`PD-22`／`PD-24`（A2A 边界）、`PD-34`（需真实访谈）。
5. **本次不重排 W 编号**：本版为 **W89**（V3.11 计划止于 W88）。
