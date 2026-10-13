# versioned-contracts 变更（w91）

## MODIFIED Requirements

### Requirement: Every required runtime-config key is read, pinned, or named

`schemas/config/` 下**每份 schema** 的每个**受判 `required` 键** SHALL 落在三类的**恰一个**里：
有**派生**出来的读取点、无读取点但值被 `const` 钉死、或无读取点也未被钉死而**逐键在册并附成因**。
三类 MUST 两两不相交、并集 MUST **恰等于**受判键集合；受判键集合 MUST 派生（`required` 键中减去
**有 `required` 子键的容器**，其消费由子键蕴含）。读取点 MUST 由**非测试、非生成、已跟踪**的源码
派生，MUST NOT 把数组字面量、注释、JSON 数据或生成物算作读取点；读取点的**形状**与其
**schema 归属** MUST 满足 `A config key's reader is attributed to the schema that owns it`。
⇒「配置里有这个键」MUST NOT 被当作「运行时看得见它的值」。

#### Scenario: A required key nobody reads and nobody named

- **WHEN** 任一份 `schemas/config/*` schema 新增一个 `required` 键，而仓库里没有任何读取点、
  它的值也不被 `const` 钉死，且它不在在册名单里
- **THEN** 校验失败并指名该 schema 与该键的**路径**；MUST NOT 因为「schema 接受了它」就视为已生效

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

## ADDED Requirements

### Requirement: A config key's reader is attributed to the schema that owns it

`schemas/config/` 下每份 schema 的受判键，其读取点 MUST 通过**归因文件集**绑定到该 schema：该集合 =
Rust `include_str!` 该 schema 的文件（**派生**）∪ **具名外部读者**（经 `Value` 传递、因而不
`include_str!` 的文件），且每条外部读者 MUST **自清理**。读取形状 MUST 覆盖三态：**索引** `EXPR["<key>"]`
（**只在归因集内计**；嵌套键锚到**父键**）、**结构体解码**（归因文件里被 `serde` 反序列化、且 **`struct`
定义在同一文件** 的结构体，字段名 ∩ 受判键名）、**读键辅助函数**（函数体含变量下标、或传递调用已判定者）。
读取点所属的 schema MUST 由归因决定，MUST NOT 由「文件里出现了同一个键名」决定
⇒ 跨 schema 的同名键 MUST NOT 互相充当读者。

#### Scenario: A key name shared by two schemas

- **WHEN** 两份 schema 有同一个键名（实测 `schema_version` 出现在 5 份 schema 的顶层、
  `mode` 出现 3 次、`currency` 2 次），而某文件里出现 `["<name>"]`
- **THEN** 它 MUST 只算作**其归因文件集包含该文件**的那一份 schema 的读取点；另一份 MUST NOT
  因此获得读者——「同名」MUST NOT 被当作「同一个键」

#### Scenario: A read site outside the attribution set

- **WHEN** 某文件里有某个键的索引读取，但该文件**不在**该键所属 schema 的归因文件集里
- **THEN** 它 MUST NOT 算作该 schema 的读取点——实测 `binding-plan#session_id` 的裸名在
  `crates/nai-control-api` 命中，那是**另一个对象**的同名键

#### Scenario: A struct decoded in the same file is not automatically this schema's reader

- **WHEN** 归因文件里有一个被 `serde` 反序列化的结构体，但它的字段名**与受判键名不相交**
  （实测 `plugins/avatar-2d/src/lib.rs` 里的 `Manifest` 与 `Config` **同在**该文件，
  Manifest 的 `output_width`/`kind`/`model_binding` 都不是 avatar-2d 的受判键）
- **THEN** 那些字段 MUST NOT 算作该 schema 的读取点——结构体解码 MUST 限定到
  **定义在同一文件** 且 **字段名与受判键名相交** 的结构体；MUST NOT 因为「同一文件里有解码」
  就认为该文件里的每个结构体都在读这份配置

#### Scenario: A struct decode is a reader of the schema it belongs to

- **WHEN** 归因文件里定义了结构体 `T` 并被 `serde` 反序列化，且 `T` 的字段名与受判键名相交
- **THEN** 那些键 MUST 算作**有读取点**（形状 `serde`）——实测 `plugins/avatar-2d/src/lib.rs` 的
  `Config` 覆盖 `avatar-2d` 的 7 个受判键，而 `EXPR["<key>"]` 一个都扫不到

#### Scenario: A key read through a helper function

- **WHEN** 归因文件里的键名是**字符串实参**（实测 `amount(&value, "install_bytes")`、`strings(&value, "hosts")`），
  全文没有 `["<key>"]`
- **THEN** 它 MUST 仍算作读取点（形状 `helper`）——判「读键辅助函数」只看**函数体含变量下标、或传递调用
  已判定者**（实测 `amount` → `text`）；MUST NOT 推广成「任何带字符串实参的调用」

#### Scenario: An external reader that stops reading

- **WHEN** 具名外部读者（如 `apps/engine/src/pipeline.rs` 经 `composition.config()` 读 runtime 的键）
  不再读该 schema 的任何受判键，或该文件未被版本控制跟踪
- **THEN** 校验失败并指名该配置项与文件——归因集不是豁免，它和别的台账一样 MUST **自清理**
