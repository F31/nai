## MODIFIED Requirements

### Requirement: Harness observations are decidable before they reach the log

harness 的五类观测（装配／意图／记忆／工具／缓存）SHALL 各有一组固定必需记录项，取自 §47.1 且不增不减。
一条记录进入只追加的哈希链决策日志之前 MUST 先被判定：必需项齐备、值内不含明文凭据（INV-11）、
观众原文与昵称已按 §5.8 保留期脱敏。任一项不满足 MUST **具名拒绝**并给出可落日志的已脱敏载荷，
MUST NOT 原样透传，也 MUST NOT 凭字段名猜测敏感性。五类 MUST 恰为五类，缺任一类 MUST NOT 被读作齐全。
判定的是「能否进日志与以何形态进」；真正把可落日志的记录追加进哈希链由 `runtime-composition` 的「A judged-loggable harness observation reaches the hash chain」需求承担，本需求 MUST NOT 被读作「已落盘」。

#### Scenario: A record carries a plaintext credential

- **WHEN** 某条观测记录的**值**内出现凭据字面量（如 `sk-…`）
- **THEN** 该记录被具名拒绝且不可落日志，MUST NOT 以「字段名不含 key」为由放行

#### Scenario: Only a field name looks sensitive

- **WHEN** 某字段名含 `key` 但其值不含任何凭据字面量
- **THEN** 该记录 MUST 被放行；按字段名黑名单拒绝的实现不满足本需求

#### Scenario: A required field is missing

- **WHEN** 某一层记录缺少 §47.1 为该层规定的必需项之一
- **THEN** 该记录被具名拒绝并指名缺失项，MUST NOT 以空值补齐

#### Scenario: Viewer text outlives its retention

- **WHEN** 观众原文或昵称未脱敏，或其保留期已过仍在记录中
- **THEN** 分别具名拒绝，MUST NOT 让「说不清」被读成「已脱敏」
