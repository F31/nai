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
