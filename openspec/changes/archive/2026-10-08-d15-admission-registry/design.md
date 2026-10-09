# Design

## Context

V3.6 §16 定义了统一验收注册表（S/A/R/O/X），但存在内部矛盾（R 编号与发布线二义、§1.4 vs §17.3 口径不一、W03/W15 线号冲突、冻结规则循环），已在 ADR 0015 裁决。V3.6 §30.4 只映射旧 T/E/U 与附录 B，没有 D→新编号映射，需自建。

## Decisions

- `tests/registry.yaml` 是验收唯一事实源，由 `schemas/tests/registry.schema.json` 校验。每项字段：id、release_line（slice_0/r1/r2/r3/experimental）、contract（可选）、automation（unit/contract/integration/fault/ui/manual）、status（implemented/tested/blocked/not_applicable）、evidence（归档或报告引用）、notes。
- 验收编号 S01–S12、R01–R11、A01–A11、O01–O05、X01–X02 采用 V3.6 清单并落实勘误：以 §16 为 R1 口径准；W03 卡 R04 属 R1；R09/R10 属 R1；预留 provisional 契约冻结说明。
- `mapping` 段自建 D01–D14→新编号映射。规则：已完成并归档的实测项标 `tested` 并引用归档报告；仅实现未完整认证的标 `implemented` 并注明缺口；外部条件缺失标 `blocked`；尚未开始目标能力标 `not_applicable`（本 profile 未启用）或 `implemented`（仅内核前置）。
- 旧 T/E/U 编号不再新增，保留追溯注释；xtask `check --registry` 检测重复 id、未知 release_line/status、映射悬空、以及新增旧编号。

## Risks / Trade-offs

- 回填状态是追溯声明，不是重新认证 → evidence 必须指向真实归档与测试计数，未实测目标不因映射而升级为 tested。
- registry 初期覆盖 D01–D14 已验证项，Slice-0/R1 后续工作项逐步补充 → 允许未覆盖的验收项以 pending 存在，不做空占位。
- V3.6 勘误清单以 ADR 0015 为准，registry 与文档一致，避免出现两套编号。

## Verification

- `cargo xtask check --registry` 通过：全部验收项唯一、线号合法、映射目标存在、旧编号未新增。
- `cargo xtask check --profile portable`、schema --check、前端 typecheck/build 通过。
- OpenSpec strict validate + 归档校验通过。
