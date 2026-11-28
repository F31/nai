## ADDED Requirements
### Requirement: 同上下文有界音频批次
Root SHALL 只合并同一不可混淆执行上下文中的已验证源，保留各源摘要/精确样本范围并绑定合并摘要，最多两源及既有音频/像素限额。
#### Scenario: 不同上下文或取消
- **WHEN** 输入来自不同Context或父Operation失效
- **THEN** 封存/生成/采用 MUST 拒绝，不发布部分批次或重用旧前缀
#### Scenario: 同一上下文连续输入
- **WHEN** 两段源均合法并已准备
- **THEN** worker SHALL 用单个Render消费精确拼接字节，仅整体末尾补齐生成块
### Requirement: 如实区分批次生成和实时能力
诊断 SHALL 保留候选/保留/尾帧及耗时，批次取消作用于全部依赖帧。
#### Scenario: 补尾下降但供给仍不足
- **WHEN** 合并减少生成块但耗时仍长于音频时长
- **THEN** 实时状态 MUST 仍未通过，不以预准备或未确认画质标Ready
