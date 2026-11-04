## ADDED Requirements

### Requirement: Window preflight only presents a complete report from its current run

宿主 SHALL 在开始预检前使旧报告失效，重叠的宿主预检 MUST 具名拒绝。无法启动、超时、报告缺失、超限或结构无效 MUST 返回 HOST_PREFLIGHT_* 故障且清理本次残留报告。宿主 MUST 有界读取并验证必要字段及封闭状态，不得填默认值掩盖缺失字段或把进程失败解释为成功；业务结论仍取自 engine，错误 MUST NOT 回显子进程原始输出。

#### Scenario: An earlier passing report exists but the new process writes nothing

- **WHEN** 旧报告存在而本次进程失败或未产生报告
- **THEN** 宿主返回具名故障，旧报告不会被展示为本次结果，连接后的读取也不能取得旧报告

#### Scenario: A new valid refusal report is produced

- **WHEN** 本次 engine 产生完整 failed/not_ready 报告并以拒绝退出
- **THEN** 宿主原样展示 all_passed=false 与逐轴原因，不把业务拒绝作为宿主故障

#### Scenario: A report is malformed or too large

- **WHEN** 报告超出 256KiB、缺必要字段、逐轴状态未知或退出结果与 all_passed 不符
- **THEN** 宿主返回 HOST_PREFLIGHT_REPORT 并清理该报告，不产生默认字段的结果

#### Scenario: A second host request arrives during preflight

- **WHEN** 本宿主已有预检尚未结束
- **THEN** 第二次请求立即返回 HOST_PREFLIGHT_BUSY，不启动第二个进程也不删除进行中报告
