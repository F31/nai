# Proposal: W140 — 媒体因果链：把「清队列」改成点名并报告

## Why

W139 是纯勘定轮，量出的事实（逐条现测，见 `docs/reports/2026-10-18-w139-causal-chain-appraisal.md`）：

| 事实 | 现测 |
| --- | --- |
| 身份宽度递减 | `AudioBlock`/`FrameMetadata`/`PerformanceCue` 带完整三元组 → `UtteranceTimeline` 留 1 → `AckTracker` 留 2 → `MediaGate` 队列元素是**裸字符串** → 混流层 0 |
| epoch 在生产路径上消失 | 只在 `#[cfg(test)]` 夹具里出现；`AudioBlock::validate` 的音频链没被调用 |
| 今天唯一有父键的 | SQL 那一行 `utterances`（`FK→sessions`） |
| 同一件未遂的事写了三遍 | `MediaGate::arm`（`lib.rs:159`）与 `poll`（`:173`）的 `queue.clear()` **零报告**；`AudioMixer::takeover`（`mixing.rs:404-411`）**报计数不报清单** |
| 终态口径三处打架 | SQL 已有 6 条级联 `SET status=12 … NOT IN(11..15)`，分歧点在 11 |
| 决定性一条 | 三层在**任何一条进程路径里都不共存**：`AudioMixer::new` 的 11 处全在 tests；`MediaGate::with_epoch_floor` 唯一生产点是 `crates/nai-runtime/src/composition.rs:160` |

于是「父子关系建在哪一层」的答案不是挑一层，而是**只有 `MediaGate` 那一层有生产装配者**。
本轮按落点 **(e)** 动手：在 `nai-media-core` 落一个 lineage registry，挂在 `MediaGate` 所在的
`(session, epoch)` 作用域，并把两处 `queue.clear()` 换成**点名并报告**。

**(e) 说的是「该建在哪一层」，不等于「建成后就有东西可登记」。** 本轮可交付的最多是：
一个定义性的对象 + 三条登记点 + 针对它自身的判据。

**验收映射**：本轮落在 `session-leases-and-cancellation` 规格（历史追溯 D06「会话/租约/取消」的
媒体门控部分）。`tests/registry.yaml` 的 88 条里**没有**对应「媒体因果链 / 级联取消」的条目，
故本轮**不**标任何条目状态，只落内部判据。

## What changes

- `crates/nai-media-core/src/lineage.rs`（新）：`LineageId`（`session_id`/`utterance_id`/`generation_epoch`）、
  `FacetKind` 五值闭集、`NodeState` 四值且终态由**一处**派生、`LineageRegistry`、`LineageReport`。
  零新增依赖（只用 `std`）。
- `MediaGate`：新增 `lineage` 字段；`arm` 改为「取消并点名 → 推进作用域 → 为每个被准入的 utterance
  登记可恢复行 → 回收」；`poll` 的失败分支同样取消并点名；`enqueue` 登记 `PendingGate` 子节点；
  `consume` 释放该子节点，并在「队列点名了、lineage 没有在途子节点」时**拒绝**。
  新增 `lineage()` / `take_cancellation()` / `take_reclamation()` 三个读取面。
- 判据：`crates/nai-media-core/tests/lineage.rs`（7 条）+ `tests/gate.rs` 新增 4 条接线用例。
- 文档：ADR 0114、报告、`docs/development-plan.md`。

## Impact

- **签名零变更**：`arm`/`poll`/`enqueue`/`consume` 的签名与返回值不变，`apps/output-keeper`、
  `crates/nai-control-api`、`crates/nai-runtime` 的调用点无需改动（既有 61 个用例全绿已证）。
- **零新增依赖**，纯逻辑，可在任意平台单测。
- `consume` 多了一条拒绝路径：队列点名了但 lineage 没有在途子节点 ⇒ `MediaError::Stale`。
  这是「两处口径不一致时选拒绝」，不是放宽。
- **`AudioMixer` / `SlotOwner` 不碰** —— 它们没有生产装配者，先碰等于给空气造家具。
- `tests/registry.yaml` 88 条里**没有**对应「媒体因果链」的验收条目 ⇒ 本轮**不**标任何条目为
  `tested`，只落内部判据。

## Alternatives considered

- **(a) 挂在混流层**：否决。`AudioMixer::new` 的 11 处调用全在 tests，没有生产装配者。
- **(b) 挂在 frame-slot 层**：否决。`FrameMetadata` 的身份完整，但 `SlotOwner` 同样只有测试装配者。
- **(c) 直接在 SQL 加一张 lineage 表**：否决。会把一个今天只用于「报告取消了谁」的纯逻辑关系
  变成需要迁移、需要备份、需要级联的持久事实；而它服务的两条路径（`arm`/`poll`）运行期根本不碰数据库。
- **(d) 只在 `takeover` 那里补清单**：否决。那是三处同型缺口里**唯一没有生产调用者**的一处，
  补它等于把可见性加到最没人走的那条路上。
- **给 `poll` 加 `utc_ms` 参数以便取消时打时间戳**：否决。取消**不需要**时钟——
  `recycle_at_ms` 在登记时就写入了，取消只改状态。省下 11 个既有调用点的改动。
- **让 `register` 对重复登记报错**：否决。同一个 utterance 在队列里出现两次时，
  第二次是**同一个孩子**，不是第二个；拒绝它会把合法状态判成冲突。改为刷新截止时间。
