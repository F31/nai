# Spec delta: W144 — 作业面闭世界

## ADDED Requirements

### Requirement: Every operation that accepts a job is registered, and each registered entry answers exactly one of two questions

`202` 响应体为 `JobAccepted` 的操作集 SHALL 与 `JOB_SURFACES` 的登记集**双向相等**：每一条这样的操作
MUST 在册，在册的每一条 MUST 真的是这样的操作。在册项 MUST **恰有** `kind` 或 `blockedBy` 之一，
两者都写或都不写都 MUST 具名失败。`blockedBy` MUST 落在闭集 `no_handler` / `refuses_without_installer` 内。
响应体 MUST 是 `$ref #/components/schemas/JobAccepted`，MUST NOT 是内联对象。

#### Scenario: A new route that mints a job id without being registered

- **WHEN** 契约里新增一条 `202 + JobAccepted` 的操作，而 `JOB_SURFACES` 未登记它
- **THEN** 判据具名失败并指出该操作；MUST NOT 因为「它在契约里合法」而放行

#### Scenario: A registered entry whose operation no longer accepts

- **WHEN** 在册项对应的操作不再返回 `202`
- **THEN** 判据具名失败（一条没有对象的登记）；counts 相等 MUST NOT 被当作集合相等

#### Scenario: An entry that answers neither question

- **WHEN** 在册项既没有 `kind` 也没有 `blockedBy`
- **THEN** 判据具名失败；MUST NOT 把「不知道该怎么办」当成一个合法的默认值

#### Scenario: An entry that answers both

- **WHEN** 在册项同时声明 `kind` 与 `blockedBy`
- **THEN** 判据具名失败；MUST NOT 由读取顺序决定信哪一句

#### Scenario: A blocker outside the closed set

- **WHEN** `blockedBy` 的取值不在闭集内
- **THEN** 判据具名失败并列出闭集；一个没有名字的理由与一个没看过的理由 MUST NOT 不可区分

### Requirement: A kind names a state source the repository actually has, and the contract exposes no kind without one

`JobView.kind` 的枚举 SHALL 与 `JOB_KINDS` 的键集**双向相等**。每个 `kind` MUST 在仓库里有一份
被跟踪的 journal 与一个被跟踪的 reader，且两者的锚点文本 MUST 仍在原处。
有 `kind` 而未声明为已实现 MUST 具名失败；已声明实现却声称 `blockedBy` 同样 MUST 具名失败。

#### Scenario: A contract kind with no journal

- **WHEN** `JobView.kind` 列出一个 `JOB_KINDS` 没有声明状态来源的取值
- **THEN** 判据具名失败；MUST NOT 让那条契约分支到不了任何地方

#### Scenario: A journal the contract cannot name

- **WHEN** `JOB_KINDS` 声明了一个 `JobView.kind` 未列出的取值
- **THEN** 判据具名失败；一张没有人能报出来的表 MUST NOT 被当成可用状态来源

#### Scenario: The reader stops reporting the kind

- **WHEN** 读者侧那段把该 `kind` 写出来的源码文本不在了（改名或搬走）
- **THEN** 判据具名失败；锚点 MUST 带判别键（`"kind": "<值>"`）而不是裸的值词，
  否则同一个文件里别处的同名字符串会让它永远不变红

### Requirement: An accepted job is not a job that is known to be progressing

`JobAccepted.status` 的取值域 SHALL 恰好是 `accepted` 一个值。`accepted` MUST 只表示「命令已被接收」，
能否轮询 MUST 由 journal（`JobView.kind`）决定，MUST NOT 由该响应体决定。

#### Scenario: A second status value

- **WHEN** `JobAccepted.status` 的枚举多出一个取值（例如 `queued` 或 `running`）
- **THEN** 判据具名失败；MUST NOT 让一条没有 journal 的路由靠响应体自称「进行中」

#### Scenario: A 202 with no body schema

- **WHEN** 某个操作的 `202` 没有 `application/json` 响应体
- **THEN** 判据具名失败；客户端连 job id 都拿不到的响应 MUST NOT 被当成作业面
