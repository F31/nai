# output-device-binding Specification

## Purpose
为单次私有原生音频输出提供可见设备快照、会话绑定选择及实际设备对象的固定使用，在选择和初始化期间保持无音频提交，拒绝旧选择、迟到租约和自动回退，保留默认一键播放的易用性。

## Requirements

### Requirement: 单次设备对象绑定
系统 SHALL 在独立worker内保留设备对象，并将选择绑定到本次output session；选择后直接打开该对象，禁止按名称重查或自动回退其他设备。
#### Scenario: 设备不可用
- **WHEN** 选定设备断开或格式不可用
- **THEN** 停止/Unavailable，保留真实原因与实际目标，不切到另一设备

### Requirement: 选择阶段不发声且受停止约束
系统 SHALL 在选择前不提交音频，并对选择阶段执行有界期限、当前source复核与短租约；默认路径可选系统默认快照对象，手动路径须显示设备并明确点播。
#### Scenario: 旧选择请求迟到
- **WHEN** output session已更换或到期
- **THEN** 拒绝旧device引用，不恢复旧输出，不宣称公共输出或硬件播放完成
