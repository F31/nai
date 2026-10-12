## Why

V3.9 §47.3 规定「harness 新机制**默认只在演练、影子与人工辅助下启用**，进入受监督自动 MUST 先通过 §47.2 的回归，
并在决策日志留下启用记录」；验收条目 **H18** 就是这一条。它在 `tests/registry.yaml` 里一直是 `pending`，
`notes` 写的是「未开始（W64）」——**没有任何机制层面判定面**：一个 harness 机制能不能进自动模式，
今天只能靠读代码和会议决定，无法被机器回答。

H18 属于 **R1 保留的 harness 基础纪律**（V3.9 §51.3：R1 保留 H01–H04、H15、H17、H18，其余降为能力包），
分界判据是「一个确定性的上下文装配函数能不能满足它？」——**爬坡门控满足**：它判的是
「给定机制、当前阶段、回归证据与启用记录，这次晋升是否被允许」，是纯逻辑。

本轮承接 W66：H15（观测）与 H17（降级）的判定面已落 `evaluate::harness`，
爬坡是同一节（§47「观测、评估与门控」）里**唯一仍不可判定**的第三块。

## What Changes

- 新增 `crates/nai-kernel/src/evaluate/ramp.rs`：把 §5.9／§47.3 的四段爬坡、§47.2 的六类回归、
  以及「启用记录」校验落成**纯逻辑判定面**，输出具名拒绝。
- 落**四段阶段**与「默认启用集合」的可判定形式（前段默认启用，受监督自动是唯一被门控的阶段）。
- 把「§47.2 的回归」落成**六类**可枚举类别，并要求晋升声明**受影响的类别**（`touched`），
  判定 `passed ⊇ touched`。
- 落**启用记录**的校验：必须存在、必须指名同一机制与目标阶段、值内不得含明文凭据（INV-11）；
  允许时给出可落日志的记录载荷。
- 落**回归失效**（`regression_stale`）：§47.2 的「通过相关回归**才生效**」意味着回归结论绑定 harness 修订，
  修订变化后晋升不再生效。
- **不接线**：真正按阶段启用机制、把记录追加进 `nai-storage` 的哈希链、执行 §47.2 黄金集，
  分别是 runtime 与 H16（能力包）的工作。本轮**不**改 H18 的 `status`。

### Modified Capabilities

- `agent-harness`：新增一条可判定的爬坡门控需求。

## Impact

- `crates/nai-kernel/src/evaluate/{mod,ramp}.rs`、`crates/nai-kernel/tests/ramp.rs`（新增集成测试目标）。
- `openspec/specs/agent-harness/spec.md`（新增一条 Requirement）。
- 文档：`docs/kernel.md`、`docs/development-plan.md`、`AGENTS.md`、`docs/reports/2026-10-12-w67-*.md`。
- `tests/registry.yaml`：H18 的 `notes` 改为「判定机制已交付，验收保持 `pending`」——**`status` 不动**。
- **无新增依赖、无新增契约名、无前端改动、无基线文本改动**。
