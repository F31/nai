# Proposal: W127 — 从决策日志重建装配，以**条目**为单位（H02）

## Why
`tests/registry.yaml` 的 **H02** 标题就是验收口径：「装配可重建：由决策日志的片段引用重建**同一**上下文，
不依赖当时内存」。W104 落地 `Composition::rebuild_assembly_from_log` 时把读回的多行装配
**合并成一个片段池**再交给判定，并在代码注释、报告、`docs/runtime.md`、ADR 0082/0084 里**具名登记**为
一处弱化：它把问题从「日志能否重建**这一次**装配」降为「日志能否拼出**某一次**装配」。
ADR 0084 决策 6/8 明确把这处收紧推迟为**独立一轮**——本轮即是它的对象。

本轮先跑会红的判据，实测确认弱化可复现：链上两个条目分别只承载 `frag-a` 与 `frag-b`，
记录声明 `[frag-a, frag-b]` ⇒ 当前实现返回 `rebuildable = true` 且**零拒绝名**。
没有任何条目曾经装配过这个上下文，`rebuilt` 序列也无从谈起；而「这两个标识没落日志」是**假话**——
它们都在链上。

## What Changes
1. 判定面（`crates/nai-kernel/src/evaluate/assembly.rs`）：
   - 新增 `entry_carries_assembly(record, entry)`：一个条目的片段标识**多重集**（含重复、**不计顺序**）
     与记录相同才视为承载。用**多重集**而非集合：`[f1, f1]` 不被 `[f1]` 承载；**不计顺序**是刻意的——
     按确切顺序选行会让既有的 `rebuild.order_differs` **不可达**，把一条具体原因悄悄换成更模糊的。
   - 新增 `rebuild_assembly_from_entries(record, entries)`：逐个条目判定，**存在一个**能重建即可重建；
     没有任何条目承载时具名 `rebuild.not_in_one_entry`，并对**整条链**（不是空池）重新派生
     「哪些标识真的没落日志」，避免把链上存在的标识报成未落日志。
   - `rebuild_assembly` 一行判定逻辑未改，仅把「版本齐备」抽成 `version_rejections` 供两条路径**共用**
     （一次定义）。`RebuildRejection::ALL` 6 → **7**。
2. 接线（`crates/nai-runtime/src/assembly.rs`）：`rebuild_assembly_from_log` 不再 `flat_map` 合并行，
   而是把**每一行作为自己的条目**传入；删去那段「跨行合并」的 ⚠️ 披露注释。
   写路径（`record_assembly`）**不动** —— 它的输入是「本次要落盘的素材」，本来就只有一份，
   其判定对象不是「链上一个条目」，故仍用 `rebuild_assembly`。
3. 判据：kernel `tests/assembly.rs` **36 → 40**（多重集/顺序无关、跨条目分裂被具名拒、
   存在一个承载条目即足够且不吞掉 `order_differs`、空链两名并存）；
   runtime `tests/assembly.rs` **25 → 27**（跨两条目分裂、承载条目是超集）；
   并改造既有非空洞守卫 `every_rebuild_rejection_is_reachable_through_the_runtime`：
   它原先只从**写路径**取样本，而 `not_in_one_entry` 是**链级**事实、写路径结构上产生不出来 ⇒
   该变体走**读回面**取样，并在注释里写明这一不对称本身是结论。

## Modified Capabilities
- `runtime-composition`：`An assembly is rebuilt from the persisted chain alone` —— 重建判定 MUST 以
  **条目**为单位，MUST NOT 先合并成片段池；承载判据是**标识多重集**（顺序不参与）；无条目承载时
  MUST 具名 `rebuild.not_in_one_entry`，且 MUST NOT 把链上存在的标识报成未落日志。

## Impact
- registry：`H01` / `H02` notes **就地追加**本轮段落，**两者 `status` 均维持 `pending`** ——
  本轮闭合的是「判定面 → 读回面」的口径收紧，**不是**验收达标：装配器的**真实产生者**
  （把九段片段喂给模型、§41.2 分段预算初值标定、§47.2 六类回归属 **H16**）仍未接线。
- 该变体是**链级**原因，写路径产生不出 ⇒ 既有「写路径一个入口能产出全部重建拒因」的守卫形状
  被如实拆成两面，两个面各自仍有独有覆盖。
- 零新增依赖、零契约字段改动、零迁移、零 schema 改动、零前端改动。
- 历史记录（ADR 0082、ADR 0084、W106 报告、已归档的 W106 变更 design）按纪律**追加勘误段**，
  MUST NOT 就地改写；活文档（`docs/runtime.md`、`AGENTS.md`、`docs/development-plan.md`、
  代码注释、registry）**就地 1:1 订正**。
