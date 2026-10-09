# Spec Delta

## ADDED Requirements

### Requirement: Engine-hosted control service

loopback 控制服务 SHALL 可由 `apps/engine` 以真实装配（存储 owner、共享会话、审批后端、决策日志）
启动，替代独立 example 的临时会话形态。引擎形态 MUST 保持既有全部不变量：显式 loopback、Bearer
临时凭据、权限粒度、有界连接与载荷、未知操作 404。

#### Scenario: Desktop client against real engine
- **WHEN** 桌面 `ControlClient` 以有效 token 连接引擎控制地址
- **THEN** status/mute/takeover/approvals 行为与既有契约一致，且 mute 后共享会话 epoch 递增、
  决策日志写入 critical 记录

#### Scenario: Revoked token after shutdown
- **WHEN** 引擎进入停机流程
- **THEN** 控制 token 被撤销（进程内验证），服务随后停止监听并以 0 退出

#### Scenario: Control revoke is visible to the shared session
- **WHEN** 桌面客户端通过引擎执行 mute/takeover
- **THEN** 同一会话实例的 epoch 递增对装配侧立即可见：引擎随后签发的租约与记录使用新 epoch，
  且旧租约不再被接受
