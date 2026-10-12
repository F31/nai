# versioned-contracts 变更（w75）

## MODIFIED Requirements

### Requirement: Every schema under schemas/ is enforced or named

`schemas/` 下的每个 `*.schema.json` SHALL 要么被**执行体消费**，要么出现在**具名的例外清单**里，
且每条例外 MUST 附非空理由。**消费关系 MUST 派生**（扫描源码：Ajv 校验器清单、Rust 的
`include_str!`），MUST NOT 写成一份会过期的清单；扫描 MUST 只认代码文件，MUST NOT 把数据文件或
文档里提到某个路径算作消费。提供消费证据的文件 MUST 是**仓库里已跟踪的文件**——判据的输入
MUST 等于它断言的对象。⇒「有一份 schema」MUST NOT 被当作「有一份契约」。

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
- **THEN** 它 MUST NOT 因此被判为已消费——判据 MUST 只在**仓库内容**上成立：已跟踪文件集 MUST
  从版本控制的**索引**派生（纯静态读取，不派生任何子进程），MUST NOT 写成一份靠人维护的清单。
  索引不可解析时校验 MUST **失败**并区分原因（不存在／格式不符／版本不支持／条目截断），
  MUST NOT 静默退化成「工作区即仓库」

#### Scenario: The scan set is structural and every exclusion pays for itself

- **WHEN** 判据决定跳过哪些目录，或把扫描到的路径拿去与索引比较
- **THEN** 跳过规则 MUST 是**结构性的**（跳过所有以 `.` 开头的目录）再叠加一份**具名排除**；
  每条形如「某目录名」的排除 MUST 被 `.gitignore` 忽略——**「它不是仓库的一部分」就是那条理由**。
  排除 MUST NOT 是一张靠人维护的目录名清单；扫描集里的路径 MUST 是 POSIX 相对路径（MUST NOT
  带 `./` 前缀），否则它与索引路径不可比

#### Scenario: One side of an extension pair is forgotten

- **WHEN** 判据按扩展名解析注释语法
- **THEN** 每个可扫描扩展名 MUST 在**语言表**里有条目，且可扫描扩展名 MUST 由该表**派生**——
  「新增扩展名」与「声明它的注释语言」MUST 是同一件事。`.yml` 与 `.yaml` 是同一种语言，
  MUST 一起被覆盖；只认其中一个 MUST 被当作缺陷，而不是「暂时没写」
