# w89：观众长期记忆的「默认关闭」收成单一来源，并补上写入面的执行体

## Why

W88 §八第 2 条登记了一处**张力**：台账 `PD-31`（*观众长期记忆的字段集与保留期*）的推荐写着
「R1 内判为『**空集 + 默认关闭**』」，而 W57 已交付的 `crates/nai-kernel/src/evaluate/memory.rs` 里
`MemoryPolicy::default().allowed_viewer_fields` 是 **非空**（`["preference", "answered_topic"]`）。

本轮按「**先读后判**」核实三处载体，结论比登记的张力**更靠前**：

- **F1（配置面 · 死键）**：`schemas/config/runtime.schema.json` 的 `session_defaults` 把
  `viewer_memory_enabled` 列为 **`required`**，但全仓**非测试**代码里
  `composition.config()["session_defaults"]` **只被读一次**（`apps/engine/src/pipeline.rs:874`），
  且只读同层的 `allow_auto_without_platform_grant` ⇒ 把它改成 `true` **没有任何效果**。
- **F2（判定面 · 只覆盖一半）**：`viewer_memory_enabled` 只被 `read_records`（**读取面**）与
  `viewer_record_allowed`（**无生产调用方**）消费；**写入面 `admit_write` 既不接受策略、也不检查它**
  ⇒ 默认关闭下一条 `kind = "viewer"` 的候选**会被接受**。
- **F3（措辞 · 同形不同义）**：推荐的「空集」论据是「§5.5 的默认关闭**使字段集没有对象**」，
  指**运行时被写入的字段值集合**；而 `allowed_viewer_fields` 是**启用后才生效的允许字段白名单**
  ⇒ 台账用**同一个词**指了两样东西。

⇒ 所以这不是实现缺陷，而是**「默认关闭」的落点不闭合**：三处载体里**只有读取面一处有执行体**。

## What Changes

- `crates/nai-kernel/src/evaluate/memory.rs`：
  - 新增 `viewer_memory_gate(policy)`——**「默认关闭」的唯一判定处**；
  - `read_records` 与既有的 `viewer_record_allowed` 改引它（后者保留其 `EvaluateError` 契约，
    只做错误类型映射，**判定本身不再复写**）；
  - `admit_write` 增加 `policy` 入参，对 `VIEWER_KIND` 候选在**必填项之后、来源判定之前**判能力开关；
    拒因**复用既有 `viewer_memory_disabled`**（同一件事 MUST NOT 有两个名字，拒绝名集合仍为 23）。
- `crates/nai-kernel/tests/memory.rs`：改调用点，新增「写入面默认关闭被具名拒绝」「能力先于来源的
  顺序是真的」「启用后合规候选通过且非观众类别不受开关影响」三组判据，含**正反双向**注入。
- `tests/pending-decisions.yaml`：`PD-31` 由 `decidable_in_repo` 迁 `decided_in_repo` + `landing`；
  **就地订正**原推荐里「空集」的指代（拆成两个具名对象），并写明 R1 内白名单不生效、R3 取值仍未定。

不改基线正文、不改 `tests/registry.yaml`、不新增依赖、不新增契约名、不动运行时（F1 只登记）。

## Impact

- capability 差量：`agent-harness`（一条 ADDED Requirement——记忆面本就归该能力）。
- 新增 ADR 0067；ADR 0054 **追加勘误段**（`decidable_in_repo` 的条数）——历史记录不就地改写。
- 新增活文档小节（`AGENTS.md`、`docs/development-plan.md`）。
- F1（配置面死键）登记为遗留，**留给单独一轮**（母题候选：「配置面 required 键 MUST 有消费者」）。
- 基线保持全绿。
