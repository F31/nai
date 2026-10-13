# versioned-contracts 变更（w90）

## ADDED Requirements

### Requirement: Every required runtime-config key is read, pinned, or named

`schemas/config/runtime.schema.json` 的每个**受判 `required` 键** SHALL 落在三类的**恰一个**里：
有**派生**出来的读取点、无读取点但值被 `const` 钉死、或无读取点也未被钉死而**逐键在册并附成因**。
三类 MUST 两两不相交、并集 MUST **恰等于**受判键集合；受判键集合 MUST 派生（`required` 键中减去
**有 `required` 子键的容器**，其消费由子键蕴含）。读取点 MUST 由**非测试、非生成、已跟踪**的源码派生，
形状为 `EXPR["<key>"]`，MUST NOT 把数组字面量、注释、JSON 数据或生成物算作读取点。
⇒「配置里有这个键」MUST NOT 被当作「运行时看得见它的值」。

#### Scenario: A required key nobody reads and nobody named

- **WHEN** schema 新增一个 `required` 键，而仓库里没有任何读取点、它的值也不被 `const` 钉死，
  且它不在在册名单里
- **THEN** 校验失败并指名该键的**路径**；MUST NOT 因为「schema 接受了它」就视为已生效

#### Scenario: A registration that shadows a real reader

- **WHEN** 某个键在册状态是「无读取点」，而扫描在**非测试**源码里找到了它的读取点
- **THEN** 校验失败并指名该键与命中的文件——一条遮蔽真读者的在册项让判据**自我满足**

#### Scenario: A named reader that no longer reads

- **WHEN** 在册的 [A] 类键所写的**证据文件**里已经没有该键的读取形状（读取点被删或搬家）
- **THEN** 校验失败并指名该键与那条证据——具名的读者 MUST 可被复核

#### Scenario: A read site that nobody signed

- **WHEN** 扫描找到了某个键的读取点，而该键**不在** [A] 类在册名单里
- **THEN** 校验失败并指名该键与命中的文件——观测 MUST NOT 代替人签名

#### Scenario: A pinned key stops being pinned

- **WHEN** 某个无读取点的键的 schema 节点从 `const` 改成可变的类型（如 `boolean`）
- **THEN** 它 MUST 立刻掉出「被钉死」这一类并被指名——该分类 MUST 派生自 schema，MUST NOT 是手写清单

#### Scenario: A registered cause that is not a cause

- **WHEN** 在册项缺少成因、成因 id 不在成因表里，或成因表里有条目没有任何键引用
- **THEN** 校验失败并指名——在册不是豁免，它的全部价值就是那条可核实的成因

#### Scenario: A container's transitivity is assumed

- **WHEN** 一个 `required` 的 object 键**没有** `required` 子键
- **THEN** 它 MUST 自己受判（没有子键可以蕴含它的消费）——「容器」MUST 由结构判定，MUST NOT 由键名清单决定
