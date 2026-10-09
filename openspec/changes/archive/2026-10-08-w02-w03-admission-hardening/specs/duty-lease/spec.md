## ADDED Requirements

### Requirement: Emergency control closes automatic admission
Session 的 Mute/Takeover/Stop 成功执行 SHALL 清除值守授权并退出自动模式；重新显式确认值守 MUST 不隐式恢复自动，仍须明确切换模式。系统值守到期撤权失败时 MUST 保持自动门控关闭，计数耗尽不得绕过此限制。

#### Scenario: Takeover while duty is valid
- **WHEN** Running/AutoSupervised 会话收到有效静音、接管或停止命令
- **THEN** 自动回复准入立即关闭，旧值守不可继续授权；重新确认值守不自动切回 AutoSupervised

#### Scenario: Revocation counter exhausted
- **WHEN** 值守到期撤权因 epoch/revision 耗尽失败
- **THEN** 自动门控保持关闭，重新确认也不能在旧自动模式下继续运行，运行时须保留独立媒体拒绝路径
