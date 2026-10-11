## ADDED Requirements

### Requirement: Upgrade cost is listed per component, and an unknown component is not rendered as zero

桌面 SHALL 把升级成本**逐项列示**（云资源、下载、存储、预估运行增量）。无法估算的分项 MUST 呈现为
**未知及其原因**，MUST NOT 呈现为 0；总额若含未知分项，MUST 呈现为**下界加未知计数**，MUST NOT 呈现为总额。
载荷 MUST 被严格解析：缺字段、未知字段、非法金额或空原因 MUST 导致**具名拒绝**，不得静默猜补。

本需求只保证**呈现口径**；真实升级界面的渲染、控制面暴露 plan/journal 与生产接线
**不在**其内，故它不构成 O05 的完整 UI 验收。

#### Scenario: An unknown component

- **WHEN** 某分项被声明为未知并给出原因
- **THEN** 该行呈现为「未知（原因）」，且不出现数字 0

#### Scenario: A partially known total

- **WHEN** 部分分项已知、部分未知
- **THEN** 总额呈现为下界加未知计数，且明确**不是**总额

#### Scenario: A malformed payload

- **WHEN** 载荷缺少必需字段、含未知字段、金额不是非负安全整数或原因为空
- **THEN** 解析具名失败，呈现层不产出任何行
