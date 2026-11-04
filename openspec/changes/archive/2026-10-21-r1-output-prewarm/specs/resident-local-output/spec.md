## ADDED Requirements
### Requirement: Explicit neutral output prewarm has no speech authority
桌面 SHALL 提供显式静默预热，复用当前引擎准入的合法闭嘴素材，打开常驻窗口与单条静音音频stream；MUST NOT 调用Provider、构造候选审批或签发语音租约。媒体字节不得进入控制总线。预热忙碌、设备不可用、许可/摘要/期限/epoch失效时 SHALL 如实失败，不以控制ACK表示就绪。
#### Scenario: Silent preparation succeeds
- **WHEN** 有效会话显式预热且设备确认零语音样本、单stream和匹配待机摘要
- **THEN** 界面才显示静默待机，后续真实审批播放复用该进程和stream，不产生硬件完成ACK
#### Scenario: Tampered or revoked preparation
- **WHEN** 媒体描述符/像素被篡改或预热时epoch变化
- **THEN** 拒绝该任务并有界回收未确认输出，不恢复语音或自动重试
#### Scenario: Replay and concurrent preparation
- **WHEN** 相同命令重放或已有语音任务占用输出
- **THEN** 重放不再次执行，忙碌不进入无界等待；立即静音仍可执行
