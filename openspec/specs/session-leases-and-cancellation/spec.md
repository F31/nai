# session-leases-and-cancellation Specification

## Purpose
为 nai 提供会话隔离、生命周期与 epoch 撤销、任务取消、有界命令幂等、独立媒体租约门控和诚实播放完成确认；使过期授权、迟到数据与故障恢复不能重新外发旧内容。portable 逻辑验证不替代实际设备或 Windows 音轨验收。

## Requirements

### Requirement: Scoped session lifecycle and cancellation
系统 SHALL 分离生命周期、模式、媒体状态，要求可信 actor 与 preflight；恢复 SHALL 撤销旧 epoch 和授权。任务取消 SHALL 不影响无关任务，全场撤销 SHALL 使全部旧任务失效。

#### Scenario: Recovery and late task
- **WHEN** 会话恢复或紧急撤销后旧任务返回
- **THEN** epoch 不匹配导致结果被拒绝，恢复仅进入待机并重新 Prepare

#### Scenario: Single task cancellation
- **WHEN** 同一会话仅取消一条任务
- **THEN** 该任务令牌失效，无关任务令牌和会话 epoch 保持有效

### Requirement: Independent fail-closed media gate
媒体 SHALL 在消费前复检 session、epoch、候选哈希、审批、事实版本、策略与短租约，并以本地单调期限执行失联静音。P0 SHALL 无须获取普通业务或数据库锁，先撤销本地输出再更新会话。任何旧租约 SHALL 不能自行解除紧急静音。

#### Scenario: Stalled engine and expiry
- **WHEN** 引擎不再续租或媒体本地紧急静音
- **THEN** 媒体独立拒绝播放并清除待播元数据，旧租约不能恢复输出

#### Scenario: Changed facts or approval
- **WHEN** 候选、事实、审批、模式或会话与当前租约不一致
- **THEN** 消费被拒绝

### Requirement: Bounded command idempotency
命令 SHALL 在会话作用域内幂等，重复 ID 不可改变内容，记录容量与 TTL SHALL 有界且不得提前淘汰重试窗口内记录。紧急静音 SHALL 在去重容量耗尽时仍撤销媒体输出。

#### Scenario: Replay and exhaustion
- **WHEN** 重试同一命令或去重记录已满
- **THEN** 相同命令返回原结果，不同内容被拒绝，新普通命令被拒绝但 P0 保持静音

### Requirement: Honest completion and durable revocation
Completed SHALL 仅由匹配的实际本地播放完成 ACK 推进；跨故障未知任务 SHALL 取消而非自动重播。撤销检查点 SHALL 用 revision CAS 持久化新 epoch、撤销审批与取消未完成任务，持久化失败不恢复媒体权限。

#### Scenario: Submitted is not completed
- **WHEN** 媒体仅接收或提交输出而未报告实际完成
- **THEN** 任务不能标记 Completed

#### Scenario: Storage conflict after mute
- **WHEN** 媒体已静音但撤销检查点写入发生冲突
- **THEN** 写入报错，输出继续拒绝，重新协调后才可显式授予新租约

### Requirement: Cancellation names what it dropped, and the count is the manifest

媒体门控 SHALL 为它准入的每个 utterance 维护一条 `(session_id, utterance_id, generation_epoch)`
的因果链；撤销（重新布防、租约到期、紧急静音）MUST **逐条点名**被丢掉的因果链，而不是只报一个数字。
报告 MUST 同时带清单与计数，且**计数 MUST 由清单派生**，MUST NOT 另存一个计数器——
另存就会与清单各说各话。清单长度 MUST 等于实际被改状态的节点数。
一次撤销什么都没丢时 MUST 报**空清单**（计数 `0`），MUST NOT 短路成「没发生过」：
「撤销跑了但没东西」与「撤销没跑」是两件不同的事。

#### Scenario: A re-arm drops the previous generation

- **WHEN** 门控以更高的 generation 重新布防，而上一代有在途的因果链
- **THEN** 返回的清单逐条命名上一代的链条（含 utterance 与 facade 种类），计数等于清单长度

#### Scenario: A mute drops a queued child and its parent

- **WHEN** 紧急静音后轮询门控，而队列里有一个已登记的子节点
- **THEN** 清单同时命名可恢复行与该子节点；MUST NOT 只报「清了 1 条」

#### Scenario: Nothing was in flight

- **WHEN** 首次布防，lineage 里还没有任何东西
- **THEN** 报告是 `Some(空清单)`，MUST NOT 是 `None`——`None` 只表示撤销根本没跑

### Requirement: Terminal cells are explicit and derived in one place

因果链节点的状态 MUST 是**四值闭集**：在途、已释放、已取消、已回收。其中**已取消**与**已回收**
MUST 是显式两格，MUST NOT 折叠成「不在途了」。终态与否 MUST 由一处 `terminal()` 派生，
MUST NOT 在各消费点另写一份判定。

#### Scenario: A terminal cell is dropped

- **WHEN** 有人把「已回收」从终态集合里去掉
- **THEN** 闭世界判据具名失败；MUST NOT 靠「不在途」这个反义词兜住它

#### Scenario: Every state is classified exactly once

- **WHEN** 检查四值的分类
- **THEN** 终态与非终态把它们**划分**成两份（数量相加等于四，且非终态恰一格）

### Requirement: The grace before reclamation is a value, never a wait

回收 MUST 以**登记时写入的截止时间值**（`recycle_at_ms`）为准，MUST NOT 以等待或墙钟为准：
回收函数 MUST 只把调用方传入的时钟与记录值比较，MUST NOT 读任何时钟，也 MUST NOT 阻塞。
在途节点 MUST NOT 被回收，无论时钟走到多远。同一时钟下第二次回收 MUST 报 `0`。

#### Scenario: Two nodes with different deadlines

- **WHEN** 两条链的截止时间分别是 1000 与 9000，且都已不在途
- **THEN** 时钟 1000 只回收第一条，时钟 9000 才回收第二条；MUST NOT 一次全收，也 MUST NOT 一次不收

#### Scenario: A node still in flight

- **WHEN** 节点仍在途，而时钟已远超它的截止时间
- **THEN** 不回收，且报告为空——在途不是「到期」

#### Scenario: Reclaiming twice at the same clock

- **WHEN** 同一时钟下连续两次回收
- **THEN** 第二次报 `0`，MUST NOT 重复报告同一条链

### Requirement: A child must have a registered parent, and out-of-scope ids are refused

子节点 MUST 只能挂在**已登记的可恢复行**上：没有父行的子节点 MUST 被拒绝。
会话或 generation 与门控作用域不符的 id MUST 被拒绝。作用域 MUST NOT 回退到更早的 generation。
登记 MUST NOT 静默降级——被拒时返回具名的拒绝原因。

#### Scenario: A child with no parent

- **WHEN** 登记一个子节点，而该 utterance 的可恢复行尚未登记
- **THEN** 拒绝，原因为「父不存在」；MUST NOT 隐式补一个父行

#### Scenario: An id from another session or generation

- **WHEN** 登记的 id 的会话或 generation 与门控作用域不同
- **THEN** 拒绝，原因为「作用域不符」；MUST NOT 并入当前作用域

### Requirement: Consumption is authorized by the lineage, not by the queue alone

队列只给出顺序，lineage 给出**谁**。消费 MUST 以 lineage 里在途的那个子节点为准：
队列点名了某个 utterance 而 lineage 里**没有**在途子节点时 MUST 拒绝，
MUST NOT 因为「队列里有它」就放行——那会让两个视图不一致时安静地播出去。

#### Scenario: Queue and lineage agree

- **WHEN** 队列头部是该 utterance，且它的 `PendingGate` 子节点在途
- **THEN** 消费成功，该子节点转为「已释放」（终态）

#### Scenario: Queue names an utterance with nothing in flight

- **WHEN** 队列头部是该 utterance，而 lineage 里没有在途的对应子节点
- **THEN** 拒绝消费；MUST NOT 把它当作「在途」放行
