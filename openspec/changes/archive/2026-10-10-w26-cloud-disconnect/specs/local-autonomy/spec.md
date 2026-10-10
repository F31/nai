## Purpose

把 A10 的两条否定式判据从声明变成可执行：**本地实时链路与本地静音 MUST NOT 依赖控制云**（云的缺席 MUST NOT 改变它们的可用性），**低云容量 MUST NOT 写未经测量的数字**（容量只可从申报实例与负载下的测量构造，超预算 MUST 报为限流而非静默吸收）。云能力的失效 MUST 按其自身门控失败关闭，链路恢复 MUST NOT 自动恢复旧的 Auto 状态。本 capability 不授予真实云 SDK、真实断网/弱网注入或多云故障转移的认证。

## ADDED Requirements

### Requirement: Local critical path has no cloud dependency

本地关键链路（实时链路与本地静音）MUST 以**依赖方向图**表达，且每个关键节点的**传递闭包** MUST NOT 含任何云节点。直连依赖与经中间节点到达的依赖同属违规。审计 MUST 以稳定标记区分「未声明」「关键节点被声明为云节点」与「本地关键节点依赖云」三类，MUST NOT 静默通过。

#### Scenario: A purely local critical path passes
- **WHEN** 关键节点的依赖闭包只含本地节点
- **THEN** 审计无违规，且每个关键节点的云可达集合为空

#### Scenario: A two-hop cloud dependency is caught
- **WHEN** 某个本地关键节点经另一本地节点间接依赖一个云节点
- **THEN** 审计报告「本地关键节点依赖云」，并同时指出链路两端的节点

#### Scenario: Undeclared and misclassified nodes are distinct violations
- **WHEN** 关键集合里包含一个未声明的节点，或一个被声明为云节点的节点
- **THEN** 审计分别报告「本地关键节点未声明」与「本地关键节点不是本地」，而不是同一类错误

### Requirement: Availability follows node class, and recovery does not resume Auto

可用性 MUST 是链路状态与节点归属的函数：本地节点在链路可达与不可达下**都** MUST 可用；云节点在断连时 MUST 失败关闭并以稳定标记说明原因。断连 MUST 只摘除**云**能力，MUST NOT 触碰本地能力。重连后云能力 MUST 进入 Prepare 准入（`NeedsPrepare`），MUST NOT 自动回到运行态。

#### Scenario: The local mute survives a cloud outage
- **WHEN** 云链路不可达，且本地静音节点属于本地关键链路
- **THEN** 本地静音的可用性仍为可用，且其门控状态不被改变

#### Scenario: Disconnect spares local capabilities
- **WHEN** 一个本地能力与一个云能力都处于运行态，随后链路断开
- **THEN** 只有云能力被摘下，本地能力保持运行

#### Scenario: Reconnect parks cloud capabilities
- **WHEN** 云链路恢复
- **THEN** 云能力进入 Prepare 准入而不是运行态，只有一次显式激活才能使其运行

### Requirement: Low-cloud capacity comes from a measurement

低云容量 MUST 只从带**申报实例数与负载**的测量构造；未测量的容量声明 MUST 被拒绝，MUST NOT 使用默认值或零值代替。容量与预算的比较 MUST 逐项进行并报出**全部**越界族，MUST NOT 只报第一项。越界时 MUST 限流；选择「吸收」越界的配置 MUST 被审计报告为违规。

#### Scenario: An unmeasured capacity is refused
- **WHEN** 声明容量时标注为未测量
- **THEN** 声明被拒绝，且错误码表明能力未就绪

#### Scenario: A measurement must name instances and load
- **WHEN** 测量没有实例数，或负载不在 1..=100
- **THEN** 声明被拒绝为参数非法

#### Scenario: Every breach is reported
- **WHEN** 容量同时超过连接、内存与延迟三项上限
- **THEN** 判定为限流并列出三项越界，而不是只列第一项

#### Scenario: Absorbing a breach is a violation
- **WHEN** 配置在越界时的策略是吸收而不是限流
- **THEN** 审计以稳定标记报告该违规
