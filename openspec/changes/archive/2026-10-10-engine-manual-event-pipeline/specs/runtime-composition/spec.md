## Purpose
在引擎装配中补齐「手工事件 → 知识 → 智能体 → 守门 → 文本生成 → 决策日志」的可验证编排，并保持无设备、无真实 Provider、无播放 ACK 的诚实边界。

## ADDED Requirements
### Requirement: Manual event to decision-log pipeline
engine SHALL 接受显式标注为手工来源的事件，并按「归一化/去重 → 知识检索 → 智能体决策 → 守门四阶段 → 文本生成 → 决策日志」顺序编排。每一步 MUST 在进入下一次对外调用前复检 session、generation epoch、值守租约与审批对象哈希。守门缺失或未通过时 MUST NOT 外发；无知识命中 MUST 显式记录而非静默继续。

#### Scenario: Manual event produces a logged decision
- **WHEN** 手工事件通过守门且文本生成成功
- **THEN** 决策日志写入可校验条目，记录事件、路由、守门结果、模型标识与 usage，且哈希链校验通过

#### Scenario: Stale epoch or missing guard
- **WHEN** 事件携带陈旧 epoch、值守失效或守门不可用
- **THEN** 不发生对外模型调用，记录 critical 条目并进入撤权路径

### Requirement: Deterministic rehearsal replay for the decision log
engine SHALL 支持以确定性 rehearsal 生成器对同一手工事件重放，并与原次决策日志逐项比对。两次结果不一致 MUST 视为判据不达成并如实报告。

#### Scenario: Replay matches the original decision
- **WHEN** 同一事件集与同一确定性生成器重放
- **THEN** 路由、守门结论、生成文本与 usage 与原次一致，比对通过

### Requirement: Orchestration boundary and failure ordering
编排 SHALL 位于 app 层（`apps/engine`），MUST NOT 引入 kernel→runtime 或 plugin→plugin 依赖。链路任一阶段失败 MUST 依次执行「P0 静音 → 撤销输出授权（epoch 递增）→ critical 决策日志 → 有界退出」；决策日志写入失败 MUST NOT 回退已执行的静音。

#### Scenario: Injected stage failure
- **WHEN** 演练注入使知识、守门、生成或存储阶段失败
- **THEN** 引擎先静音再撤权并写 critical 日志，随后有界退出，不产生成功结果行
