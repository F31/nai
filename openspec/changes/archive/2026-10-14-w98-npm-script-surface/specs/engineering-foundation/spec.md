## ADDED Requirements

### Requirement: Every package script has a named disposition, and its consumer is computed

`package.json` 的每个脚本 SHALL 有具名处置：或引出一处**真调用**（`{file, marker}`，在剥离注释后的
引用扫描面里逐字存在），或落入**成因闭集**并逐格付代价。判定单元 SHALL 是「包目录 + 脚本名」的配对，
MUST NOT 是裸脚本名。**文档里的命令 SHALL NOT 充当消费者**，**自引用 SHALL NOT 充当消费者**。
驱动表的每一项 SHALL 落在该包脚本上，且其首个 token SHALL 在门控认得的可执行名集合内。
声明面与台账 SHALL 双向一致；引用不可解析、成因落域外、证据不逐字存在或输入缺失
SHALL 具名失败而非静默降级。

#### Scenario: A script nobody calls is named

- **WHEN** 某个 `package.json` 里新增（或原有）一个脚本，而没有任何真调用点
- **THEN** 检查失败并具名该「包目录 + 脚本名」，要求它给出成因与代价

#### Scenario: A driver step that names an unknown script fails

- **WHEN** 驱动表里出现一个 `(包目录, 脚本名)` 在该包 `scripts` 里不存在，或首个 token 不在门控认得的可执行名内
- **THEN** 检查失败并具名该项与那个 token——不得等到真跑前端门控才以运行时错误暴露

#### Scenario: Neither a document nor the script itself is a consumer

- **WHEN** 一处「消费者」证据落在 `.md` 文档里，或落在这个脚本自己的身体里（自引用）
- **THEN** 检查失败并具名该证据——文档是给读者的指引，自引用是恒真的自我满足
