# versioned-contracts 变更（w74）

## MODIFIED Requirements

### Requirement: Every schema under schemas/ is enforced or named

`schemas/` 下的每个 `*.schema.json` SHALL 要么被**执行体消费**，要么出现在**具名的例外清单**里，
且每条例外 MUST 附非空理由。**消费关系 MUST 派生**（扫描源码：Ajv 校验器清单、Rust 的
`include_str!`），MUST NOT 写成一份会过期的清单；扫描 MUST 只认代码文件，MUST NOT 把数据文件或
文档里提到某个路径算作消费。判据 MUST 只依赖**仓库内容**。⇒「有一份 schema」MUST NOT 被当作
「有一份契约」。

#### Scenario: A schema nobody reads

- **WHEN** `schemas/` 下存在一份没有任何执行体引用的 `*.schema.json`，且它不在具名例外清单里
- **THEN** 校验失败并指名该 schema 文件，MUST NOT 因为「目录里有它」而视为已生效

#### Scenario: A schema quietly drops out of the validator list

- **WHEN** 某份本来被校验器清单引用的 schema 被从清单里删掉（文件仍在）
- **THEN** 它 MUST 立刻变成「无人消费」并被上面的判据抓住；MUST NOT 因为文件仍存在而通过

#### Scenario: An exception is declared without a reason

- **WHEN** 例外清单里有一条没有给出理由，或给出的理由为空
- **THEN** 校验失败——具名例外的全部价值就是那一条理由，缺了它例外就退化成豁免

#### Scenario: An untracked file does not become a consumer

- **WHEN** 一个**未被版本控制**的文件出现在工作区，内容里带有某 schema 的路径，而那份 schema
  没有任何真实的执行体在读它
- **THEN** 它 MUST NOT 因此被判为已消费——判据 MUST 只在**仓库内容**上成立。本机宿主禁止派生
  子进程（无法取「仓库文件集」），所以这条靠**结构性收敛**成立：扫描集 MUST 由结构规则与具名
  排除定义，MUST NOT 是「工作区里所有匹配扩展名的文件」

#### Scenario: The scan set is structural and every exclusion pays for itself

- **WHEN** 判据决定跳过哪些目录
- **THEN** 跳过规则 MUST 是**结构性的**（跳过所有以 `.` 开头的目录）再叠加一份**具名排除**；
  每条形如「某目录名」的排除 MUST 被 `.gitignore` 忽略——**「它不是仓库的一部分」就是那条理由**。
  排除 MUST NOT 是一张靠人维护的目录名清单

#### Scenario: One side of an extension pair is forgotten

- **WHEN** 判据按扩展名解析注释语法
- **THEN** 每个可扫描扩展名 MUST 在**语言表**里有条目，且可扫描扩展名 MUST 由该表**派生**——
  「新增扩展名」与「声明它的注释语言」MUST 是同一件事。`.yml` 与 `.yaml` 是同一种语言，
  MUST 一起被覆盖；只认其中一个 MUST 被当作缺陷，而不是「暂时没写」

## ADDED Requirements

### Requirement: Each derivation source pays for itself

每条**派生来源** SHALL 找到至少一份**只有它能找到**的 schema。零独有覆盖的来源 MUST 被移除，
或与别的来源合并——它不提供判据，只提供「看起来更严格」的外观。

#### Scenario: A source whose every hit is covered elsewhere

- **WHEN** 某条来源找到的每一份 schema 也都被别的来源找到
- **THEN** 判据 MUST 失败并指名该来源：删掉它而没有任何断言变红，说明它不是守卫，是装饰

#### Scenario: Every remaining source has hits of its own

- **WHEN** 判据运转
- **THEN** **两侧独有集合都 MUST 非空**，且各自的独有成员 MUST 具名可考：
  Ajv 清单 MUST 独有找到 `schemas/tests/registry.schema.json` 与
  `schemas/platform-matrix/platforms.schema.json`；Rust `include_str!` MUST 独有找到
  `schemas/config/upgrade-package.schema.json`

### Requirement: The generator and the validator read one list of schemas

生成契约产物的执行体与校验契约的执行体 SHALL 读**同一份** schema 清单。生成面 MUST 是校验面的
子集，生成物的名字 MUST 互异，生成目录的内容 MUST 与清单**双向相等**。

#### Scenario: A schema is generated but never validated

- **WHEN** 某份 schema 出现在生成面的清单里，却不在校验面的清单里
- **THEN** 判据失败并指名该 schema——生成了一份没人校验的契约，正是「有一份 schema 就是有一份
  契约」的另一种说法

#### Scenario: Two schemas would write the same generated file

- **WHEN** 生成面上两条 schema 的 basename 相同（生成物按 basename 落盘）
- **THEN** 判据失败：那会让先写的那份被后写的覆盖，而「生成成功」看起来一切正常

#### Scenario: The generated directory and the list disagree

- **WHEN** 生成目录里存在清单外的产物，或清单里的某条在生成目录里没有产物
- **THEN** 判据失败并指名多出/缺失的那一项——生成器**实际做了什么**才是事实，
  一份自我声明的数组不是
