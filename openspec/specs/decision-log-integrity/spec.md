# decision-log-integrity Specification

## Purpose
建立统一的只追加哈希链决策日志（V3.10 §5.8、INV-19、R07 前置）：所有事件→决策→守门→审批→取消的关键链路记录在同一追加日志中，链完整性可机器校验；按 ADR 0015 勘误实现 critical/telemetry 分级写入与有界缓冲。

## Requirements

### Requirement: Append-only hash-chained decision log

存储层 SHALL 提供 `decision_log` 表：每条记录包含 workspace 作用域、可空 session、kind、criticality、payload_json、payload_hash、prev_hash、entry_hash、created_at_ms 与 trace_id；同一 workspace 内每条记录的 prev_hash MUST 等于前一条的 entry_hash，创世条目 prev_hash 为 64 个 '0'。表 MUST 通过触发器拒绝一切 UPDATE 与 DELETE。

#### Scenario: Chain verification passes
- **WHEN** 通过 API 顺序追加多条记录后执行 `verify_decision_chain`
- **THEN** 校验通过，且 payload_hash/entry_hash 可由存储内容独立重算一致

#### Scenario: Forged entry detected
- **WHEN** 绕过 API 直接插入 prev_hash 不正确的记录后执行链校验
- **THEN** 校验在该 seq 处失败并报告位置

#### Scenario: Mutation rejected
- **WHEN** 对 decision_log 执行 UPDATE 或 DELETE
- **THEN** 触发器中止操作，既有链保持不变

### Requirement: Criticality-tiered writes with bounded buffer
critical 类别（approval、cue、cancellation、duty_lease）的写入失败 MUST 原样返回错误且不得进入缓冲或丢弃；critical MUST 不在提交前冲刷遥测。telemetry 类别写入失败 SHALL 进入容量有界的内存缓冲，缓冲满时丢弃最旧条目、累加丢弃计数并告警。恢复后缓冲与后续新遥测 SHALL 按缓冲顺序在同一事务提交，失败全部回滚；提交成功前不得清除缓冲。critical 可先于缓冲提交，日志序号表示持久化顺序，不宣称事件时间全序。

#### Scenario: Telemetry failure buffered
- **WHEN** telemetry 记录因底层失败无法写入
- **THEN** 该记录进入有界缓冲，调用方不收到致命错误，缓冲统计可见

#### Scenario: Critical failure never buffered
- **WHEN** critical 记录写入失败
- **THEN** 错误原样返回，缓冲长度不变，不允许静默丢弃

#### Scenario: Buffer overflow drops oldest
- **WHEN** 缓冲已满且又有 telemetry 写入失败
- **THEN** 最旧条目被丢弃、丢弃计数增加并产生告警，缓冲长度不超过容量

#### Scenario: Partial flush failure
- **WHEN** 缓冲中后续记录或新遥测写入失败
- **THEN** 本次冲刷没有部分条目落盘，旧缓冲保留，新遥测不得越过旧遥测独立提交

#### Scenario: Critical bypasses degraded telemetry
- **WHEN** 已有遥测缓冲且关键记录需要提交
- **THEN** 关键记录独立提交，不先冲刷或清空遥测缓冲

### Requirement: Playback ACK segment remains Draft

决策日志 SHALL 接受 playback_ack 类别记录，但本轮不存在生产端（无 output-keeper 与真实播放 ACK）；验收注册表中 R07 MUST 保持 implemented 而非 tested，并注明回放一致性与 ACK 生产端缺口。

#### Scenario: No fabricated ACK producer
- **WHEN** 审查本轮代码
- **THEN** 不存在任何产生 playback_ack 记录的运行路径；该类别仅由存储 API 定义与测试覆盖

### Requirement: Claims about the decision log are bounded

决策日志的宣称 SHALL 限于三项。① 哈希链 MUST NOT 被宣称构成「防篡改证明」：它只能发现**部分**改动，
MUST NOT 声称能单独证明本机管理员未重写整条链（跨机留存才构成证据）。② **确定性回放**（同一录制输入）
与**重调在线模型**（结果本可不同、且产生费用）MUST 分开验收，R07 的验收项 MUST 指明是其中哪一种。
③ 业务状态的恢复 SHALL 允许「事务状态表 + 快照 + 关联日志」的**混合**，MUST NOT 绑定单一机制。
限定宣称 MUST NOT 改变 R07 的发布线（仍为 Slice-0），也 MUST NOT 移除哈希链本身。

#### Scenario: A tamper-proofness claim is made

- **WHEN** 文档、报告或对外材料把哈希链描述为「能证明未被篡改」或等价的证明性表述
- **THEN** 该表述 MUST 被订正为「能发现部分改动」；MUST NOT 作为验收通过的依据

#### Scenario: Replay consistency is claimed without naming the kind

- **WHEN** 某项验收声称「回放与原决策一致」，却未指明是确定性回放还是重调在线模型
- **THEN** 该验收项视为不可判定，MUST 先指明属于哪一种；两者 MUST NOT 用同一条验收覆盖

#### Scenario: Recovery is bound to a single mechanism

- **WHEN** 恢复设计被写成「全部业务状态由日志投影重建」这一单一机制，或据此排除事务表与快照
- **THEN** 该设计 MUST 被拒绝：投影表是恢复材料的**一部分**，不是全部
