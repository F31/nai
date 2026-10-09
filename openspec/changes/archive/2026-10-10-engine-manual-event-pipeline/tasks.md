# Tasks

## 1. 规格与校验
- [x] 1.1 proposal/design/tasks/specs 完成并通过 `openspec validate engine-manual-event-pipeline --strict --no-interactive`。

## 2. Provider 接缝与桩
- [x] 2.1 `nai-provider-core` 新增 `TextGenerator` 接缝与确定性 rehearsal-only 桩，含 usage 记账；不改变 HTTP 适配器行为。
- [x] 2.2 测试：桩可复现、无网络、拒绝对外请求；接缝测试用桩驱动。

## 3. engine 编排
- [x] 3.1 手工事件源（显式 `manual` 标注）与有界归一化/去重队列。
- [x] 3.2 链路编排：知识检索 → 智能体决策 → 守门四阶段 → 文本生成 → 决策日志落库；每步复检 session/epoch/值守/审批哈希。
- [x] 3.3 失败顺序与注入：先 P0 静音 → 撤权 → critical 日志 → 有界退出；仅演练的故障注入开关。
- [x] 3.4 `--event-file` / `--rehearsal-provider` 参数与 stdout 结果行；不破坏既有 `--halt-file` 与 `--media-drill` 行为。

## 4. R07 桩回放
- [x] 4.1 同一手工事件 + 同一桩重放，比对决策日志与原次一致；不一致即判据失败。

## 5. 交付
- [x] 5.1 集成测试（真实 engine 进程）：哈希链校验、回放一致、陈旧 epoch/守门缺失/注入失败不外发。
- [x] 5.2 ADR 0022、docs/engine.md、docs/provider.md、tests/registry.yaml（R07/S01 备注，状态按判据判定）、docs/development-plan.md、AGENTS.md。
- [x] 5.3 匹配检查：workspace 测试、fmt/clippy、check-deps、portable、registry 通过；schema/contracts/typecheck/engine-smoke 因本机 node 无法 spawn 子进程（EBUSY）且无 pnpm 未执行，已如实记录（见报告「平台边界」）。
- [x] 5.4 报告 docs/reports/2026-10-09-engine-manual-event-pipeline.md，归档并 `validate --archived`。
