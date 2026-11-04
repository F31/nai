## ADDED Requirements

### Requirement: Explicit native device smoke
系统 SHALL 只在显式设备演练入口打开本机音频和窗口，记录实际回调与窗口更新结果；缺设备或格式不支持 SHALL 明确拒绝，不生成设备完成 ACK。

#### Scenario: Native callback survives stalled window thread
- **WHEN** Windows 演练中主线程暂停超过600ms
- **THEN** 音频回调独立静音，报告观察到的零输出，不把窗口或回调提交当硬件完成

#### Scenario: Emergency mute or device unavailable
- **WHEN** P0 静音发生或输出设备不可用
- **THEN** 回调清零或报告具名不可用，失败不得保留旧成功报告
