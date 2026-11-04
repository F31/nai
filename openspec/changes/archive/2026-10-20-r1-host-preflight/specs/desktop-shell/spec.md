## ADDED Requirements

### Requirement: Preflight can be run from the window without a command line

宿主 SHALL 使用已保存的非机密设置运行 `nai-engine --preflight`，并在显式请求时启用受限连通探针。结果 MUST 来自引擎报告（`all_passed` 与逐轴状态），宿主 MUST NOT 重新判定。进程失败、超时或报告缺失 MUST 返回具名故障，MUST NOT 与“预检拒绝”混为一谈。预检 MUST NOT 阻塞连接，也 MUST NOT 把 `not_ready` 呈现为通过。

#### Scenario: Operator runs preflight before connecting

- **WHEN** 操作者在设置面板运行预检
- **THEN** 界面逐轴展示 passed/failed/not_ready；报告写入数据根，连接后既有面板读取同一份

#### Scenario: Preflight process fails

- **WHEN** engine 无法启动、超时或未写出报告
- **THEN** 返回 `HOST_PREFLIGHT_*` 具名故障，界面不展示为预检结论
