# spec-driven-delivery 变更（w82）

## MODIFIED Requirements

### Requirement: The ledger's field set is closed and fully classified

待裁定台账的**字段集** SHALL 在**两个层级上各自封闭**：**信封**（台账顶层）出现的每个键 MUST ∈
已声明集合 `TOP_LEVEL_FIELDS`，**条目**出现的每个键 MUST ∈ 已声明集合 `LEDGER_FIELDS`；
两个集合 MUST **不相交**——一个字段名只属于一个层级。每个条目 MUST 被分入**结构字段**∪
**引用字段**∪**id 清单字段**之一，三类 MUST 两两不相交且其并集恰等于 `LEDGER_FIELDS`；
信封的每个已声明字段 MUST 在台账上**必需**。只遍历一份清单的扫描，盲区恰是**清单的补集** ⇒
新增键 MUST 命中一条**闭世界**断言，且 MUST NOT 只在一个层级上成立。`question` 这类
**复述字段** MUST 被纳入引用扫描，MUST NOT 只做非空检查。

#### Scenario: An entry carries a field the checker does not know

- **WHEN** 某条目新增一个不在 `LEDGER_FIELDS` 里的字段（如 `rationale: "见 §99.9"`），
  **不论其值是否含 `§NNN`**
- **THEN** 校验 MUST 失败并指名该条目与那个字段名——一个不受检查的字段，与没有这条断言是同一件事

#### Scenario: A top-level key the checker does not know

- **WHEN** 台账顶层新增一个不在 `TOP_LEVEL_FIELDS` 里的键（如 `revision: "见 §99.9"`），
  **不论其值是否含 `§NNN`**
- **THEN** 校验 MUST 失败并指名那个键——条目的键集封闭 MUST NOT 被当作整个台账的键集封闭

#### Scenario: A declared envelope field carries no value

- **WHEN** `TOP_LEVEL_FIELDS` 声明了某字段，而台账上没有它
- **THEN** 校验 MUST 失败并指名该字段——声明它就要在数据里有它，否则它是一个没人读的幽灵字段

#### Scenario: A field name that claims both levels

- **WHEN** 某个字段名同时出现在 `TOP_LEVEL_FIELDS` 与 `LEDGER_FIELDS` 里
- **THEN** 校验 MUST 失败并指名该字段——一个名字只属于一个层级；否则它会同时在两个闭世界的缝里

#### Scenario: A declared field falls outside every check category

- **WHEN** 某字段进了 `LEDGER_FIELDS` 却没有被分到任何一类，或同时落入两类
- **THEN** 校验 MUST 失败并指名该字段——否则它在两个检查之间落空，且没有任何信号

#### Scenario: A restatement field is left unscanned

- **WHEN** `question` 这类复述基线内容的字段只做非空检查、不在引用扫描清单里
- **THEN** 它 MUST 被纳入引用扫描（`CITATION_FIELDS`），MUST NOT 只做非空检查——
  「复述处可以随便编」适用于**每一个**复述字段，遗漏一个即与无断言等价
