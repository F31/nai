# versioned-contracts 变更（w73）

## ADDED Requirements

### Requirement: Every schema under schemas/ is enforced or named

`schemas/` 下的每个 `*.schema.json` SHALL 要么被**执行体消费**，要么出现在**具名的例外清单**里，
且每条例外 MUST 附非空理由。**消费关系 MUST 派生**（扫描源码：Ajv 校验器清单、Rust 的
`include_str!`、脚本里的路径字面量），MUST NOT 写成一份会过期的清单；扫描 MUST 只认代码文件，
MUST NOT 把数据文件或文档里提到某个路径算作消费。⇒「有一份 schema」MUST NOT 被当作「有一份契约」。

#### Scenario: A schema nobody reads

- **WHEN** `schemas/` 下存在一份没有任何执行体引用的 `*.schema.json`，且它不在具名例外清单里
- **THEN** 校验失败并指名该 schema 文件，MUST NOT 因为「目录里有它」而视为已生效

#### Scenario: A schema quietly drops out of the validator list

- **WHEN** 某份本来被校验器清单引用的 schema 被从清单里删掉（文件仍在）
- **THEN** 它 MUST 立刻变成「无人消费」并被上面的判据抓住；MUST NOT 因为文件仍存在而通过

#### Scenario: An exception is declared without a reason

- **WHEN** 例外清单里有一条没有给出理由，或给出的理由为空
- **THEN** 校验失败——具名例外的全部价值就是那一条理由，缺了它例外就退化成豁免
