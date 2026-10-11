## ADDED Requirements

### Requirement: A gate-failure summary surfaces signals, not filler

失败摘要 SHALL 以**指向失败的行**为中心：编译器与测试框架的错误报告、其紧邻的失败详情，
以及失败所属的**测试目标**。MUST NOT 用日志里的无关行把配额填满。判定一行是否为信号的依据
SHALL 是「该行是否报告了失败」，而 MUST NOT 是「该行是否含某个关键词」——测试名、依赖名都可能含信号词。

#### Scenario: A passing test whose name contains a signal word is not a signal

- **WHEN** 某测试**通过**，但其名字里含信号词（例如 `…_unimplemented_routes_…`）
- **THEN** 该行 MUST NOT 被判为失败信号，MUST NOT 因其把后续无关行带入摘要

#### Scenario: Scarcity is preserved over fullness

- **WHEN** 整份日志只有一条真正指向失败的行
- **THEN** 摘要 SHALL 以该行为中心输出少量注解；MUST NOT 用无关行补足到条数上限而把该行淹没

#### Scenario: Failure details travel with the signal

- **WHEN** 日志含失败测试的汇总段（失败名列表与各自的标准输出）
- **THEN** 摘要 SHALL 连带其紧邻的详情（断言消息、失败名）以及失败所属的**测试目标头**

### Requirement: An incomplete capture is declared as such

失败摘要的来源日志若明显不完整——判据是「出现测试框架的逐测试行，却缺少其汇总行」——
诊断 MUST 在注解中**明示**该不完整并给出日志总行数。MUST NOT 只罗列出现过的行而不提这份日志并不完整；
否则读者会把一份被截断的日志当成全部证据。

#### Scenario: Truncated capture is declared

- **WHEN** 采集到的门控日志含逐测试行，但没有 `test result:` 汇总行
- **THEN** 摘要中出现一条警示，说明日志在汇总行之前结束，并给出总行数

#### Scenario: A complete capture is not flagged

- **WHEN** 采集到的日志含 `test result:` 汇总行
- **THEN** MUST NOT 出现「不完整」警示
