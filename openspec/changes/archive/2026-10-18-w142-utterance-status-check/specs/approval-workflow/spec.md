# approval-workflow Specification

## ADDED Requirements

### Requirement: The database refuses utterance statuses the code cannot classify

存储层 SHALL 用 schema 级约束把 `utterances.status` 限定在已声明的取值域内，
该域 MUST 是 `utterance_status::classify` 不肯返回 `Unassigned` 的那些取值。
域外整数（`0`、`7..=10`、`16..`、负数）MUST 在**写入时**被拒绝，
MUST NOT 先写进去、由读侧把它归成「未分类」。
拒绝 MUST 由该约束给出且可归因，MUST NOT 归因于别的约束（`NOT NULL`、外键、`STRICT` 类型）。

#### Scenario: An undeclared value cannot be written

- **WHEN** 任何 SQL 客户端（含绕开 `Store` 的直连写入）把域外整数写进这一列
- **THEN** 写入 MUST 失败，且错误 MUST 指明是取值域约束

#### Scenario: Every declared value still round-trips

- **WHEN** 写入已声明域内的取值
- **THEN** 写入 MUST 成功，且 schema 接受的集合 MUST 与声明的域**逐值**相同

#### Scenario: The stored constraint and the classifier agree

- **WHEN** 从存储读出 `utterances` 的建表语句，与 `classify` 派生的域比较
- **THEN** 两者 MUST 逐字一致；改了域而没有新迁移（或反之）MUST 失败

### Requirement: Rebuilding the utterance table preserves rows, evidence links and the foreign key

整表重建 MUST 逐字保留既有行，MUST NOT 丢失 `evidence_links` 的子行，
且重建后 `PRAGMA foreign_key_check` MUST 为空。该升级路径 MUST 在**有既有行且带子行**的
数据库上验证 —— 空表上的重建不构成证据。

#### Scenario: A populated database upgrades

- **WHEN** 一个已有 utterance 行与 evidence 链的旧版本数据库升级到新版本
- **THEN** 行逐字保留、子行仍在、外键完好，且新约束升级后才开始生效

#### Scenario: The hole existed before the upgrade

- **WHEN** 在升级**前**尝试写入域外整数
- **THEN** 它 MUST 成功 —— 这是「新约束真的是新增的闸门」的判别性前提
