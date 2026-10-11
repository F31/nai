# versioned-contracts 变更（w52）

## MODIFIED Requirements

### Requirement: Single contract source

跨语言消息 SHALL 由 Protobuf 定义并生成 Rust/TS/C++；配置和清单 SHALL 由 JSON Schema 定义，HTTP SHALL
由 OpenAPI 定义。能力目录 SHALL 为**每一条**声明阶段 `stage`：`draft` 或 `frozen`。

**只有 `frozen` 条目**引用正式请求/响应消息并生成消费者类型（消息类型表、会话消息表与协商能力清单）；
目录 MUST NOT 存在无法解析的正式消息类型。**`draft` 条目 MUST NOT 声明** `request`、`response`、
`stream` 或 `session_scoped`——半声明的 draft 会让阶段字段失去意义，等于把「已冻结」与「未冻结」混为一谈。

阶段语义的判据与偏差 MUST 被记录：目录的「完整模型」（类别、必需性、首次出现的切片、缺失时的降级行为）
若尚未落地，MUST NOT 被表述为已完成。

#### Scenario: Capability references missing message

- **WHEN** 能力目录出现重复 ID、缺失类型，或 `frozen` 条目缺少或无正式请求/响应 Schema
- **THEN** 契约检查失败，不报告目录完整

#### Scenario: Draft entry claims consumer messages

- **WHEN** 一个 `stage: draft` 的条目声明了 `request`、`response`、`stream` 或 `session_scoped`
- **THEN** 目录校验失败，且该 id MUST NOT 出现在协商能力清单或消息类型表中

#### Scenario: Draft entry stays out of the consumer surface

- **WHEN** 目录中存在 `stage: draft` 的条目，且有切片验收引用该契约
- **THEN** 生成的消息类型表、会话消息表与协商能力清单中 MUST NOT 出现该 id；
  它 MUST 单独可见（可列出但不可协商），且该引用 MUST NOT 被当作「已冻结」或使任何验收升级为通过

## ADDED Requirements

### Requirement: Registry contract references resolve

`tests/registry.yaml` 中每条验收的 `contract` 取值 SHALL 要么命中能力目录中的条目（**任意阶段**——
draft 也算已声明），要么命中**具名的非能力契约白名单**。白名单 MUST 保持极小，且每项 MUST 就地附上
「为什么它不是能力」的理由。`contract` 字段缺失 SHALL NOT 视为错误。门控 MUST 对不可解析的取值报错，
且 MUST NOT 因为「目录里暂时没有」而静默通过。

#### Scenario: Unresolvable contract reference

- **WHEN** 某条验收的 `contract` 既不命中目录条目，也不在白名单中
- **THEN** 注册表门控返回非零，并指名该验收 id 与不可解析的取值

#### Scenario: Draft contract satisfies the reference without upgrading

- **WHEN** 某条验收引用的是一个已登记但 `stage: draft` 的契约
- **THEN** 引用校验通过，但该验收的 `status` MUST NOT 因此改变，也 MUST NOT 被报告为已实现或已测试
