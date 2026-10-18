# W139：因果链勘定——utterance × frame-slot × mix-queue 的身份与生命周期

- 日期：2026-10-18
- 性质：**只勘定，不交付**。本轮没有 openspec 变更、没有 ADR、没有新增判据、没有门控改动、没有提交。
- 由来：W137 末「外部对照：OpenAvatarChat 对 nai 的可吸收面」的**第 1 项**
  （流对象 + 引用计数 + 祖先链级联取消 + 回收宽限），原文给的判断是「若只做一件，做第 1 项」。
  那条里有半句现测：`UtteranceTimeline` **只有 `utterance_id`、无父子关系**（timeline.rs:32-42），
  `CommitState::Late` 只做单句内取消，`takeover` 清的是 `auto_queue` **计数**（mixing.rs:404-411）。
- 本轮要回答的两个问题：
  - **Q1** 这三层今天各自有哪些可辨认的对象？它们拿什么当身份、活多久、怎么死？
  - **Q2** 要把「LLM 在途 + TTS 已合成未播 + 渲染队列中的帧」串到一条因果链上，**父子关系应该建在哪一层**？
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`

> 口径：下面每一格都有 `file:line`。计数是**现测**（本轮 `grep`/`Read` 得到），不是沿用上一轮的说法。

---

## 一、量出来的身份（每格可按 `file:line` 复核）

| 对象 | 定义处 | 它握着的身份分量 | 能不能回答「它是谁的孩子」 |
| --- | --- | --- | --- |
| `AudioBlock` | `crates/nai-media-core/src/timeline/audio.rs:18-29` | `session_id`、`utterance_id`、`generation_epoch`、`seq`（**4**） | 不能。四个是**并列**字段，`validate`（:38-69）只在「后一块 vs 前一块」之间判不变性，没有任何一段指出「谁生了它」 |
| `UtteranceTimeline` | `timeline.rs:32-42` | `utterance_id`（**1**） | 不能。构造函数（:44-64）只收这一个 id |
| `AckTracker` | `timeline/ack.rs:19-25` | `utterance_id` + `output_id`（**2**） | 不能。`for_utterance`（:39-54）只收这两个字符串 |
| `PlaybackProgress` | `lib.rs:289-295` | `output` + `utterance`（**2**） | 不能 |
| `PlaybackLane` | `playback.rs:73`（字段）+ :95-104（内部 `UtteranceTimeline` / `AckTracker`） | `utterance_id` | 不能 |
| `MediaGate` | `lib.rs:65-74` | `session` + `epoch` + `queue: VecDeque<String>`（队列元素是 **utterance_id 字符串**） | 不能。`enqueue`（:258-270）只把 `cue.utterance_id` 推进去 |
| `FrameMetadata` | `schemas/domain/media.proto:9` | `session_id`、`utterance_id`、`generation_epoch`、`frame_seq` | 不能 |
| `FrameSlotTicket` | `frame_slots.rs:285-297` | `pool_id`、`session_id`、`generation_epoch`、`slot_index`、`slot_generation`（**5**）；`buffer_ref` = `pool_id:slot_index:slot_generation`（:95-100） | **utterance_id 只存在于内嵌的 `frame` 里，不在槽位自己的身份里**（`matching` :306-319 是全 ticket 比较，不是按 utterance 索引） |
| `PoolSpec` | `frame_slots.rs:39-48` | `pool_id`、`session_id`、`generation_epoch`、`producer_ref`、`consumer_ref` | 没有 utterance 这个概念 |
| `AudioMixer.auto_queue` | `mixing.rs:314` | **一个 `usize`** | **根本没有对象**。`enqueue_auto(count: usize)`（:373-385）收的是「几」，不是「谁」 |
| `utterances` 行 | `migrations/sqlite/0001-foundation.sql:10` | `utterance_id` 主键 + `(workspace_id, utterance_id)` UNIQUE，带 `session_id`、`generation_epoch`，**FK → `sessions`** | **能**：这一行有父键 |

**第 1 个结论（身份宽度从 4 递减到 0）**。三个数据源（`AudioBlock` / `FrameMetadata` / `PerformanceCue`
（`schemas/domain/agent.proto:13-20`）都带完整的 `(session, utterance, epoch)` 三元组，甚至多带一个序号；
而**接它们的对象一路丢弃分量**：`UtteranceTimeline` 只留 1 个、`AckTracker` 留 2 个、MediaGate 的队列元素是裸字符串、
混流层是 0。⇒ 不是「没有身份可用」，是**身份在传递过程中被逐层削薄**。

**第 2 个结论（epoch 在生产路径上消失了）**。现测：`generation_epoch` 在 `timeline.rs`、`timeline/ack.rs`、
`playback.rs` 三个文件里**只出现在测试夹具**（分别是 :167、:130、:333，均在 `#[cfg(test)]` 块内）；
生产代码三处**零命中**。而 `AudioBlock::validate`（那份坚持 epoch 不变的不变量）在 `*.rs` 里的调用点
只有 `PoolSpec::validate`（`frame_slots.rs:188`、`shared_ram.rs:191`、`consumer.rs:61`、`apps/engine/src/lib.rs:954`）
和测试 `audio.rs:95,99` —— **音频时轴/ACK/播放这条链一次都没调用它**。

**第 3 个结论（今天唯一有父键的对象是那一行 SQL）**。有指向父表外键的只有 `utterances` 表行
（`FOREIGN KEY(workspace_id,session_id) REFERENCES sessions`）；而三处活对象一个父字段都没有。
这构成 Q2 的核心 tension：有表关系的那层不参与播放，参与播放的那些层没有表关系。

---

## 二、量出来的生命周期（四套状态机，逐字）

| 层 | 取值域 | 终态 | 能不能表示「已被取消」 |
| --- | --- | --- | --- |
| 时轴 `CommitState`（`timeline.rs:21-27`） | `NeedsAudio`、`NeedsVideo`、`Ready`、`Committed`、`Late` | `Committed` | **不能。** `commit()`（:128-145）返回 `Late` 但**不设置任何字段** ⇒ `Late` 是「这次调用的回答」，不是存储状态；下一次调用拿当下的 `now_ms` 重算 ⇒ **「这一句已被取消」在 `UtteranceTimeline` 上不可表示**（同样的问题 W138 在错误码上见过一次：状态存在，但没有出口） |
| ACK `PlaybackState`（`timeline/ack.rs:11-17`） | `Pending`、`Committed`、`Playing`、`Completed`、`Cancelled` | `Completed` / `Cancelled` | **能。** `cancel()`（:113-117）是黏的，且 `Completed` 不被覆盖 |
| 槽位 `SlotPhase`（`frame_slots.rs:166-172`） | `Free`、`Writing`、`Ready`、`Reading`、`Retired` | `Retired`（且 `docs/media.md:76` 明写「退休整池、不复用旧内存」） | 能，但**粒度是池**：`poll`（:225-241）里**任一槽位过期 ⇒ `retire()` 整池**；`disconnect`（:242-248）同理 |
| 混流 `MixMode`（`mixing.rs:259-262`） | `Auto`、`Takeover` | — | **没有队列项的状态**可言：`takeover`（:404-411）读 `auto_queue` 计数后归零，`cleared_utterances` 是报告值，从来不是被清除者的清单 |

**第 4 个结论（同一件未遂的事，仓库里已写了三遍，粒度互不相同）**。祖先失效 → 丢掉在途的后代：

1. `MediaGate::arm`（`lib.rs:159`）—— `self.queue.clear()`，**零报告**。
2. `MediaGate::poll`（`lib.rs:173`，过期/静音路径）—— `self.queue.clear()`，**零报告**。
3. `AudioMixer::takeover`（`mixing.rs:404-411`）—— `auto_queue` 归零，报告**一个计数**。

前两处丢完什么都不说；第三处说个数不说谁。 ⇒ OAC 第 1 项要求的 `ref_by` / `cancel_with_ancestors`
在这里**不是从零起步**，而是把已有的三处统一成「点名」的形状。

**第 5 个结论（SQL 侧已经写了六条级联，但终态口径三处打架）**。逐字现测：

- **6 条** `UPDATE utterances SET status=12` 语句（store.rs:149 全局、:264 工作区、:450 会话撤销、:527 事实版本变更、
  knowledge.rs:483 知识失效、publication.rs:445 文档再版），每条都带 `status NOT IN(11,12,13,14,15)`；
  其中 3 条紧接着用 `candidate_hash IN (SELECT text_hash FROM utterances … status=12)` 把审批一并撤回
  （knowledge.rs:485、publication.rs:446、store.rs:528）。⇒ 「祖先失效 → 后代连带失效」在这里**已经是既有机制**，
  不是待补的构想。
- `status BETWEEN 1 AND 5`（`store.rs:604`，pending 计数）；
- `status NOT IN(12,13,14,15)`（`store.rs:955`，preplay 有效性）。

现测命中行数：`NOT IN(11..15)` **6**、`BETWEEN 1 AND 5` **3**、`NOT IN(12..15)` **3**、`SET status=12` **6**
（`publication.rs:445` 一行同时含 `SET status=12` 与 `NOT IN(11..15)`，故计数有重叠）。
**分歧点：11 在口径 A 里是终态、在口径 C 里不是。** 这不是本轮该修的东西，但它决定了「能不能把新关系建在这张表上」。

---

## 三、装配现状（决定了 Q2 不是「挑一层」，而是「有没有一层」）

现测构造点分布（全仓 `--include=*.rs`，排除 `target/`）：

| 构造式 | 命中位置 | 生产装配点 |
| --- | --- | --- |
| `AudioMixer::new` | 11 处，全部在 `crates/nai-media-core/tests/mixing.rs` | **0** |
| `SlotOwner::new` | 8 处 `tests/frame_slots.rs` + 1 处 `frame_slots/shared_ram.rs`（即只经 `SharedFramePool::create`） | 仅 engine 的单帧 drill + broker drill 二进制 |
| `PlaybackLane::new` | `apps/engine/src/pipeline.rs:1338`（**函数局部变量**，一次一句、同步跑完即弃） | 有，但生命周期是一次调用 |
| `UtteranceTimeline::new` | `playback.rs:95`（被 lane 持有）+ 测试 | 同上，随 lane 一块儿消失 |
| `MediaGate::with_epoch_floor` | `crates/nai-runtime/src/composition.rs:160`（`GATE_CAPACITY = 64`，`:48`） | **有，且是全仓唯一一处** |

而且 `apps/engine/src/pipeline.rs:1-24` 的诚实边界逐字写着：

> Frames are produced in memory: **no frame slot is written, no keeper consumes them** and no device,
> window or hardware playback exists.

⇒ **第 6 个结论（决定性）：这三层今天在任何一条进程路径里都不共存。**
走音视频的那条路径（manual rehearsal pipeline）不碰槽位；碰槽位的那条路径（engine media drill）一帧到底、不碰 utterance 链；
混流层没有任何生产构造者。所以 Q2 的正确答案是：

> **没有一层今天能同时看见 utterance / frame-slot / mix-queue。落点不能靠「谁拥有它们三个」来选——因为谁都不拥有。**

顺带：`utterances` 表的生产者 `Store::record_candidate`（`store.rs:568`）的调用点只有测试
（`crates/nai-control-api/src/tests.rs:409` + `crates/nai-storage/src/tests/` 下 9 处）⇒ 那条「最有父键的形状」
在生产里同样**没人写**。这与 W138 撞到的形状同族（journal 有表有代码、没有生产者）。

---

## 四、五个候选落点，逐层排除

**(a) 混流队列（`AudioMixer`）——排除。**
理由不是它不重要，是它今天**没有一个可作为子项的对象**：`auto_queue` 是 `usize`。要在此建父子，第一步必然是把计数换成带身份的条目，而那个身份只能是 `utterance_id` ⇒ 等于在**已经存在的**上游身份之外再造一份。这撞本仓的硬规则（同一件事 MUST NOT 两处定义）。它的正确位置是**报告面**而不是**关系面**。

**(b) 帧槽位（`SlotOwner` / 池）——排除作父，可以作子。**
两条阻断：① 失败域是**池**不是槽（任一槽过期退体整池，`frame_slots.rs:229-240`），挂在槽上的血缘会被连坐；② 槽位靠 `slot_generation` 自增**复用**（`:281-284`），同一 `(pool_id, slot_index)` 换了 generation 就是另一帧 ⇒ 亲属关系会被静默继承给下一帧。③ `PoolSpec` 的作用域是 `(session, epoch)`，压根没有 utterance 概念；要以池当父，先得把 ADR 0017 的所有权模型改成按 utterance 切池。

**(c) 现有 utterance 侧对象（`UtteranceTimeline` / `AckTracker`）——排除作「户籍」，只能作**被登记的孩子**。**
它们三者确实念同一个 id（这也是 W137 那半句现测的出处），但：
- `UtteranceTimeline` 的生命周期**等于调用方的局部变量**（`pipeline.rs:1338` 里随函数结束消失），没有任何 registry 能回答「现在在途几句」；
- 「在途几句」今天有**两个互不相干的答案**：`AudioMixer::queued()`（`mixing.rs:348`）返回计数、`MediaGate::queued()`（`lib.rs:114`）返回 `queue.len()` —— 两者覆盖的是**不同的集合**、口径不同、互不知情；
- `CommitState` 没有 `Cancelled`，`Late` 不黏 ⇒ **「这句已被取消」在这层不可表示**。这与 W138 的 `NOT_FOUND` 是同一类缺陷：一个已存在但没有出口的状态。

**(d) 存储 `utterances` 行——排除作主，但它是唯一现成的父键，应被引用而非被绕过。**
阻断：① `status` 是裸整数且三种终态口径互相矛盾（11 是不是终态两处打架）⇒ 在建关系之前先得有单一列举，而这本身是另一轮的活；② 它的生产者只有测试；③ 持久 vs 内存的寿命差今天不可表示（内存对象已经消失、DB 行还活着，没有人对得上）。

**(e) 新建 lineage registry，挂在 `MediaGate` 所在的作用域（`(session, epoch)`）——推荐落点。**
三条现测理由：
1. `MediaGate` 是**唯一有生产装配点**的那个（`composition.rs:160`），寿命随整个 `Composition` 而不是一次函数调用；
2. 它的键（`(session, epoch)`）与三个数据源那个三元组的前两位**已经对齐**，只差补上 `utterance_id` 这一位 ——
   也就是「把它已经持有的 session/epoch 再加一维」，不是另起一套命名空间；
3. 今天那三处「祖先失效就丢后代」的前两处就在它里面（`arm` / `poll` 的 `queue.clear()`），混合器那处是它的邻居 ⇒ 在这里建立 registry，`queue.clear()` 才能从「丢完什么都不说」升级为「点名」。

**不是**在时轴里、**不是**在槽位里、**不是**在混流里；也**不是**直接落 SQL —— 而是把 OAC 的流对象建成一个
以 `(session_id, utterance_id, generation_epoch)` 为身份、寿命随 `Composition` 的对象，三层的现有东西按
**facade kind**（pending-gate / audio-block / slot-ticket / output-ack）登记为它的孩子， storage `utterances`
行作为**可恢复的那一份**（不是主权所在）。

---

## 五、若按 (e) 动手，判据至少要有这三种形状（先写死，免得当场发挥）

1. **清点 vs 计数成对**：任何「取消/清空」必须同时给出**清单**（丢了哪几条）与**计数**（几条）。
   只有计数 ⇒ 「丢的是不是它」不可判（这正是今天 `cleared_utterances` 的现状，B05 的 registry 备注里
   已明写它「作为报告值而非断言」）。
2. **终态必须有一个入口**：新的取值域 MUST 含显式的 cancelled / recycled 两格，并由**一处**派生
   （照 W138「`terminal` 由 `UpgradeStep::terminal()` 派生」的形状）。反例就摆在 `timeline.rs`：`Late`
   返回值不落库 ⇒ 同一个问句两次调用可以给不同答案。
3. **宽限必须是值不是 sleep**：OAC 的 `recycle_ttl` 对应到今天是**零**（`grace/drain/recycle/宽限` 在这三条链上
   零命中，存在的只有 `SessionLifecycle::Draining`（`nai.v1`）与 `IoBudget` 的即时 deadline）。
   要做就得做成「可观测的剩余时间」，MUST NOT 是 `sleep` 或「下次 poll 才发现」。

---

## 六、诚实的边界（防止把勘定读成「已经想好了」）

- 上面 **(e)** 说的是**该建在哪一层**，不等于**建成后就有东西可登记**：按今天的装配，能被真的登记进去的
  只有音频/ACK 那一面（经 `pipeline.rs`）；槽位那一面只有 drill，混流那一面**生产的构造者为零**。
- 因此下一轮若动手，可交付的最多是：一个定义性的对象 + 三条登记点 + 针对它自身的判据；
  **MUST NOT** 在报告里写「LLM 在途 / TTS 未播 / 渲染队列已在同一因果链上」—— 那要等各自的生产者接进来才算。
- 「宽限」今天没有任何参考实现，OAC 那半条（`recycle_ttl`）在本仓的形状必须由将来某一轮靠实测定，不能照抄数值。

---

## 七、具名登记（本轮发现但不做，逐条归宿）

| # | 现测事实 | 为什么本轮不改 | 归宿 |
| --- | --- | --- | --- |
| N1 | `utterances.status` 三种终态口径互相打架（11 在 A 是终态、在 C 不是） | 锁定词汇要动 6 条写入语句 + 3 处读取口径，属独立一轮 | 建议与 (e) 的 terminal 派生同轮处理 |
| N2 | `UtteranceTimeline::push_audio` 只比 `utterance_id`（`timeline.rs:80`），丢掉 `session_id`/`generation_epoch` | 一动就改时轴的准入面，牵动既有 media 判据组 | (e) 落地时顺手补，或单列一轮 |
| N3 | `AudioBlock::validate` 坚持的「同一 utterance 内 epoch 不变」在音频链上从未被调用 | 接上去要先给端到端 fixture 补 epoch 维度，本轮不动代码 | 与 (e) 的 facade 登记同轮 |
| N4 | 「在途几句」有两个互不知情的答案（`AudioMixer::queued` / `MediaGate::queued`） | 二者今天都无生产装配者 ⟹ 合并没有意义 | (e) 落地后由 registry 独占 |
| N5 | `AudioMixer` / `SlotOwner` 生产构造者为 0 | 属第 ④ 环缺口（安装器/ launcher 类），仓库内不可解 | 与 W138 的 12 条同族路由一类处理 |
| N6 | engine 的 media drill 只有一帧、rehearsal pipeline 不写槽位（由 `pipeline.rs:20-24` 自述） | 是既有的诚实边界，不是缺陷 | 不动 |

---

## 八、下一轮的最小切片（若继续）

1. 只在 `crates/nai-media-core` 里落一个 lineage registry（纯逻辑、零新增依赖、可单测），身份是
   `(session_id, utterance_id, generation_epoch)`，孩子按 facade kind 登记，含两格终态与一处派生；
2. 把 `MediaGate::arm` / `poll` 的 `queue.clear()` 改成**点名并报告**（清单 + 计数成对）；
3. 判据：注入至少覆盖「只报计数不报清单」「终态缺一格」「宽限不是值」三种退化；
4. `AudioMixer` / `SlotOwner` **不碰**——它们没有生产装配者，先碰就变成给空气造家具。

（本轮无提交、无门控、无 CI 观测 —— 勘定轮按惯例不产生这三样。）
