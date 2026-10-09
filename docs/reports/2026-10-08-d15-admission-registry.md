# D15 验收注册表与基线切换实测报告

- 日期：2026-10-08；环境：Linux/WSL，项目锁定 Rust/Node/pnpm。
- 基线：V3.6 经 ADR 0015 升为唯一实现基线；V3.5 降为历史参考。
- 结论：验收唯一来源 tests/registry.yaml 已建立并实测，D01–D14 映射与回填规则落地；决策日志/值守租约为下一轮前置。

## 背景与裁决

V3.6 经评审（opencode + kimi 复审）确认为新基线。ADR 0015 记录三项决策与勘误清单：

1. **V3.6 升基线**，V3.5 降为历史；docs/design.md、AGENTS.md、openspec/config.yaml 同步指向 V3.6。
2. **勘误裁决**：release_line 独立字段避免 R 编号与 R1/R2/R3 发布线二义；R1 退出条件以 §16 为准；W03/W15 线号以 §16 为准；引入 provisional 契约冻结中间态；值守租约失效后 600ms 内撤销输出授权；决策日志缓冲满丢弃遥测段但关键提交先持久化；X01 门槛为 M0b 前写定；回填规则为「实测项 tested + 引用归档」。
3. **命名保持 nai-\***：V3.6 的 dh-* 仅为方案建议名，不重命名仓库/crates/包/契约，避免污染 14 份归档与生成产物。

## 实现

- `schemas/tests/registry.schema.json`：验收注册表 JSON Schema（release_lines/acceptance/mapping）。
- `tests/registry.yaml`：44 项验收（S01–S12、R01–R11、A01–A11、B01–B06、O01–O05、P01、X01–X02），带 release_line、契约、自动化等级、状态、证据；D01–D14 映射段与回填规则。
- `scripts/contracts/registry.mjs` + `tests/registry.test.mjs`：机器可读校验（编号唯一、线号/状态合法、映射目标存在、旧 T/E/U 不再新增）。
- `xtask check --registry` 新增并接入 `check --profile portable`。
- design.md/AGENTS/config.yaml/development-plan 更新基线条款与 D 表追溯说明。

## 实测

| 检查 | 结果 |
|---|---|
| `cargo xtask check --registry` | Passed：registry.yaml 内部一致，映射不悬空 |
| `cargo xtask check --profile portable` | Passed：222 项 Rust 测试、依赖边界、fmt、Clippy、registry 检查 |
| `cargo xtask schema --check` | Passed：生成无漂移（本轮无契约变更） |
| `pnpm typecheck` / `pnpm build` | Passed |
| OpenSpec strict validate（实施前） | Passed |
| 实施后 strict validate / archive / spec:validate / archived validate | Passed：17 个主规格、15 项归档 |

## 回填摘要

D01–D11 标 tested（引用各自归档报告）；D12 标 blocked（缺完整合法模型/G0–G3）；D13 标 implemented（准入逻辑通过，SDK/网关 blocked）；D14 标 tested（loopback 闭环，审批/预检未建）。
回填是追溯声明，不重新认证；未实测目标能力保留 pending/blocked，未因映射升级为 tested。

## 状态与边界

- **已实现/实测**：验收注册表、D→映射、xtask 检查、基线文档切换。
- **未开始（下一轮）**：decision_log 哈希链（R07 前置，播放 ACK 段 Draft）、值守租约（含 600ms 短期限初值实现验证）、故障域表补测（进程级 kill 缺）。
- **保持 Blocked**：B02 写实 Gate（缺模型）、P01 平台生命周期（缺 SDK）、O03/O04（手机包/安装）、B05 真人音轨、R05/R09/R10 升级器。
- **NotApplicable**：S12 虚拟人标识（依赖平台规则）、X01/X02 实验（Lite-Mobile 未立项）。

本变更已归档为 `openspec/changes/archive/2026-10-08-d15-admission-registry/`，主规格 `admission-registry` 生效。
