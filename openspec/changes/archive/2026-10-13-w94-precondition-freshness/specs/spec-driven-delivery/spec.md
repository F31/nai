## ADDED Requirements

### Requirement: Every precondition is still unmet

`needs_precondition` 断言的是「这条现在还不能裁定，因为它在等前置」。判据 MUST 断言
`precondition` 的每个元素**仍未达成**；已达成的前置 MUST 被**具名拒绝**。判定 MUST 由**注入的
事实**算出，MUST NOT 由注入侧直接给答案：注入验收 id → `status` 的表与工作项 → 关联验收的表，
「达成了没有」由判据自己按取值域判定（工作项没有状态列，其达成状态由基线**明写**的关联验收派生，
措辞 MUST 只说到派生那一层）。`status` 取值域 MUST 被 `MET_STATUSES` 与 `UNMET_STATUSES`
**恰好划分**，运行时遇域外取值 MUST 具名失败；工作项缺关联验收、或关联验收不在表内 ⇒ 同样
MUST 具名失败，MUST NOT 静默跳过。触发条件 MUST 只由**字段是否存在**决定，MUST NOT 由处置决定。

#### Scenario: A precondition that is already met is reported

- **WHEN** 某条 `precondition` 指着一个 `status` 已属 `MET_STATUSES` 的验收项，
  或指着一个其关联验收**全部**已达成的工作项
- **THEN** 校验 MUST 失败并指名该条目、该字段与那个 id——「还在等」若只看 id 是否解析得开，
  它就是一条被判据放过的空声明：等一个已达成的前置，与等一个真正悬空的前置会得到同一个答案

#### Scenario: An undecidable precondition does not pass silently

- **WHEN** 某工作项在基线里没有关联验收 id，或其关联验收不在注入的 `status` 表里，
  或取到的 `status` 落在声明的取值域之外
- **THEN** 校验 MUST 失败并指名该条目与那个 id——「没看到」与「出错了」在类型上分开，
  MUST NOT 被当成「仍未达成」而放行

#### Scenario: A status domain that no longer covers reality

- **WHEN** `MET_STATUSES` 与 `UNMET_STATUSES` 之并不再恰等于声明的 `status` 取值域（新增了
  一个取值却忘了归类），或两者相交
- **THEN** 校验 MUST 失败并指名该取值——否则判据会**静默地不认识**那个新取值，而没有任何信号

#### Scenario: A met precondition on an entry of another disposition

- **WHEN** `precondition` 出现在**非**其专属处置的条目上，而它指着一个已达成的前置
- **THEN** 校验 MUST 失败——触发条件是字段是否存在，与该条目当前归哪一类无关

## MODIFIED Requirements

### Requirement: Every register-id reference is resolved wherever its field appears

待裁定台账里**承载登记 id 引用**的字段（`precondition`），其每个元素 SHALL 被解析为**基线作业项**
或**注册表验收 id**。校验 MUST 按**字段是否存在**触发，MUST NOT 按**处置**触发——把
`precondition` 挂到非 `needs_precondition` 的条目上，此前**完全静默**。引用扫描域 MUST 是**单一
声明**（`REFERENCE_FIELDS`），且 MUST 与类别划分**双向相等**——声明了却无类别认领，其引用
**没人解析**；被类别认领却不在声明集里，扫描会**跳过它**。「解析得开」只答「这个 id 存在」，
**不**答「它现在还悬空着」——后者是**另一条**断言，两条 MUST NOT 互相冒充。

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
