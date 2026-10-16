## ADDED Requirements

### Requirement: The decision payload is bounded and the refusal is named
决策条目载荷 SHALL 受一个声明上限约束；超出该上限的载荷 MUST 以 `resource_exhausted` 具名拒绝，MUST NOT 被静默截断或写入。

#### Scenario: A payload beyond the declared bound is refused by name
- **WHEN** 提交一条序列化后超过声明上限的决策载荷
- **THEN** 写入被拒绝并返回 `resource_exhausted`
- **WHEN** 提交一条明显小于该上限的载荷
- **THEN** 写入被接受

### Requirement: A snapshot carrying a mutable journal is rejected
恢复快照 SHALL 拒绝任何携带可变 journal 副档（`-wal` / `-shm` / `-journal`）的快照，使其无法夹带批准数据库哈希之外的字节。

#### Scenario: A sidecar is refused and the clean snapshot still restores
- **WHEN** 在快照目录注入任一 journal 副档后执行恢复
- **THEN** 恢复以 `conflict` 拒绝
- **WHEN** 该副档被移除
- **THEN** 同一快照必须能够恢复
