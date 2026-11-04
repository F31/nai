## ADDED Requirements

### Requirement: Native and HTTP adaptations share one control judgement

原生继承管道与 loopback HTTP SHALL 调用同一个 ControlApi 路由与业务处理器，MUST NOT 在壳内复制权限、审批、epoch 或幂等规则。原生客户端 SHALL 复用现有响应解析与控制 ACK 判据。未配置、启动失败和 engine 退出 MUST 显示具名不可用状态，不得回退到测试 HTTP 或假数据。

#### Scenario: Native command uses the existing approval and session guards

- **WHEN** 原生请求缺权限、会话代次过期或审批对象不符
- **THEN** 同一业务处理器给出与 HTTP 一致的拒绝，控制 ACK 不被读成硬件 ACK

#### Scenario: Native engine cannot start

- **WHEN** 缺失场景、数据根不可用或固定侧车不能启动
- **THEN** 原生界面显示实际失败，不监听 HTTP、不合成成功响应
