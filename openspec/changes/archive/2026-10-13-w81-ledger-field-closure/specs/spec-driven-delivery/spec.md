# spec-driven-delivery 变更（w81）

## ADDED Requirements

### Requirement: The ledger's field set is closed and fully classified

待裁定台账的**字段集** SHALL **封闭且完备**：每个条目出现的每个键 MUST ∈ 已声明字段集 `LEDGER_FIELDS`；
该集合 MUST 恰等于**结构字段**（由格式或枚举专属判定）∪**引用字段**∪**id 清单字段**，三类 MUST
两两不相交。只遍历一份清单的扫描，其盲区恰是**清单的补集**——清单挡得住「清单上的字段被编造」，
挡不住**新增字段** ⇒ 新增字段 MUST 命中一条**闭世界**断言。`question` 这类**复述字段** MUST 被
纳入引用扫描，MUST NOT 只做非空检查。

#### Scenario: An entry carries a field the checker does not know

- **WHEN** 某条目新增一个不在 `LEDGER_FIELDS` 里的字段（如 `rationale: "见 §99.9"`），
  **不论其值是否含 `§NNN`**
- **THEN** 校验 MUST 失败并指名该条目与那个字段名——一个不受检查的字段，与没有这条断言是同一件事

#### Scenario: A declared field falls outside every check category

- **WHEN** 某字段进了 `LEDGER_FIELDS` 却没有被分到任何一类，或同时落入两类
- **THEN** 校验 MUST 失败并指名该字段——否则它在两个检查之间落空，且没有任何信号

#### Scenario: A restatement field is left unscanned

- **WHEN** `question` 这类复述基线内容的字段只做非空检查、不在引用扫描清单里
- **THEN** 它 MUST 被纳入引用扫描（`CITATION_FIELDS`），MUST NOT 只做非空检查——
  「复述处可以随便编」适用于**每一个**复述字段，遗漏一个即与无断言等价
