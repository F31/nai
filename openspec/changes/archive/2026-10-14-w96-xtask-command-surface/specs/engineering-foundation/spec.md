## ADDED Requirements

### Requirement: Every cargo xtask command is declared with its gate coverage

`cargo xtask` 的每一条命令（判定单元 SHALL 是**签名**，即空格连接的顶层字面量序列，而非命令名）
SHALL 在一个在册登记表里有且只有一条登记，并 SHALL 归入一个闭集类别之一：`gate`
（执行体在本机门控闭包里被执行）、`standalone`（是判定命令但不在门控闭包里）、`artifact`
（生成产物、不产生判定结论）、`help`（不产生动作的入口）。`gate` 的每条 SHALL 引出一处
真的存在的门控落点并证明它逐字落在门控闭包内；`standalone` 的每条 SHALL 给出成因而成因
SHALL 取自一个闭集。`Usage:` 字符串 SHALL 与分发集合对账：它 MUST NOT 声称一条不存在的命令，
且每一条非 help 命令 SHALL 出现在它里面。新增、删除或改名一条命令 SHALL 使检查失败并具名
指出该命令。

#### Scenario: An unregistered or vanished command fails by name

- **WHEN** 往 `execute()` 插入一条没有登记的臂，或删掉一条已登记的臂
- **THEN** 检查失败，且错误里具名指出该命令的签名

#### Scenario: Claiming local gate coverage has to pay for itself

- **WHEN** 一条 `gate` 命令引出的门控落点不在门控闭包内，或该落点所指的文件没有被提供给判据
- **THEN** 检查失败并具名该命令——「本机门控真的跑它」不是可以空口说的话

#### Scenario: The usage string must not drift away from the dispatch

- **WHEN** `Usage:` 字符串声称一条 `execute()` 里不存在的命令，或漏写一条非 help 命令
- **THEN** 检查失败并具名那条命令

#### Scenario: A reason outside the closed set is a label, not a reason

- **WHEN** 一条 `standalone` 命令给出的成因不在闭集里，或它同时又挂了一处门控落点
- **THEN** 检查失败并具名该命令
