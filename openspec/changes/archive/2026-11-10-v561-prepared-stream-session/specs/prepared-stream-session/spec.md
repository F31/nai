## ADDED Requirements
### Requirement: 单次真实准备及模型复用
音画装配 SHALL 复用同一个已认证GPU worker，准备耗时与每句业务耗时分开，准备本身不得暗中调用TTS。
#### Scenario: 连续两句
- **WHEN** 一次准备后显式播放两句
- **THEN** 使用同一模型加载、递增task和epoch，各句独立校验EOS及设备完成，不再次预热。
### Requirement: 停止与故障不复活旧任务
停止 SHALL 取消当前父操作、清退后再允许显式新句；损坏实例不得自动重建或报告可用。
#### Scenario: 停止后恢复
- **WHEN** 当前句停止后用户显式提交新句
- **THEN** 新epoch可在干净实例上播放，旧帧/旧时钟不得参与新句，若实例损坏则拒绝。
