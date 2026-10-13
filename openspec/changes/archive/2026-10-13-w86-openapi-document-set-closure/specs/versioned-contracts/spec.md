# versioned-contracts 变更（w86）

## ADDED Requirements

### Requirement: Every OpenAPI description is covered by the operation-set closure

仓库里**每一份** OpenAPI 描述 SHALL 被操作集闭世界覆盖，且**文档集本身 MUST 是闭世界**：
`schemas/openapi/` 下**已跟踪**的描述 MUST 恰好等于逐份写死的声明集，MUST NOT 由「校验器当前
硬编码的那一份」决定覆盖范围。逐文档的已实现声明 MUST **显式注入**，缺失即失败，MUST NOT
静默退回某一份文档的默认值。权限规则的**定义只有一处**，生成器 MUST NOT 复述它。

#### Scenario: A second description nobody validates

- **WHEN** `schemas/openapi/` 下存在一份**已跟踪**的 OpenAPI 描述，而声明集里没有它
- **THEN** 校验失败并指名该文档；且它内部一个缺 `x-required-permission` 的操作
  MUST 与 `control.yaml` 的操作走**同一条**判据，MUST NOT 因为「没人列举它」而无人过问

#### Scenario: A declaration with no document

- **WHEN** 声明集里有一条指向仓库中不存在的描述（改名、删除、或从未落地）
- **THEN** 校验失败并指名该条——一条只有声明、没有对象的覆盖 MUST NOT 通过

#### Scenario: The closure is bound to one document by hand

- **WHEN** 声明集收缩回只剩一份描述，而目录下仍有别的已跟踪描述
- **THEN** 上一场景与「未声明」两条之一 MUST 开火——覆盖范围 MUST NOT 由手写清单决定

#### Scenario: The per-document declaration is missing

- **WHEN** 判据被调用时没有注入该文档的已实现声明
- **THEN** 具名失败，MUST NOT 静默使用另一份文档的声明（恒真的断言比没有断言更坏）

#### Scenario: A second description, declared and complete

- **WHEN** 一份新的描述被加进声明集，且其每个操作都带权限声明、状态与声明一致
- **THEN** 校验通过——本判据补的是「**覆盖**文档集」，MUST NOT 变成「**只允许**一份文档」
