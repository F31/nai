# Spec delta: W138 — O02 的另一半：作业状态接上真实 journal

## ADDED Requirements

### Requirement: A job's state is read from the durable journal, not derived by the reader

`GET /api/v1/jobs/{id}`（权限 `job.read`）SHALL 从 `upgrade_journal` 读出该事务当前的步骤并原样呈现。
步骤值域 MUST 只有 `prepared`/`migrated`/`checked`/`committed`/`rolled_back` 五值，且 MUST 与该枚举的
定义处**逐字相等**。终态与否 MUST 由枚举自己的 `terminal()` 派生，MUST NOT 在控制面另写一份判定。
响应 MUST 满足它自己声明的 `JobView`（两层 `additionalProperties: false`）。

#### Scenario: A transaction that crashed after migrating

- **WHEN** journal 里该事务的 `step` 是 `migrated`，`backup_ref` 有值
- **THEN** 返回 200，`step: migrated`、`terminal: false`、`backup_ref` 为该值

#### Scenario: A committed transaction

- **WHEN** 该事务的 `step` 是 `committed`
- **THEN** `terminal: true`；MUST NOT 因为它是成功终态就省略 `step`

#### Scenario: A step the contract does not know

- **WHEN** 契约的 `step` 枚举与枚举定义处不再逐字相等（例如定义处新增一态而契约未改）
- **THEN** 判据具名失败；MUST NOT 靠一个「其它」分支吞掉未知步骤

### Requirement: A job the journal has no record of is reported as absent, and that is not the same as no job surface

查不到的作业 MUST 与「作业面未接入」分开报：前者 404 `NOT_FOUND`（该 id 不命名任何事务），
后者 503 `CAPABILITY_NOT_READY`（读出方根本没有被注入）。两者 MUST NOT 合并成同一个响应——
合并会让「我们这条事务丢了」和「我们还没有这个功能」在界面上不可区分。拒绝 MUST 可观测（有界排空），
MUST NOT 静默断开。

#### Scenario: An id that names nothing

- **WHEN** 后端已注入，而 `upgrade_journal` 里没有该 `transaction_id`
- **THEN** 404 且 `error.code` 为 `NOT_FOUND`；MUST NOT 返回 200 加一个空的步骤

#### Scenario: No reader was wired

- **WHEN** 控制面未被注入 job 后端
- **THEN** 503 `CAPABILITY_NOT_READY`；MUST NOT 退化成 404——那会把「没接」说成「没有这条」

### Requirement: A path that cannot execute refuses by name and mints no job id

`POST /api/v1/upgrades/execute` 与 `POST /api/v1/upgrades/{id}/rollback` 在没有平台安装器时
MUST 具名拒绝（503 `CAPABILITY_NOT_READY`，`category` 指明是升级面），并在 `message` 里**点名缺什么**。
此时 MUST NOT 铸造 `job_id`：一个查不到终态的 id 是伪造的可观测面。契约 MUST 声明该 503。

#### Scenario: Execute with no installer injected

- **WHEN** 调用 execute，而 `UpgradeHost` 的生产实现不存在
- **THEN** 503 且响应里**没有** `job_id`；MUST NOT 返回 202

#### Scenario: Rollback of a transaction that exists

- **WHEN** journal 里存在该事务，但回滚同样需要安装器
- **THEN** 同样具名拒绝；MUST NOT 因为「记录存在」就声称已开始回滚

#### Scenario: The refusal names what is missing

- **WHEN** 读拒绝响应的 `message`
- **THEN** 能看出缺的是平台安装器（**内容锚点**断言）；只判「非空字符串」等于没判
