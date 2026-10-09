# Proposal

## Why

V3.6 已评审并确认为新基线（ADR 0015）。现有验收仍按 D 任务与 T/E/U 编号，V3.6 统一为 S/A/R/O/X 注册表并新增 release_line，且 §30.4 不提供 D→新编号映射。本轮建立唯一的机器可读验收注册表与 D→新编号映射，作为后续切片发布线的单一事实源。

## What Changes

- 新建 `tests/registry.yaml`：全部验收项（S/A/R/O/X + 补充项），带 `release_line`、契约、是否自动化、状态（implemented/tested/blocked/not_applicable）、回填引用。
- 新建 `schemas/tests/registry.schema.json`，xtask 增加 `check --registry` 校验注册表内部一致性（编号唯一、线号合法、映射不悬空、旧编号不再新增）。
- 自建 D01–D14 → 新编号映射（V3.6 §30.4 未提供），纳入 registry 的 `mapping` 段并记录回填规则。
- ADR 0015 生效：design.md/AGENTS/config.yaml 指向 V3.6，V3.5 降为历史参考；勘误清单落实（release_line 字段、R1 口径、预冻结中间态、值守租约初值、X01 时间点、日志缓冲取舍、回填规则）。

## Capabilities

### New Capabilities

- `admission-registry`: 机器可读验收注册表、D→新编号映射与一致性校验。

### Modified Capabilities

无。既有契约与规格不变；本变更仅切换验收与基线记录。

## Impact

新增 tests/registry.yaml、schemas/tests/registry.schema.json、xtask registry 检查模块；无新增常驻依赖、crate 或进程。
AGENTS.md、openspec/config.yaml、docs/design.md、docs/development-plan.md 同步更新。后续切片任务卡以 registry 验收为准。
