## ADDED Requirements

### Requirement: A memory row carries when its payload was purged

记忆存储 SHALL 为每条记录承载 `purged_at_ms`（可空）：未清除为 `NULL`，清除后为生效时刻。
清除 SHALL 把 `content` 置空并**保留该行**（tombstone），且 SHALL **只置一次、不覆盖**——
重复清除返回**同一个**时刻。已撤销的记录 SHALL 仍可被清除，MUST NOT 因已阻断而拒绝。

#### Scenario: Purging is idempotent in its recorded instant

- **WHEN** 同一条记录被清除两次
- **THEN** 两次返回**同一个** `purged_at_ms`，MUST NOT 覆盖

#### Scenario: The row survives as a tombstone

- **WHEN** 一条记录被清除
- **THEN** 该行仍在库里而 `content` 为空，`purged_at_ms` 非空

#### Scenario: A blocked record can still be purged

- **WHEN** 一条**已撤销**（读取侧已阻断）的记录被清除
- **THEN** 清除成功，MUST NOT 因已阻断而拒绝

### Requirement: Upgrading to the purge column keeps rows and pins one version

迁移 SHALL 为既有库加列且**不丢行**；`db_schema_version` SHALL 随之升版，
其值 MUST 在迁移清单、备份 schema 契约、Rust 契约样本与 Node 备份样本中**一致**。

#### Scenario: An existing database keeps its rows

- **WHEN** 从上一版本的库升级到含 `purged_at_ms` 的版本
- **THEN** 既有记忆行**逐行保留**，新列为 `NULL`