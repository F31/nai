## ADDED Requirements

### Requirement: A control route exposes the latest preflight report

`GET /api/v1/sessions/{id}/preflight`（`session.read`）SHALL 返回最近一次 `--preflight` 写出的
报告（逐轴状态、退出结论、运行时间）；尚未跑过预检或读取不到报告 MUST 返回 `CAPABILITY_NOT_READY`
并说明原因，MUST NOT 伪造一份空报告冒充已检查。报告载荷 MUST NOT 声称已播放、已接设备或
PlaybackAck。

#### Scenario: The client reads the latest preflight report

- **WHEN** 桌面在 engine 运行后请求预检路由
- **THEN** 返回最近预检的逐轴状态与整体结论，界面明确标注未播放、未接设备

#### Scenario: No preflight has run

- **WHEN** 数据根下没有预检报告
- **THEN** 返回 `CAPABILITY_NOT_READY` 并说明「预检尚未运行」，不提供空报告

### Requirement: The desktop shows preflight status honestly

桌面 SHALL 提供「预检」呈现：无报告时写明「尚未预检」而非绿灯；每轴状态与原因原样呈现；任何
成功状态 MUST 带有「未播放、未接设备」字样，MUST NOT 把预检通过读作播出或设备验收。

#### Scenario: Desktop shows a preflight result

- **WHEN** 桌面收到预检报告
- **THEN** 逐轴显示状态与原因，整体结论旁的说明字段含未播放/未接设备标注
#### Scenario: Desktop shows no preflight

- **WHEN** 路由返回 `CAPABILITY_NOT_READY`
- **THEN** 界面显示「预检尚未运行此命令」，不显示模拟通过状态