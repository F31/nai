# spec-driven-delivery 变更（w80）

## ADDED Requirements

### Requirement: Every field that can carry a section reference is checked

待裁定台账的引用完整性 SHALL 覆盖**每一个可能承载 `§NNN` 的字段**——`basis`、`introduced_in`、
`also_in`、`recommendation`、`closure`、`note`；扫描域 MUST 由一份**单一清单**派生，MUST NOT 在
循环里手写。校验器自身的两张常量表 MUST 互相一致：处置枚举与必填字段 MUST **一一对应**。

#### Scenario: A reference in a field outside the scanned set

- **WHEN** 任一承载 `§NNN` 的字段（含 `recommendation`／`closure`／`note`）引用一个当前基线里
  不存在的章节号
- **THEN** 校验 MUST 失败，并**指名该条目、该字段与那个章节号**——漏掉字段里的一条引用，
  与没有这条断言是同一件事

#### Scenario: A disposition without a required field

- **WHEN** 处置枚举里出现一个在必填字段表里没有对应项的处置
- **THEN** 校验 MUST 失败并指名该处置——否则它的条目会「没有必填字段」地通过，而校验对它沉默

#### Scenario: A required field that no check covers

- **WHEN** 某个处置的必填字段既不在引用扫描清单里、也不在 id 校验清单里，或一个字段同时出现在两处
- **THEN** 校验 MUST 失败并指名该字段——否则它在两个检查之间落空，且没有任何信号
