# Spec Delta

## Purpose
为本机操作员提供显式启动且可真实连接的受控状态读取与撤权入口，使用当前 Session 状态和独立媒体撤销信号形成桌面控制闭环；保持身份、设备、会话作用域与重试语义，明确区分控制状态执行和真实设备播放/静音确认。

## ADDED Requirements

### Requirement: Protected bounded local access
本轮服务 SHALL 仅监听 loopback，身份令牌须有作用域、权限、期限和撤销检查；token MUST 不出现在 URL/日志/持久化。输入、并发和连接期限 SHALL 有界，未知路由不得伪造成功。

#### Scenario: Invalid identity or scope
- **WHEN** token 缺失/错误/过期/撤销，权限不足或目标会话不匹配
- **THEN** 读取/命令拒绝，不泄露会话状态或变更输出

### Requirement: Honest live status and command result
读取 SHALL 返回当前 Session 的精确 uint64 字符串及真实生命周期/模式/媒体状态；没有插件/设备时不报告 Ready。命令 ACK 的 executed SHALL 仅在控制状态已执行时为 true，不当作硬件静音/播放 ACK。

#### Scenario: Empty runtime
- **WHEN** 本机控制进程只有新建 Session、没有实际媒体设备或插件
- **THEN** 状态保持 created/standby，并明确真实媒体未连接

### Requirement: Safe idempotent emergency command
mute/takeover SHALL 在鉴权、权限、作用域及结构检查后先撤销独立媒体信号，再更新 Session。新命令须验证 epoch/revision/TTL 和绑定设备的单调 seq；同 ID 同内容重试返回原 ACK，改内容或过量记录拒绝。状态锁忙/CAS 失败 MUST 不恢复媒体输出。

#### Scenario: Busy or stale command
- **WHEN** 合法撤权命令到达而状态锁忙或 revision 已变
- **THEN** 媒体信号仍先撤销，状态更新返回明确失败，不返回已执行 ACK

#### Scenario: Replay
- **WHEN** 重试已执行命令或使用旧 seq 的新 ID
- **THEN** 前者返回原 ACK，后者拒绝，不重复推进 epoch

### Requirement: Explicit desktop connection
桌面 SHALL 明确区分未连接、连接中、已鉴权状态与错误，token 仅内存保存。命令失败/断连后不得按成功更新界面或自动重发；关闭 UI 不隐式 Stop 会话。

#### Scenario: Response missing
- **WHEN** 读取或命令返回网络/鉴权/结构错误
- **THEN** 显示错误并关闭依赖旧状态的操作，需显式刷新/连接获得真实状态
