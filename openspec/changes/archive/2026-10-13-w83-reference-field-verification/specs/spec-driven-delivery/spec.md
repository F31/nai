# spec-driven-delivery 变更（w83）

## MODIFIED Requirements

### Requirement: Every field that can carry a section reference is checked

待裁定台账的引用完整性 SHALL 覆盖**每一个可能承载 `§NNN` 的字段**——`basis`、`introduced_in`、
`also_in`、`recommendation`、`closure`、`note`、`question`；扫描域 MUST 由一份**单一清单**派生，
MUST NOT 在循环里手写。触发条件 MUST 只由**字段是否存在**决定，MUST NOT 由**条目当前的处置**
决定——一个字段由谁解析它的引用，是**字段的属性**；按处置触发会让同一个字段换个处置就**没人管**。
校验器自身的常量表 MUST 互相一致：处置枚举与必填字段 MUST **一一对应**。

#### Scenario: A reference in a field outside the scanned set

- **WHEN** 任一承载 `§NNN` 的字段（含 `recommendation`／`closure`／`note`／`question`）引用一个
  当前基线里不存在的章节号
- **THEN** 校验 MUST 失败，并**指名该条目、该字段与那个章节号**——漏掉字段里的一条引用，
  与没有这条断言是同一件事

#### Scenario: A restatement field is left unscanned

- **WHEN** `question` 这类**复述字段**只做非空检查、不在引用扫描清单里
- **THEN** 它 MUST 被纳入引用扫描，MUST NOT 只做非空检查——「复述处可以随便编」适用于
  **每一个**复述字段，遗漏一个即与无断言等价

#### Scenario: A disposition without a required field

- **WHEN** 处置枚举里出现一个在必填字段表里没有对应项的处置
- **THEN** 校验 MUST 失败并指名该处置——否则它的条目会「没有必填字段」地通过，而校验对它沉默

#### Scenario: A required field that no check covers

- **WHEN** 某个处置的必填字段既不在引用扫描清单里、也不在 id 校验清单里，或一个字段同时出现在两处
- **THEN** 校验 MUST 失败并指名该字段——否则它在两个检查之间落空，且没有任何信号

## ADDED Requirements

### Requirement: Every register-id reference is resolved wherever its field appears

待裁定台账里**承载登记 id 引用**的字段（`precondition`），其每个元素 SHALL 被解析为**基线作业项**
或**注册表验收 id**。校验 MUST 按**字段是否存在**触发，MUST NOT 按**处置**触发——把
`precondition` 挂到非 `needs_precondition` 的条目上，此前**完全静默**。引用扫描域 MUST 是**单一
声明**（`REFERENCE_FIELDS`），且 MUST 与类别划分**双向相等**——声明了却无类别认领，其引用
**没人解析**；被类别认领却不在声明集里，扫描会**跳过它**。

#### Scenario: An id reference on an entry of another disposition

- **WHEN** 某条**非** `needs_precondition` 的条目的 `precondition` 写成基线里不存在的作业项编号，
  或注册表里没有的验收 id
- **THEN** 校验 MUST 失败并**指名该条目、该字段与那个 id**——静默接受一条「等不存在的东西」的
  前置，等于把这条登记挂在了空气上

#### Scenario: A legal reference on an entry of another disposition

- **WHEN** `precondition` 出现在**非**其专属处置的条目上，而它的每个 id 都能解析
- **THEN** 校验 MUST NOT 失败——本轮拒绝的是**不可解析**的引用，不是**放错位置**的引用；
  把「位置」也一并拒绝会把合法数据判红（假阳性方向）

#### Scenario: A reference field that no category owns

- **WHEN** 某字段进了引用扫描域却没有被任何类别认领，或被类别认领却不在扫描域里
- **THEN** 校验 MUST 失败并指名该字段——否则它在两个检查之间落空，且没有任何信号
