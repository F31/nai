## ADDED Requirements

### Requirement: The memory read judgement is wired to the real store, and each of its three entry inputs is derived from a repository fact

组合根 SHALL 提供一处把 §42.3 的记忆读取判定接上**真实记忆存储**的入口：先取数，再**诚实派生**
每一条的 `revoked` / `permitted` / `approximate_of`，然后过判定。三个输入 MUST 各由一条**仓库事实**
派生（`revoked` 取自记忆存储的撤销列、`permitted` 取自该 actor 对该记录作用域的授权是否已被撤销、
`approximate_of` 恒为「无」），MUST NOT 写默认值。取数 MUST **只按工作区过滤**——按会话预筛会让
越作用域拒绝**失去对象**。判定 MUST NOT 在存储层重跑。

#### Scenario: An out-of-scope record is refused by name, not pre-filtered away

- **WHEN** 同一工作区的**另一个会话**存在记录，而按本会话读取
- **THEN** 该记录进 `refused` 且拒因为越作用域；取数阶段就把它筛掉的实现不满足本需求

#### Scenario: A revoked record is refused because the store says so

- **WHEN** 一条记录已在记忆存储中被标为已撤销
- **THEN** 该记录进 `refused` 且拒因为已撤销；撤销判定 MUST 读存储中的撤销列，
  MUST NOT 由调用方写死

#### Scenario: Permission denial is reachable, not a dead branch

- **WHEN** 该 actor 对该记录作用域的授权**已被撤销**
- **THEN** 该记录进 `refused` 且拒因为权限不足；本场景证明该拒因在本入口**可达**

#### Scenario: Budget trimming is visible

- **WHEN** 合格条目数超过本次预算
- **THEN** 落选的每一条 MUST 进 `refused` 且拒因为超配额；
  静默截断的实现不满足本需求

### Requirement: Forgetting blocks the read side before any statement about it is judged

组合根 SHALL 提供一处 §42.4 遗忘链的**阻断**落点：先把该记录在记忆存储中标为**已撤销**
（读取侧先失效），再判定该次遗忘的**陈述是否如实**。阻断 MUST **只置一次**——已撤销的行
MUST NOT 被覆盖，撤销时刻是一条事实。本需求**不**要求清除副本。

#### Scenario: Blocking precedes the statement

- **WHEN** 一次遗忘请求指明一条已落库的记录
- **THEN** 该记录先被标为已撤销，其后的陈述判定以「读取侧已阻断」为输入；
  顺序颠倒（先判陈述、后阻断）的实现不满足本需求

#### Scenario: An already-revoked row is not overwritten

- **WHEN** 对一条**已**被标为已撤销的记录再次请求阻断
- **THEN** 原有的撤销时刻 MUST **保持不变**

## MODIFIED Requirements

### Requirement: The memory store read-back is a per-column fetch, not the read judgement

存储层 SHALL 提供一处**只读**入口，把记忆存储按作用域读回，逐列还原一条记忆记录的九个字段；
读回 MUST NOT 追加、修改或删除任何行，MUST NOT 施加 §42.3 的读取判定（越作用域、过期、撤销、
权限不足、近似顶替与配额）。
记忆存储的撤销列是**存储态**而非契约字段 ⇒ 该入口 MAY 带出它，但 MUST NOT 据此施加判定；
该入口 MUST NOT 被读作「记忆读取面已接线」。

#### Scenario: Stored columns come back verbatim

- **WHEN** 一条记录已经落库，且按它自己的作用域读回
- **THEN** 读回入口返回该记录，九个字段**逐字相等**

#### Scenario: The read-back does not cross scopes

- **WHEN** 另一个会话存在记录，而按本会话读回
- **THEN** 返回结果 MUST NOT 含那一条记录

#### Scenario: The read-back judges nothing

- **WHEN** 某条已落库记录**已过期**
- **THEN** 读回入口**仍然**返回它——时效判定属 §42.3 的读取面，不由取数入口施加；
  返回结果里 MUST NOT 出现任何 §42.3 的条目级拒因
