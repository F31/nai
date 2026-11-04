## ADDED Requirements
### Requirement: Per provider test action and current result
配置界面 SHALL 提供已保存条目的显式付费测试动作，测试后清空该项Key，只显示身份匹配且本次产生的报告。
#### Scenario: Probe completes
- **WHEN** 操作者点击一个条目的测试按钮
- **THEN** 显示该项状态/耗时/错误码，清空该Key且不把成功描述为设备或整组认证
#### Scenario: Stale report or process failure
- **WHEN** 探针超时、进程失败或报告身份/版本不符
- **THEN** 拒绝显示旧成功，具名宿主错误与供应商失败分开
