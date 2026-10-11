# admission-registry Specification

## Purpose
建立唯一的机器可读验收注册表，作为 V3.9 发布线（Slice-0/R1/R2/R3/实验/能力包）的单一事实源；自建 D01–D14 到新验收编号的映射与回填规则，落实 ADR 0015 对基线文本内部矛盾的勘误裁决，保证后续任务卡、认证与报告使用同一套编号且旧编号不再新增。

## Requirements

### Requirement: Single machine-readable admission registry

仓库 SHALL 维护 `tests/registry.yaml` 作为唯一验收来源，列出全部验收项并带 `release_line`、所属契约（如适用）、自动化等级、状态与证据引用；`release_line` MUST 使用独立字段区分验收编号与发布线名（slice_0、r1、r2、r3、experimental、**capability_pack**）。schema 校验 SHALL 拒绝重复编号、未知 release_line/status、悬空映射与新增的旧 T/E/U 编号。

#### Scenario: Duplicate or stale identifier
- **WHEN** 注册表中出现重复验收 ID 或新增旧 T/E/U 编号
- **THEN** 注册表校验失败，不进入后续切片任务

#### Scenario: Missing release line
- **WHEN** 某验收项缺少合法 release_line 或线号与 V3.9 §16 裁决不符
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

注册表 SHALL 覆盖基线（V3.7 §16 与 §39.5、V3.9 §48.5、V3.9 §51.3 与 §57.3）定义的**全部**验收家族——`S`/`R`/`A`/`B`/`O`/`P`/`M`/**`H`**/`X`。
基线把某编号体系段列为验收时，其条目 MUST 全部登记在 `tests/registry.yaml`；
**「尚未开始」MUST 以 `pending` 表达，MUST NOT 以之为由省略条目**——在册与测得是两个不同的陈述。
校验 SHALL 拒绝不属这些家族的编号（例如从既有条目拆分出的 `B01a`），
并 SHALL 断言每个「基线已定义完整编号段」的家族**逐号在册**（当前为 **M01–M18** 与 **H01–H18**，各 **18** 条）。
家族**在册**断言与发布线**分线**断言是两个不同的检查，MUST NOT 互相替代：前者证「条目没被省略」，
后者证「归属没被错线」。

#### Scenario: A baseline family is entirely absent

- **WHEN** 基线把某家族定义为验收编号体系的一部分（如开放平台与技能市场的 M01–M18，见 V3.7 §37.5/§38.10；
  或智能体 Harness 与开放互操作的 H01–H18，见 V3.9 §48.5），而 `tests/registry.yaml` 未登记其中任何条目
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

### Requirement: Release lines are declared, and the capability pack is never a first-release gate

发布线 SHALL 为 `slice_0`/`r1`/`r2`/`r3`/`experimental`/**`capability_pack`** 六条（V3.9 §51.1）。
`capability_pack` 的条目集合 MUST **恰为** **M01–M18 与 H05–H14、H16**（共 **29** 条，V3.9 §51.3、§57.3）。
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
