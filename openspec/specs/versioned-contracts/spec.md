# versioned-contracts Specification

## Purpose
为 nai 会话、能力和插件提供跨语言一致且可验证的版本化数据边界，以唯一 Schema 源生成消费者类型，并让后续内核、媒体、平台与管理实现能够识别无效输入和不兼容协议。

## Requirements

### Requirement: Single contract source

跨语言消息 SHALL 由 Protobuf 定义并生成 Rust/TS/C++；配置和清单 SHALL 由 JSON Schema 定义，HTTP SHALL
由 OpenAPI 定义。能力目录 SHALL 为**每一条**声明阶段 `stage`：`draft` 或 `frozen`。

**只有 `frozen` 条目**引用正式请求/响应消息并生成消费者类型（消息类型表、会话消息表与协商能力清单）；
目录 MUST NOT 存在无法解析的正式消息类型。**`draft` 条目 MUST NOT 声明** `request`、`response`、
`stream` 或 `session_scoped`——半声明的 draft 会让阶段字段失去意义，等于把「已冻结」与「未冻结」混为一谈。

阶段语义的判据与偏差 MUST 被记录：目录的「完整模型」（类别、必需性、首次出现的切片、缺失时的降级行为）
若尚未落地，MUST NOT 被表述为已完成。

#### Scenario: Capability references missing message

- **WHEN** 能力目录出现重复 ID、缺失类型，或 `frozen` 条目缺少或无正式请求/响应 Schema
- **THEN** 契约检查失败，不报告目录完整

#### Scenario: Draft entry claims consumer messages

- **WHEN** 一个 `stage: draft` 的条目声明了 `request`、`response`、`stream` 或 `session_scoped`
- **THEN** 目录校验失败，且该 id MUST NOT 出现在协商能力清单或消息类型表中

#### Scenario: Draft entry stays out of the consumer surface

- **WHEN** 目录中存在 `stage: draft` 的条目，且有切片验收引用该契约
- **THEN** 生成的消息类型表、会话消息表与协商能力清单中 MUST NOT 出现该 id；
  它 MUST 单独可见（可列出但不可协商），且该引用 MUST NOT 被当作「已冻结」或使任何验收升级为通过

### Requirement: Deterministic generation
生成工具 SHALL 锁定版本，从源重新生成全部产物；`schema --check` 必须在临时目录生成并拒绝新增、删除或内容漂移，不覆盖消费者文件。

#### Scenario: Generated code drift
- **WHEN** 已生成文件被修改、遗漏或增加陈旧产物
- **THEN** 检查返回非零并指明差异；正常重新生成可恢复一致性

### Requirement: Safe protocol representation
消息 SHALL 使用字符串 ID，TS 的 64 位整数保持字符串；信封必须包含协议、关联 ID、期限及载荷，会话操作必须包含 session/epoch；缺少必需字段、无期限、未知消息类型或不兼容主版本必须拒绝。

#### Scenario: Large epoch and future field
- **WHEN** 三种语言交换超出 JS 安全整数的 epoch，或带同主版本新增未知字段的消息
- **THEN** 已知字段无精度损失且可解码；未知字段不授予权限，未知枚举不当作 granted/pass

#### Scenario: Invalid control message
- **WHEN** 信封过大、主版本不兼容、无剩余期限或会话作用域字段缺失
- **THEN** 返回明确结构错误；编码成功不代表通过业务授权

### Requirement: Validated configuration and API description
配置/插件/模型清单 SHALL 拒绝未知字段、明文密钥位置、非法资源/期限及缺失必需组件；OpenAPI 必须描述鉴权、命令幂等/状态版本、会话 epoch 与实际 ACK，且保留端点尚未实现的状态。

#### Scenario: Invalid manifest or config
- **WHEN** 配置包含未声明字段、危险数值或凭据明文，模型包缺少图/I/O/许可/兼容矩阵
- **THEN** JSON Schema 校验失败，不报告插件/模型 Ready

### Requirement: Verified compatibility boundary
契约 suite SHALL 实际编译 Rust/TS，并在可用环境编译 C++ 生成代码，执行三语言互通、未知字段和协议范围测试；缺 C++ 环境必须失败或明确 Blocked，不能将生成成功视为编译或互通通过。

#### Scenario: Run contract suite
- **WHEN** 运行具备匹配工具/运行库的 contracts suite
- **THEN** 实际生成检查、配置/API 校验和跨语言数据交换通过后才报告成功

### Requirement: Registry contract references resolve

`tests/registry.yaml` 中每条验收的 `contract` 取值 SHALL 要么命中能力目录中的条目（**任意阶段**——
draft 也算已声明），要么命中**具名的非能力契约白名单**。白名单 MUST 保持极小，且每项 MUST 就地附上
「为什么它不是能力」的理由。`contract` 字段缺失 SHALL NOT 视为错误。门控 MUST 对不可解析的取值报错，
且 MUST NOT 因为「目录里暂时没有」而静默通过。

#### Scenario: Unresolvable contract reference

- **WHEN** 某条验收的 `contract` 既不命中目录条目，也不在白名单中
- **THEN** 注册表门控返回非零，并指名该验收 id 与不可解析的取值

#### Scenario: Draft contract satisfies the reference without upgrading

- **WHEN** 某条验收引用的是一个已登记但 `stage: draft` 的契约
- **THEN** 引用校验通过，但该验收的 `status` MUST NOT 因此改变，也 MUST NOT 被报告为已实现或已测试

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

### Requirement: The API description's operation set is closed

OpenAPI 描述里**每个操作** SHALL 被逐项校验权限声明与实现状态，且判据 MUST 遍历**文档自身声明的操作键**，
MUST NOT 遍历一张手写的方法子集——清单的补集就是盲区（未列出的方法既不查权限、也不查状态）。
方法键集 MUST 封闭，且与 path-item 字段集 MUST 不相交；已实现操作的声明 MUST 逐条指回文档里
真实存在、状态相符的操作。

#### Scenario: An operation under a method nobody listed

- **WHEN** 文档新增一个方法键（如 `patch`），而它不在一张手写的方法清单里
- **THEN** 它 MUST 与别的方法一样被逐项校验权限声明与实现状态，MUST NOT 被静默跳过；
  缺 `x-required-permission` MUST 具名失败

#### Scenario: A key that is neither a method nor a field

- **WHEN** 某个 path item 出现一个既不是 HTTP 方法、也不是标准 path-item 字段、也不带 `x-`
  扩展前缀的键
- **THEN** 校验失败并指名该键与其所在路径——一个谁都认不出的键 MUST NOT 悄悄成为一个无人检查的操作

#### Scenario: A declared implementation with no operation

- **WHEN** 已实现操作清单里的某一项在文档里找不到对应操作，或该操作的状态与声明不符
- **THEN** 校验失败并指名该条；MUST NOT 因为「清单里有它」而视为已实现——
  一条只有声明、没有对象的契约 MUST NOT 通过

#### Scenario: The judgement sees nothing

- **WHEN** 文档不暴露任何操作
- **THEN** 校验失败——逐操作检查在空集上恒真，非空 MUST 被显式断言

#### Scenario: A method outside the old list, done correctly

- **WHEN** 一个落在任何手写清单之外的方法**带**权限声明，且其实现状态与是否在已实现清单中一致
- **THEN** 校验通过——判据 MUST NOT 因为方法「新」或「不常用」而拒绝它，只校验它是否被声明完整
