# ADR 0114：媒体因果链——父子关系建在 MediaGate 那一层，撤销改成点名并报告

- 状态：已接受（W140，2026-10-18）
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`
- 相关：W139 勘定报告 `docs/reports/2026-10-18-w139-causal-chain-appraisal.md`、
  ADR 0113（W138 `UpgradeStep::terminal()` 单一派生的同形做法）、
  `crates/nai-media-core/src/lineage.rs`

## 背景

W139 是纯勘定轮，把 utterance × frame-slot × mix-queue 三处的身份与生命周期量了一遍，得到六条结论。
其中决定本轮形状的是两条：

1. **身份宽度从 4 递减到 0**：`AudioBlock`/`FrameMetadata`/`PerformanceCue` 带完整
   `(session_id, utterance_id, generation_epoch)`，`UtteranceTimeline` 只剩 1、`AckTracker` 剩 2，
   而 `MediaGate` 的队列元素是**裸 `String`**，混流层是 0。
2. **三层在任何一条进程路径里都不共存**：`AudioMixer::new` 的 11 处调用点全在 tests；
   `MediaGate::with_epoch_floor` 的生产装配点只有 `crates/nai-runtime/src/composition.rs:160`
   与 `apps/output-keeper/src/lib.rs:99`。

同一件未遂的事被写了三遍，且三遍都是「不报告」：`MediaGate::arm`（`lib.rs:159`）与 `poll`（`:173`）
的 `queue.clear()` 零报告，`AudioMixer::takeover`（`mixing.rs:404-411`）报计数不报清单。

于是「父子关系应该建在哪一层」的答案不是挑一层，而是**只有一层有生产装配者**。

## 决定

1. **因果链挂在 `MediaGate` 所在的 `(session, epoch)` 作用域**，身份是
   `(session_id, utterance_id, generation_epoch)`，孩子按 facade 种类登记，
   可恢复的那一份是 `FacetKind::UtteranceRow`（对应 SQL `utterances` 行）。
2. **在 `nai-media-core` 落一个纯逻辑 registry（`src/lineage.rs`），零新增依赖。**
   它不碰数据库、不碰设备、不做 IO，因此可以在任意平台单测。
3. **撤销改成点名并报告**：报告同时带清单与计数，**计数由清单派生**，不另存计数器。
4. **终态四值闭集**（在途 / 已释放 / 已取消 / 已回收），`cancelled` 与 `recycled` 是显式两格，
   终态与否由**一处** `terminal()` 派生（照 ADR 0113 里 `UpgradeStep::terminal()` 的同形做法）。
5. **宽限是值不是等待**：`recycle_at_ms` 在登记时写入，`reclaim(utc_ms)` 只把调用方传入的时钟
   与记录值比较，不读任何时钟、不阻塞。
6. **`AudioMixer` / `SlotOwner` 本轮不碰**——它们没有生产装配者。

## 被否决的落点

| 落点 | 否决理由 |
| --- | --- |
| (a) 混流层 | `AudioMixer::new` 的 11 处调用点全在 tests，没有生产装配者 |
| (b) frame-slot 层 | `FrameMetadata` 身份完整，但 `SlotOwner` 同样只有测试装配者 |
| (c) SQL 里新建一张 lineage 表 | 会把一个只用于「报告取消了谁」的纯逻辑关系变成需要迁移/备份/级联的持久事实；而它服务的两条路径（`arm`/`poll`）运行期根本不碰数据库 |
| (d) 只在 `takeover` 那里补清单 | 那是三处同型缺口里**唯一没有生产调用者**的一处，补它等于把可见性加到最没人走的那条路上 |

另外两条较小的否决：

- **给 `poll` 加 `utc_ms` 参数**：取消不需要时钟——`recycle_at_ms` 在登记时就写入了。
  加上它要改 11 个既有调用点，换来的是一个用不上的参数。
- **让 `register` 对重复登记报错**：同一个 utterance 在队列里出现两次时，第二次是**同一个孩子**
  而不是第二个；拒绝它会把合法状态判成冲突。改为刷新截止时间。

## 判据的三条形状（先写死，再实现）

1. **清点 vs 计数成对**：`LineageReport::count()` 是 `manifest.len()`，类型上没有第二个数字可存；
   判据另外断言「清单长度 == 实际被改状态的节点数」，这样即使将来有人加了一个计数器也会红。
2. **终态两格 + 一处派生**：闭世界断言四值被终态/非终态**划分**（相加等于四、非终态恰一格），
   并逐格断言 `cancelled`/`recycled` 是终态。
3. **宽限是值**：两条不同截止时间的链在同一时钟下**只收该收的那条**；在途节点时钟走到多远都不收；
   同一时钟第二次回收报 `0`。

## 诚实边界（MUST NOT 越线）

- 本轮落的是**定义性的对象 + 三条登记点 + 针对它自身的判据**。
  **MUST NOT** 在报告里写「LLM 在途 / TTS 未播 / 渲染队列已在同一因果链上」。
- `FacetKind` 五值里，只有 `UtteranceRow`（`arm` 登记）与 `PendingGate`（`enqueue` 登记）**有登记者**；
  `AudioBlock` / `SlotTicket` / `OutputAck` **定义了但没有登记者**——它们是把未来三种 facade 的
  接缝留出来，不是已覆盖的证据。
- `enqueue` / `consume` 全仓只在 `tests/gate.rs` 被调用 ⇒ 这两条登记点在**生产路径上没有调用者**。
  生产路径上真正会写 lineage 的只有 `arm`（output-keeper 的 `Action::Arm`）与 `poll` 的取消分支。
- `tests/registry.yaml` 的 88 条验收条目里**没有**对应「媒体因果链/级联取消」的条目
  ⇒ 本轮**不**标任何条目为 `tested`。

## 残余风险

- 终态口径在 SQL 侧（6 条级联 `SET status=12 … NOT IN(11..15)`）与本 registry 之间**尚未对齐**，
  分歧点在值 11。本轮没有动 SQL，也没有把两者对账——那需要先裁定 11 到底算不算终态。
- 回收在 `arm` 与 `enqueue` 触发；若一个门控布防后再无人问津，已取消的节点会留到下一次布防才被收走。
  这是**有界的**（每代最多 `capacity` 条 + 每 utterance 一个子节点），不是泄漏，但也不是即时回收。
