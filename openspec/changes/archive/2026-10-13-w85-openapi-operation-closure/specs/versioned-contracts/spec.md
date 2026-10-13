# versioned-contracts 变更（w85）

## ADDED Requirements

### Requirement: The API description's operation set is closed

OpenAPI 描述里**每个操作** SHALL 被逐项校验权限声明与实现状态，且判据 MUST 遍历**文档自身声明的操作键**，
MUST NOT 遍历一张手写的方法子集——清单的补集就是盲区（未列出的方法既不查权限、也不查状态）。
方法键集 MUST 封闭，且与 path-item 字段集 MUST 不相交；已实现操作的声明 MUST 逐条指回文档里
真实存在、状态相符的操作。

#### Scenario: An operation under a method nobody listed

- **WHEN** 文档新增一个方法键（如 `patch`），而它不在一张手写的方法清单里
- **THEN** 它 MUST 与别的方法一样被逐项校验权限声明与实现状态，MUST NOT 被静默跳过；
  缺 `x-required-permission` MUST 具名失败

#### Scenario: A key that is neither a method nor a field

- **WHEN** 某个 path item 出现一个既不是 HTTP 方法、也不是标准 path-item 字段、也不带 `x-`
  扩展前缀的键
- **THEN** 校验失败并指名该键与其所在路径——一个谁都认不出的键 MUST NOT 悄悄成为一个无人检查的操作

#### Scenario: A declared implementation with no operation

- **WHEN** 已实现操作清单里的某一项在文档里找不到对应操作，或该操作的状态与声明不符
- **THEN** 校验失败并指名该条；MUST NOT 因为「清单里有它」而视为已实现——
  一条只有声明、没有对象的契约 MUST NOT 通过

#### Scenario: The judgement sees nothing

- **WHEN** 文档不暴露任何操作
- **THEN** 校验失败——逐操作检查在空集上恒真，非空 MUST 被显式断言

#### Scenario: A method outside the old list, done correctly

- **WHEN** 一个落在任何手写清单之外的方法**带**权限声明，且其实现状态与是否在已实现清单中一致
- **THEN** 校验通过——判据 MUST NOT 因为方法「新」或「不常用」而拒绝它，只校验它是否被声明完整
