# Spec delta: W146 — 作业状态的写入点、安装器名册与「能轮询 ⇔ 有安装器」

## ADDED Requirements

### Requirement: A kind's journal has exactly one writer, and the declaration names that file

每个 `JobView.kind` 取值 SHALL 指名一个写入点：文件、逐字锚点、以及那次写入的**逐字语句**。
该语句 MUST 在已跟踪的源码与迁移文件里**恰好出现一次**，且 MUST 位于被声明的文件里。
一次都不出现 MUST 具名失败；出现多于一次 MUST 具名失败。

#### Scenario: The write disappears

- **WHEN** 被声明的写入语句在跟踪树里一次也找不到
- **THEN** 判据具名失败并说明「这个 kind 的 journal 永远收不到行，它的每个 `/jobs/{id}` 都是构造性的 404」；
  MUST NOT 读成「这个 kind 没有写入者」这种与「没扫到」不可区分的结论

#### Scenario: A second write path appears

- **WHEN** 同一个写入语句在跟踪树里出现第二处（例如某个文件直接写库，绕过 journal）
- **THEN** 判据具名失败并列出所有命中文件；「谁写了这一行」MUST NOT 有两个答案

#### Scenario: The declaration points at a file that does not write

- **WHEN** 被声明的文件里没有那句写入，而另一个文件里有
- **THEN** 判据具名失败并给出真实的写入文件

#### Scenario: The anchor is only a declaration prefix

- **WHEN** 写入锚点是「声明前缀」（例如只锚 `fn record_x` 而不带调用形态）
- **THEN** 该锚点在函数被改名后仍然命中，判据 MUST NOT 继续绿；锚点 MUST 带调用形态，
  这一条由用例的结构断言守住

#### Scenario: The scan domain is blind rather than empty

- **WHEN** 扫描域里没有一个可读的源文件，或某个源文件读不出来
- **THEN** 判据分别具名失败；「扫不到」MUST NOT 与「不存在」共用同一个信号

### Requirement: Every UpgradeHost implementation is on the roster, and the roster says whether it is production

仓库里每个 `impl UpgradeHost for X` MUST 在安装器名册里占一行，名册的每一行也 MUST 对应一个真实
实现（双向相等）。每行 MUST 声明 `production` 的**布尔**取值与一句非空理由；`production` MUST NOT
由实现者的名字推断。

#### Scenario: An unregistered installer

- **WHEN** 仓库里出现一个 `impl UpgradeHost for X` 而名册没有 `X`
- **THEN** 判据具名失败；「生产安装器落地了而登记表没动」正是本轮要防的那件事

#### Scenario: A roster entry with no implementation

- **WHEN** 名册声明了一个仓库里没有任何实现的实现者
- **THEN** 判据具名失败；名册 MUST NOT 描述一个不存在的世界

#### Scenario: Production is unknown

- **WHEN** 某行的 `production` 不是布尔值（缺失、`null`、字符串）
- **THEN** 判据具名失败；「不知道」与「不是生产」是两件事，而下面那条成对规则读的就是这个值

#### Scenario: The roster is emptied

- **WHEN** 名册被清空
- **THEN** 判据具名失败；「没有生产安装器」MUST NOT 用一张空表来表达

### Requirement: A pollable job surface requires a production installer, and a production installer requires a pollable surface

登记表里任何声称 `kind` 的作业面 MUST 同时存在至少一个 `production: true` 的安装器；
反过来，名册里存在生产安装器时，登记表 MUST 至少有一条声称 `kind` 的作业面。两侧任一单独变动
MUST 具名失败。

#### Scenario: A surface claims a kind with no installer

- **WHEN** 某条作业面从 `blockedBy` 改成 `kind`，而名册里没有生产安装器
- **THEN** 判据具名失败并说明「没有任何东西会推进这些事务，它们的 journal 行永远不会被写」；
  「你能轮询这个作业」MUST NOT 在没有任何写入者时成立

#### Scenario: An installer lands with no pollable surface

- **WHEN** 名册里出现一个生产安装器，而登记表仍说每条作业都被挡着
- **THEN** 判据具名失败；要么登记表过期了，要么那个安装器建好了却没用

#### Scenario: The pair is empty on both sides today

- **WHEN** 真实仓库此刻 0 条 `kind` 作业面、0 个生产安装器
- **THEN** 该规则不如实改变任何结论，其判别力 MUST 由合成世界的两个反例证明；
  MUST NOT 记作「已由真实仓库验证」
