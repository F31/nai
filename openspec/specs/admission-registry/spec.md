# admission-registry Specification

## Purpose
建立唯一的机器可读验收注册表，作为 V3.6 切片发布线（Slice-0/R1/R2/R3/实验）的单一事实源；自建 D01–D14 到新验收编号的映射与回填规则，落实 ADR 0015 对 V3.6 内部矛盾的勘误裁决，保证后续任务卡、认证与报告使用同一套编号且旧编号不再新增。

## Requirements

### Requirement: Single machine-readable admission registry

仓库 SHALL 维护 `tests/registry.yaml` 作为唯一验收来源，列出全部验收项并带 `release_line`、所属契约（如适用）、自动化等级、状态与证据引用；`release_line` MUST 使用独立字段区分验收编号与发布线名（slice_0、r1、r2、r3、experimental）。schema 校验 SHALL 拒绝重复编号、未知 release_line/status、悬空映射与新增的旧 T/E/U 编号。

#### Scenario: Duplicate or stale identifier
- **WHEN** 注册表中出现重复验收 ID 或新增旧 T/E/U 编号
- **THEN** 注册表校验失败，不进入后续切片任务

#### Scenario: Missing release line
- **WHEN** 某验收项缺少合法 release_line 或线号与 V3.6 §16 裁决不符
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
