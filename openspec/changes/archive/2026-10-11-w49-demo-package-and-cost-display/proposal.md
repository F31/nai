## Why

`tests/registry.yaml` 里 O01 与 O05 是两条 `pending`，且是**外部依赖最少**的两条：

- **O01**（`manual`）：基线 §612「可用性目标：安装后约 10 分钟内听到第一句演练语音；约 30 分钟内完成一次
  真实输出验证」，§615「演示包：授权的演示形象、示例知识、示例场景，加 Provider 预设、一键连通测试与成本预估」。
  注册表自记「**演示包未建**」；真实链路 W36/W37 已打通并落了真值证据（`docs/reports/evidence/`），
  缺的是**演示包**与**人工演练**。
- **O05**（`ui`）：基线 §1330「升级成本显示：云资源、下载、存储与预估运行增量准确列示」。
  `CostEstimate`（`crates/nai-upgrade-core/src/cost.rs`）已实现且可测（未知 ≠ 0、部分和不当总额），
  缺的是**成本显示**本身。

两条的共同点是：**仓库内可控的那一半还没做**——O01 缺一个「演示包是否齐备且诚实」的判定，
O05 缺一个「把成本逐项准确列示」的呈现。其余部分（授权的演示形象内容、真实人工演练、真实渲染与生产接线）
都需要外部条件，**不在本轮范围**，如实登记。

本轮把这两半落成**可执行、可断言、零新增依赖**的产物，**不改两者的 `status`**（O02 的 W38 先例：
界面/逻辑出现不等于验收达标）。

## What Changes

- **新增 `crates/nai-kernel/src/demo.rs`**（判别式、纯函数）：把 §615 演示包五要素与 §612 可用性目标
  落成**就绪判定**——`DemoPackage`（描述符）＋`audit_demo_package`（稳定标记）＋`DemoReadiness` 三态。
  与 O03 的 `monitoring::audit_surface`、O04 的 `device::audit_host` 同形（同层、同形、同纪律）。
- **新增 `crates/nai-kernel/tests/demo.rs`**：每条规则可构造 + 「构造描述的审计并集**恰为**这些标记」，
  防未登记标记混入。
- **新增 `apps/desktop/src/upgrade.ts`**（零 DOM 的可测逻辑）：把成本载荷**严格解析**为
  「逐项呈现行 + 总额呈现」；**未知分项显示为「未知：<原因>」而非 0**；**部分和显示为下界 + 未知计数，
  绝不冒充总额**。与 W38 的 `console.ts` 同形（Node `--experimental-strip-types` 直接驱动）。
- **新增 `tests/ui/upgrade-cost.test.mjs`**：断言上述性质与严格解析。
- 文档：新增 `docs/demo-package.md`；更新 `docs/ui.md`、`docs/development-plan.md`、`AGENTS.md`、
  `tests/registry.yaml`（O01/O05 的 notes 追加、**状态保持 `pending`**）。
- 不做：不建真实演示包内容（授权形象、真实示例知识正文）；不跑人工演练；不接控制面升级状态来源；
  不改 `nai-upgrade-core` 与任何既有 `src/`；不新增依赖；不加 xtask 命令与约定外证据目录。

## Capabilities

### New Capabilities

- `demo-package`：演示包与可用性目标的**就绪判定**——五要素齐备性、授权引用、预设不内嵌明文密钥、
  连通测试必须是真实探针（**连通 ≠ 达标**）、成本预估未知须说明原因，以及 §612 的两条时间目标。

### Modified Capabilities

- `local-control-surface`：新增一条需求——升级成本**逐项列示**的呈现口径（未知 ≠ 0、部分和不作总额），
  并明确它**只**保证呈现口径，不构成 O05 的完整 UI 验收。

## Impact

- 新增：`crates/nai-kernel/src/demo.rs`、`crates/nai-kernel/tests/demo.rs`、
  `apps/desktop/src/upgrade.ts`、`tests/ui/upgrade-cost.test.mjs`、`docs/demo-package.md`、
  `openspec/specs/demo-package/spec.md`（经归档吸收）、`openspec/specs/local-control-surface/spec.md`（同）。
- 修改：`crates/nai-kernel/src/lib.rs`（`pub mod demo;`）、`docs/ui.md`、`docs/development-plan.md`、
  `AGENTS.md`、`tests/registry.yaml`。
- **零新增依赖**：kernel 只依赖 contracts + prost + serde_json（`demo.rs` 只用 std）；
  前端不引入 DOM 库，沿用 Node 22 内置 `node:test` 与 `--experimental-strip-types`。
- **零 workspace 包增减、零 CI workflow 改动**；`check-deps` 仍为 18 包。
- **登记纪律**：O01 与 O05 保持 `pending`。本轮交付的是**判定/呈现逻辑**，
  **不**升级 registry 状态到证据之上（O02 的 W38 先例）。
- **验证**：本机门控（`check-deps` / `check --registry` / `check --profile portable`）、
  `pnpm test:ui`、`openspec validate --all --strict --archived`；CI 观测针对既有门控。
