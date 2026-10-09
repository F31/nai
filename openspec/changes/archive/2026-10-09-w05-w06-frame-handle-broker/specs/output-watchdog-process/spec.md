# Spec Delta

## Purpose

让 keeper 具备帧消费与待机切换路径：在租约、epoch、池状态与完整性任一不满足时切待机并保持静音通道，而不产生任何设备输出或播放 ACK。

## ADDED Requirements

### Requirement: Keeper frame consumption with standby semantics

keeper SHALL 仅在有效租约下消费已提交帧；租约失效、epoch 陈旧、池退休或完整性失败 MUST 进入待机并停止投递。待机状态 MUST 通过状态消息可观测，且 `output_available` MUST 保持 false。

#### Scenario: Frame presented under valid lease
- **WHEN** 租约有效且收到通过校验的已提交帧
- **THEN** keeper 消费该帧并报告 presenting=true

#### Scenario: Lease lost switches to standby
- **WHEN** 已投递帧后租约到期或被撤销
- **THEN** keeper 停止投递、报告待机原因（LeaseLost）且不产生播放 ACK

#### Scenario: Retired pool switches to standby
- **WHEN** 收到 peer 错误或池已退休后仍有帧事件
- **THEN** keeper 保持待机并拒绝对应帧

### Requirement: Explicit frame-drill activation

帧路径 SHALL 仅在显式 `--frame-drill` 下启用；池文件通过继承的标准流失交付并由 keeper 只读重开，MUST NOT 通过控制总线传帧数据或 Base64。未启用时 keeper MUST 不建立任何帧路径。

#### Scenario: Drill not selected
- **WHEN** keeper 以普通 watchdog 模式启动
- **THEN** 不存在帧消费者，提交帧请求被拒绝且不产生 presenting 证据
