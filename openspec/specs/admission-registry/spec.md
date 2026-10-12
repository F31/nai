# admission-registry Specification

## Purpose
建立唯一的机器可读验收注册表，作为**当前基线**发布线（Slice-0/R1/R2/R3/实验/能力包）的单一事实源；自建 D01–D14 到新验收编号的映射与回填规则，落实 ADR 0015 对基线文本内部矛盾的勘误裁决，保证后续任务卡、认证与报告使用同一套编号且旧编号不再新增。

## Requirements

### Requirement: Single machine-readable admission registry

仓库 SHALL 维护 `tests/registry.yaml` 作为唯一验收来源，列出全部验收项并带 `release_line`、所属契约（如适用）、自动化等级、状态与证据引用；`release_line` MUST 使用独立字段区分验收编号与发布线名（slice_0、r1、r2、r3、experimental、**capability_pack**）。schema 校验 SHALL 拒绝重复编号、未知 release_line/status、悬空映射与新增的旧 T/E/U 编号。

#### Scenario: Duplicate or stale identifier
- **WHEN** 注册表中出现重复验收 ID 或新增旧 T/E/U 编号
- **THEN** 注册表校验失败，不进入后续切片任务

#### Scenario: Missing release line
- **WHEN** 某验收项缺少合法 release_line 或线号与 V3.10 §16 裁决不符
- **THEN** 校验失败并给出可读原因

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

发布线 SHALL 为 `slice_0`/`r1`/`r2`/`r3`/`experimental`/**`capability_pack`** 六条（V3.10 §51.1）。
`capability_pack` 的条目集合 MUST **恰为** **M01–M18 与 H05–H14、H16**（共 **29** 条，V3.10 §51.3、§57.3）。
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
基线文档 SHALL 取 `docs/` 中版本号最大者，且 MUST 与 `openspec/config.yaml` 声明的一致。
对账 MUST NOT 是空洞的：章节号被改、该节缺失、计数行与表格不符，均 MUST 分别具名失败。

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
