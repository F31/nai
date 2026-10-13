# admission-registry Specification

## Purpose
建立唯一的机器可读验收注册表，作为**当前基线**发布线（Slice-0/R1/R2/R3/实验/能力包）的单一事实源；自建 D01–D14 到新验收编号的映射与回填规则，落实 ADR 0015 对基线文本内部矛盾的勘误裁决，保证后续任务卡、认证与报告使用同一套编号且旧编号不再新增。

## Requirements

### Requirement: Single machine-readable admission registry

仓库 SHALL 维护 `tests/registry.yaml` 作为唯一验收来源，列出全部验收项并带 `release_line`、所属契约（如适用）、自动化等级、状态与证据引用；`release_line` MUST 使用独立字段区分验收编号与发布线名（slice_0、r1、r2、r3、experimental、**capability_pack**）。schema 校验 SHALL 拒绝重复编号、未知 release_line/status、悬空映射与新增的旧 T/E/U 编号。**该 schema 校验 MUST 真的被执行**——注册表 MUST 与 `schemas/tests/registry.schema.json` 逐字段校对，MUST NOT 出现「注册表写得进去、schema 认不出来」的状态；未知顶层键与未知条目字段一律拒绝。

#### Scenario: Duplicate or stale identifier
- **WHEN** 注册表中出现重复验收 ID 或新增旧 T/E/U 编号
- **THEN** 注册表校验失败，不进入后续切片任务

#### Scenario: Missing release line
- **WHEN** 某验收项缺少合法 release_line 或线号与 V3.11 §16 裁决不符
- **THEN** 校验失败并给出可读原因

#### Scenario: The registry is written but never checked against its schema

- **WHEN** `tests/registry.yaml` 出现一个 schema 未声明的顶层键或条目字段（W72 新增 `baseline_deviations`
  时即为此形状），而没有任何执行体把它交给该 schema
- **THEN** 该状态 MUST 被视为不满足本条：形状契约 MUST 在工程门控内被执行，
  MUST NOT 以「schema 文件存在」当作「schema 生效」；修复时 MUST NOT 只改 schema 的说明文字

### Requirement: Honest backfill mapping with traceable evidence

注册表 SHALL 提供 D01–D14 到新编号的映射段，且每个映射目标的状态 MUST 是 implemented/tested/blocked/not_applicable 之一；`tested` 仅可用于已有归档实测证据的项目，且证据 MUST 指向真实报告与测试计数。未完成或外部阻塞的目标 MUST 保留为 implemented/blocked，不得因映射自动升级为 tested。

#### Scenario: Backfill without real certification
- **WHEN** 某 D 任务仅有可移植逻辑检查、没有真实设备/SDK/模型认证
- **THEN** 对应验收项标 implemented 并注明缺口，不标 tested

#### Scenario: External dependency missing
- **WHEN** 目标能力缺合法模型、官方 SDK 或账户凭据
- **THEN** 对应验收项标 blocked 并附原因，演练不替代真实集成

### Requirement: Errata-adjudicated registry consistency

注册表 SHALL 落实 ADR 0015 勘误清单：R1 退出条件以 §16 为准；R04/R05/R08–R10 属 R1；值守租约失效后 600ms 内撤销输出授权；决策日志缓冲满时丢弃遥测段但关键提交先持久化；X01 门槛为 M0b 前写定。验收与任务卡不一致时 MUST 以注册表为准。

#### Scenario: Conflicting task-card milestone
- **WHEN** 任务卡声明的验收线号与注册表不一致
- **THEN** 以注册表为准并记录勘误引用，不创建两套编号

### Requirement: Registry covers every baseline acceptance family

注册表 SHALL 覆盖基线（`§16` 与 `§39.5`、`§48.5`、`§51.3` 与 `§57.3`；`§` 引用按**当前基线**解析——
版本号只在 `docs/design.md` 与最新基线 ADR 处声明）定义的**全部**验收家族——`S`/`R`/`A`/`B`/`O`/`P`/`M`/**`H`**/`X`。
基线把某编号体系段列为验收时，其条目 MUST 全部登记在 `tests/registry.yaml`；
**「尚未开始」MUST 以 `pending` 表达，MUST NOT 以之为由省略条目**——在册与测得是两个不同的陈述。
校验 SHALL 拒绝不属这些家族的编号，
并 SHALL 断言每个「基线已定义完整编号段」的家族**逐号在册**；
该短语的取值 MUST 从 §48.5 家族索引**逐行读出**，MUST NOT 取自写死的清单——
§48.5 声明几个连续段，就覆盖几个（当前 9 个）。
家族**在册**断言与发布线**分线**断言是两个不同的检查，MUST NOT 互相替代：前者证「条目没被省略」，
后者证「归属没被错线」。

#### Scenario: A baseline family is entirely absent

- **WHEN** 基线把某家族定义为验收编号体系的一部分（如开放平台与技能市场的 M01–M18，见 §37.5/§38.10；
  或智能体 Harness 与开放互操作的 H01–H18，见 §48.5），而 `tests/registry.yaml` 未登记其中任何条目
- **THEN** 注册表视为不完整，`cargo xtask check --registry` MUST 失败并指出缺失的编号

#### Scenario: A gap inside a registered family

- **WHEN** 某家族已在册但**缺号**（例如 18 条中少了 `H05`），或出现超出该家族定义范围的编号（例如 `H19`）
- **THEN** 校验失败并**指名**缺失或多出的编号，MUST NOT 因为「家族非空」而通过
- **AND** 断言 MUST 按家族**分别**计数与比号，MUST NOT 用「合计至少 36 条」这类**可互相掩盖**的口径；
  且 MUST NOT 是空洞的——MUST 以「人为删掉某个在册编号」的注册表验证它确实失败

#### Scenario: Not-yet-started is registered, not omitted

- **WHEN** 某验收家族尚无可交付实现（其对应工作项未动工），但该家族**在**当前发布线的退出条件内
- **THEN** 其条目 MUST 以 `pending` 登记在册，MUST NOT 省略条目，MUST NOT 把状态写成 `implemented`/`blocked`

#### Scenario: An identifier outside every family is rejected

- **WHEN** 注册表中出现不属于这些家族的编号（如 `B01a`）
- **THEN** 校验失败，不进入后续切片任务

#### Scenario: The judgement source is narrowed to the families named today

- **WHEN** 某实现把「基线已定义完整编号段」的家族理解成一份**写死的清单**（例如只覆盖 M 与 H），
  而 §48.5 的家族索引声明了更多连续编号段
- **THEN** 该实现 MUST 被视为不满足本条：判据来源 MUST 是 §48.5 的正文，MUST NOT 是清单里的示例。
  括号里的旧文「（当前为 M01–M18 与 H01–H18）」是**示例**而非枚举，此点已由 W72 裁定

### Requirement: Release lines are declared, and the capability pack is never a first-release gate

发布线 SHALL 为 `slice_0`/`r1`/`r2`/`r3`/`experimental`/**`capability_pack`** 六条（V3.11 §51.1）。
`capability_pack` 的条目集合 MUST **恰为** **M01–M18 与 H05–H14、H16**（共 **29** 条，V3.11 §51.3、§57.3）。
首版门槛（`slice_0`/`r1`/`r2`）与 `capability_pack` MUST **互不相交**——即 **INV-33**：
默认组合不依赖能力包，首版门槛内的每一条验收 MUST 在**未安装任何能力包**时可判定。
集合缺一条、混入归属外的编号，或任一能力包 id 落在首版门槛线上，校验 MUST 失败并**同时给出 id 与当前所在线**。

#### Scenario: A capability pack item is promoted back to the first-release gate

- **WHEN** 某能力包条目（例如 `M01`）被改回首版门槛线（`r1`/`r2`/`slice_0`）
- **THEN** 校验失败，错误 MUST 同时指名该 id 与它现在所在的线（形如 `M01=r1`），并引用 **INV-33**

#### Scenario: An identifier outside the pack set is parked on the capability pack line

- **WHEN** 一个不属于 M01–M18 与 H05–H14、H16 的编号（例如 `H17`）被标为 `capability_pack`
- **THEN** 校验失败并指名该编号，MUST NOT 因为「集合条数看起来够」而通过

#### Scenario: The capability pack line is emptied

- **WHEN** 能力包线的条目被清空（或整批改写为其他线），集合不再完备
- **THEN** 校验失败并指名缺失的编号（形如 `M01`），MUST NOT 因为「能力包线可以为空」而通过

#### Scenario: The new line is not registered in the schema

- **WHEN** `capability_pack` 出现在 `tests/registry.yaml`，而 schema 的 `release_lines` 或
  `acceptance.release_line.enum` 未把它列为合法取值
- **THEN** schema 校验失败，MUST NOT 出现「注册表写得进去、schema 认不出来」的状态

### Requirement: The frozen baseline and the registry reconcile, modulo named deviations

`tests/registry.yaml` SHALL 与**冻结基线**逐 id 对账：§48.5 的家族索引与 §57.3 的发布线正本
（后者含表格与计数行两处写法，二者 MUST 互相自洽）。在册 MUST 等于基线声明，
**除** `baseline_deviations` 中逐条具名登记的偏差——每条 MUST 给出 id、登记轮次与非空理由，
且 MUST 对应一处**真实**差异；基线收录后未清空 MUST 按**陈旧条目**失败。
**任一条目的 `since_baseline` MUST 等于当前基线版本**——跨版只有两种处置：收录，或显式结转
（改 `since_baseline` + 记 `carried_at` + 非空理由）。
基线文档 SHALL 取 `docs/` 中版本号最大者，且 MUST 与 `openspec/config.yaml` 声明的一致。
对账 MUST NOT 是空洞的：章节号被改、该节缺失、计数行与表格不符、缺版本入参，均 MUST 分别具名失败。

#### Scenario: The baseline lags behind the registry

- **WHEN** 注册表在 `r1` 下登记了 `B07`（或把它移到了 `r2`），而冻结基线 §57.3 的 R1 行仍不含它
- **THEN** 对账失败并**同时给出该 id 与两侧的线**（在册所在线 / 基线声明所在线），
  MUST NOT 只报「条数不符」——条数不符有一百种原因，id 才是可执行的线索

#### Scenario: A deviation is registered but never collected

- **WHEN** 某偏差已被下一轮升基线收录（基线声明与在册重新一致），而 `baseline_deviations` 仍保留该条目
- **THEN** 对账失败并**指名该条目**、要求清空——否则台账只增不减，最终无人再读

#### Scenario: A deviation entry does not match any real difference

- **WHEN** `baseline_deviations` 登记了 `S02`，而实际差异是 `B07`
- **THEN** 两条都 MUST 失败：真实差异未被解释，且该条目**张冠李戴**，MUST NOT 因为「台账非空」而通过

#### Scenario: The section the gate reads is renamed or renumbered

- **WHEN** §48.5 或 §57.3 的**章节号**被改动，或该节被删除（改标题措辞**不**属此列）
- **THEN** 对账 MUST 报出「该节找不到」这一**解析失败**，与「设计漂移」分开具名，
  MUST NOT 因为解析结果为空集而**通过**

#### Scenario: The baseline pointer is stale

- **WHEN** `docs/` 中版本号最大的基线文档与 `openspec/config.yaml` 声明的不是同一份
- **THEN** 对账失败并**同时给出两个文件名**，MUST NOT 静默地对着其中一份跑

#### Scenario: A registered deviation survives a baseline upgrade

- **WHEN** 台账里有一条 `since_baseline` 为上一版的条目，而当前基线已经换版（该偏差既未收录、
  也未被显式结转）
- **THEN** 对账失败并**同时给出该条目的 id 与两侧版本**，MUST NOT 因为「条目仍然相称」而通过；
  正确处置只有两种——收录（删条目），或结转（改 `since_baseline` + 记 `carried_at` + 非空理由）

#### Scenario: The reconciliation is asked to decide without knowing the version

- **WHEN** 调用对账时没有给出当前基线版本（或被给出一个非法版本串）
- **THEN** 对账 MUST 具名报出「无法判定台账是否跨版」，MUST NOT **静默跳过**该判据——
  一条会因为入参缺失而恒真的断言，比没有断言更坏

### Requirement: The registry's shape layer and semantic layer are defined once

注册表的**形状** SHALL 由 `schemas/tests/registry.schema.json` 定义，**语义**（家族完整性、能力包集合、
发布线归属、跨文件契约绑定）由 `scripts/contracts/*.mjs` 定义，两者 MUST 同时生效。
它们在**三处**重叠——id 家族 pattern、发布线枚举、状态枚举——每处 MUST 逐字一致；
不一致时校验 MUST 失败并**指名是哪一处**（哪一边、哪个常量）。重叠是允许的，漂移不是。

#### Scenario: The identifier family pattern drifts between the two layers

- **WHEN** 脚本的 id 正则与 schema 的 `acceptance.id.pattern`（以及映射段 targets 的 pattern）不再相同
  （例如脚本里家族白名单少了一个字母）
- **THEN** 校验失败并指明是 **id 家族 pattern** 这一处漂移，MUST NOT 因为「两边都能跑」而通过

#### Scenario: The release-line or status enumeration drifts between the two layers

- **WHEN** 发布线枚举（脚本的 `RELEASE_LINES` 与 schema 的 `release_lines` 键集合 / `release_line.enum`）
  或状态枚举（脚本的状态集合与 schema 的 `status.enum`）有一边被改动
- **THEN** 校验失败并指出是**发布线枚举**还是**状态枚举**漂移，MUST NOT 静默取其中一边为准

### Requirement: The capability-pack split is a partition stated by §51.3

§51.3 的「能力包清单」SHALL 被**解析**并与在册对账，MUST NOT 只被引用。表的行数、**逐行标签与编号**、
以及每行「依据」非空，MUST 逐项判定。该表 SHALL 是一**划分**：任一编号 MUST NOT 同时属于两个能力包。
表后「保留在 R1 的基础纪律」列出的编号 MUST 非空且与能力包并集**不相交**。
`§51.3 的并集`、registry 的 `capability_pack` 行、门控常量，三者 MUST 一致。

#### Scenario: A pack row silently shrinks

- **WHEN** §51.3 的某一行把编号改小（如 `M01–M12` 改成 `M01–M02`），而行的数量不变
- **THEN** 对账 MUST 失败并**同时给出该行的标签与两侧的编号集合**，MUST NOT 因为「行数没变」而通过

#### Scenario: An identifier belongs to two packs

- **WHEN** 某个编号同时出现在 §51.3 的两行里（并集因此看起来仍然完整）
- **THEN** 对账 MUST 失败并**同时给出该编号与它所在的两个包**——打包粒度是一个划分，不是一个并集

#### Scenario: A pack is stated without a reason

- **WHEN** §51.3 的某一行「依据」列为空
- **THEN** 对账 MUST 失败并指名该包——没有写下来理由的能力包，就是一个没人复核过的能力包

#### Scenario: The R1-retained discipline list is emptied or overlaps the packs

- **WHEN** §51.3 表后「保留在 R1 的基础纪律」那段列不出任何编号，或列出的编号与能力包并集相交
  （例如把 `H18` 从该段移进包行）
- **THEN** 对账 MUST 失败：分界判据（*一个确定性的上下文装配函数能不能满足它？*）必须两侧都非空且互斥，
  MUST NOT 因为「解析结果为空集」而**通过**

#### Scenario: The gate constant drifts from the baseline it mirrors

- **WHEN** 门控用的能力包常量集合与 §51.3 表格展开后的并集不再相同（常量漏一条或增一条）
- **THEN** 对账 MUST 失败并具名给出差异的编号与两侧来源，MUST NOT 只在「常量与 registry 相符」时通过

### Requirement: A baseline section is parsed inside its own bounds

按章节号取表的 `sectionTable` SHALL 把搜索域封闭在该节标题与**下一个层级不高于它的标题**之间，
表与「表后首段」MUST 落在域内。三种情形 MUST 分开具名：①该节标题不存在；②该节存在但**本节内没有表**；
③解析成功。情形② MUST NOT 回落到**邻节**的表，也 MUST NOT 用「解析出一张空表」代替。

#### Scenario: A section whose own table was removed or moved away

- **WHEN** §57.3 的标题仍在，而本节内的表被删除，或被一个更浅的标题挡开（表仍在文档里）
- **THEN** 对账 MUST 报「§57.3 has no table inside its section」，
  MUST NOT 把紧随其后那一节的表当作 §57.3 自己的表

#### Scenario: A deeper heading inside the section

- **WHEN** 该节内部出现层级更深的标题（如 `####`）
- **THEN** 它 MUST NOT 截断该节的搜索域，本节内的表仍然 MUST 被取到

#### Scenario: The three parsed sections still resolve

- **WHEN** 对基线正文解析 §48.5／§51.3／§57.3
- **THEN** 每一节的表 MUST 落在该节边界内——边界若被收得过紧，三节都会报「本节内无表」

#### Scenario: A prose section is stated as such, not remembered

- **WHEN** 对 §51.2 取表（W78 的规格刻意不读它）
- **THEN** MUST 得到「本节内无表」，使「刻意不读 §51.2」从**记性**变成**会被检查的前提**
