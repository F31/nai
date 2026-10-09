## Context
Kimi 交付的哈希链/STRICT/触发器和恢复不续权机制可复用。审查发现 flush_decisions 每条独立事务、append 忽略冲刷失败后写新遥测；command 的 Mute/Takeover 仅 revoke epoch，没有清值守。

## Decisions
- 复用既有哈希前像与迁移，避免重写已提交数据。将事务内插入抽成私有函数；整批遥测（最多1024条）+新条目在同一事务内重新链接，提交成功才清缓冲。每条检查 op；失败保留旧缓冲，新遥测按既有容量规则入队。
- critical 单独事务提交，不先消耗遥测冲刷预算。critical 可以先于降级缓冲落盘；seq 表示提交顺序，created_at_ms 保留原事件记录时间，不宣称跨 critical/telemetry 的到达时间全序。
- Mute/Takeover/Stop 执行成功后清空 duty、撤销自动模式（转 Assisted）；新显式值守确认只给 lease，恢复自动仍须显式 change_mode。控制前媒体 EmergencyHandle 撤权职责不变。
- enforce_duty 达到撤权期限时先关闭 duty gate，再尝试 epoch/revision 更新；耗尽返回 HaltedPendingRevoke 但不能让后续显式确认在原 Auto 模式下恢复自动。没有运行时驱动不宣称600ms实际停声。
- 本轮不改变持久化哈希格式；哈希链是逻辑一致性校验，不能防有数据库权限者整体改写或截断，没有外部锚定。

## Verification
先添加能在旧实现失败的回归：新遥测不得超车、冲刷中途失败全回滚、缓冲+新条目原子、critical不等待缓冲、mute/takeover/stop停止自动、计数耗尽仍关闭自动。执行 storage/kernel/control-api、portable、registry、schema、OpenSpec 验证；真实模型/SDK/设备保持边界。
