## ADDED Requirements
### Requirement: Observable native engine startup
桌面 SHALL 对启动操作即时展示阶段，并保留受控宿主错误码与可执行恢复建议；不得把原生字符串拒绝吞成无细节的失败，不得以加载页面代替实际连接成功。
#### Scenario: Native startup rejection
- **WHEN** 启动返回具名字符串错误
- **THEN** 工作台显示失败与对应恢复入口，解除忙碌且不自动重试、不调用付费API
#### Scenario: Real engine connection
- **WHEN** 引擎启动成功且控制状态查询成功
- **THEN** 才显示已连接并开放基于真实会话的操作

#### Scenario: Pending request crosses the native decoder
- **WHEN** WebView提交合法seq=0待发送请求
- **THEN** 宿主验证完整形状后交给唯一actor分配线上序号；前端指定线上序号或包含认证令牌时拒绝
