## ADDED Requirements

### Requirement: A judged memory write lands in the real memory store, and a refused one is named and lands nothing

组合根 SHALL 提供一处把 §42.2 的记忆写入判定接上**真实记忆存储**的入口：**先**跑判定，
**放行**才把该记录写进记忆存储；**拒绝**时 MUST **具名拒绝**且 MUST **一列不落库**。
判定 MUST NOT 被短路，MUST NOT 在存储层重跑——一条门槛 MUST 只答一次（存储层只做存储层的事：
校验作用域标识、查权限、写列、把主键冲突具名报出）。落库的每一列 MUST 逐字取自判定**审过**的输入：
runtime MUST NOT 由正文重算任何一列、MUST NOT 补默认值、MUST NOT 落判定未审过或存储中无对应列的字段。
写入 MUST 复用组合根既有的单 owner 存储路径，MUST NOT 另开第二条写入路径。

#### Scenario: An admitted write lands exactly one row with the judged columns

- **WHEN** 一次记忆候选在来源、作用域、来源标识、时间与「叙述还是权威事实」的标注上都过门槛
- **THEN** 该记录落进记忆存储**恰一行**，且每一列与判定输入**逐字相等**

#### Scenario: A refused write is named and leaves the store empty

- **WHEN** 某候选的来源是不可信来源（工具返回、外部智能体返回、评论原文或模型草稿）
- **THEN** 入口**具名拒绝**，且记忆存储**未新增**任何行；先落库再判定的实现不满足本需求

#### Scenario: The capability switch is answered once, on the write face too

- **WHEN** 观众长期记忆未启用，而候选是一条观众记录
- **THEN** 写入被具名拒绝且零落库；只判读取面、写入面照旧放行的实现不满足本需求

### Requirement: The memory store read-back is a per-column fetch, not the read judgement

存储层 SHALL 提供一处**只读**入口，把记忆存储按作用域读回，逐列还原一条记忆记录的九个字段；
读回 MUST NOT 追加、修改或删除任何行，MUST NOT 施加 §42.3 的读取判定（越作用域、过期、撤销、
权限不足、近似顶替与配额）。
**撤销状态在记忆存储中没有任何承载列** ⇒ 依赖它的判定 MUST NOT 在缺少该事实时被假装执行；
该入口 MUST NOT 被读作「记忆读取面已接线」。

#### Scenario: Stored columns come back verbatim

- **WHEN** 一条记录已经落库，且按它自己的作用域读回
- **THEN** 读回入口返回该记录，九个字段**逐字相等**

#### Scenario: The read-back does not cross scopes

- **WHEN** 另一个会话存在记录，而按本会话读回
- **THEN** 返回结果 MUST NOT 含那一条记录

#### Scenario: The read-back judges nothing

- **WHEN** 某条已落库记录**已过期**
- **THEN** 读回入口**仍然**返回它——时效判定属 §42.3 的读取面，不由取数入口施加；
  返回结果里 MUST NOT 出现任何 §42.3 的条目级拒因
