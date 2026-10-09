## MODIFIED Requirements

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
