## ADDED Requirements

### Requirement: Setup is host configuration, not a second business surface

首次设置 SHALL 只保存主机配置、运行 engine 的 provision 与启动；会话、审批、知识、守门与媒体判定 MUST 仍由同一个 ControlApi 与 engine 执行。设置界面 SHALL 在配置失败或引擎未运行时如实显示，MUST NOT 回退到 HTTP 测试适配或伪造连接状态。

#### Scenario: Setup succeeds but the engine is not running

- **WHEN** 设置已保存而 engine 启动失败
- **THEN** 界面显示具名启动错误，控制功能保持不可用；不伪造在线状态
