# W49 实测报告：O01/O05 的库内收尾（演示包就绪判定 + 升级成本显示）

- 日期：2026-10-11
- 变更：`openspec/changes/w49-demo-package-and-cost-display/`（归档后为 `openspec/changes/archive/2026-10-11-w49-demo-package-and-cost-display/`）
- 指南：[docs/demo-package.md](../demo-package.md)、[docs/ui.md](../ui.md)、[ADR 0044](../adr/0044-demo-package-readiness-boundary.md)
- 判据：`tests/registry.yaml` **O01**、**O05**（**两者均保持 `pending`**）

## 结论

`tests/registry.yaml` 里 O01 与 O05 是两条 `pending`，且是**外部依赖最少**的两条。两条的缺项都是
「仓库内可控的那一半还没做」：

- **O01** 自记「**演示包未建**」——`preset`/`演示包` 两个词在 `crates/`、`apps/`、`schemas/` 里**一次都没出现过**；
- **O05** 自记「真实升级 UI 与成本显示未建」——成本**模型**已在 W15 落地（`CostEstimate`，未知 ≠ 0）。

本轮把这两半落成**可执行、可断言、零新增依赖**的产物，并**不改两者的 `status`**——
W38 的 O02 先例：逻辑/界面出现**不等于**验收达标。

## 一、O01：演示包与可用性目标的判定机制

`crates/nai-kernel/src/demo.rs`（与 O03 的 `monitoring.rs`、O04 的 `device.rs` **同层同形**），
`crates/nai-kernel/tests/demo.rs` **19 项**。判 §615 演示包五要素与 §612 两条时间目标，**十六条**稳定标记：

| 分组 | 标记 |
|---|---|
| 形象（§615「**授权的**」） | `demo_avatar_missing`、`demo_avatar_without_license` |
| 其余要素齐备 | `demo_knowledge_missing`、`demo_scenario_missing`、`demo_provider_presets_missing`、`demo_cost_estimate_missing` |
| Provider 预设 | `provider_preset_without_credential`、`provider_preset_embeds_secret` |
| 一键连通测试 | `demo_connectivity_missing`、`connectivity_probe_is_stub`、`smoke_claims_acceptance` |
| 成本预估 | `cost_unknown_without_reason` |
| 演练记录（§612） | `drill_record_incomplete`、`drill_timeline_inconsistent`、`first_utterance_beyond_target`、`real_output_beyond_target` |

四条口径是本轮真正要定死的东西（[ADR 0044](../adr/0044-demo-package-readiness-boundary.md)）：

1. **未声明即条件化**：空描述符审计为空、读作 `Blocked`——尚未交付**不是违规**，是**未开始**
   （与 O03/O04 同纪律）。一旦声明任一部分，其余即成为**欠项**：半份演示包是**缺陷**。
2. **「授权的」是要求，不是形容词**：有形象而无授权引用 ⇒ `demo_avatar_without_license`。
3. **连通 ≠ 达标**：`connectivity_probe_is_stub` 判「测没测」，`smoke_claims_acceptance` 判「有没有把冒烟当验收」
   ——与 W36/W37 固化的纪律一致（**主机的连通成功不是端点的验收结果**）。
4. **演练记录的时间戳早于安装就绪是不自洽**（`drill_timeline_inconsistent`），不是「跑得很快」；
   **无**演练记录 ⇒ 审计为空、读作 `Blocked`（§1261：未运行的验收记 Blocked，不写成 Passed）。

`DemoReadiness` 三态且 **`Defective` 优先**：一份违规的声明不得因子项未演练而被读成 `Blocked`。
`every_finding_token_is_constructible` 断言**一组构造描述**的审计并集**恰为**这十六条——既保证每条可构造，
也保证没有未登记的额外标记。（用一组而非一份：`missing` 与 `defective` 对同一部分互斥，一份描述覆盖不到两者。）

**不新增 xtask 命令与证据目录**：§30.1 为 UI / RAG / 升级 / 媒体 / 故障 / 契约 / 集成 / 性能各指定了目录，
**演示包没有**；其真值来自真实演练与真实端点，不是仓库载荷。臆造 `tests/demo` 属过度绑定。

## 二、O05：升级成本显示的呈现层

`apps/desktop/src/upgrade.ts`（**零 DOM**，与 W38 的 `console.ts` 同形），`tests/ui/upgrade-cost.test.mjs` **19 项**。
把「**准确列示**」落成可断言的性质：

| 性质 | 断言 |
|---|---|
| 未知分项呈现为**原因**，绝不呈现为 0 | `未知分项呈现为原因，绝不呈现为 0`（断言该行文本内不出现 `0`） |
| 显式的 0 是一个断言，被如实呈现为 0 | `显式的 0 是一个断言，被如实呈现为 0` |
| 全已知 ⇒ 合计 | `全已知时总额是合计` |
| 含未知 ⇒ **下界 + 未知计数**，且**不是**总额 | `含未知分项时总额是下界加未知计数，不是总额`（断言文本不含「合计」） |
| 缺字段/未知字段/非法金额/空原因 ⇒ **具名拒绝** | 十条拒绝用例（缺分项、多分项、负金额、小数、字符串金额、空原因、全空白原因、两者并存、非法货币码、非对象） |

载荷形状镜像 `nai-upgrade-core::cost` 的语义（每个分项 `known` 与 `unknown` **二者择一**）。
**它不构成 O05 的完整验收**：控制面尚未暴露 `nai-upgrade-core` 的 `plan` 与成本来源
（与 §30.1 第 4 条「升级与回滚**提示**」**同一缺失前置**），故该模块**未接入任何界面**，真实渲染与生产接线未做。

## 三、验证

| 检查 | 结果 |
|---|---|
| `cargo fmt --all --check` | 干净 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 干净（首次撞 `clippy::collapsible_if`，已改 `if let … && …`） |
| `cargo xtask check-deps` | **18 个 workspace 包**通过（零新增依赖、零包增减） |
| `cargo xtask check --registry` | 通过（O01/O05 状态保持 `pending`） |
| `cargo xtask check --profile portable` | **EXIT=0**；**83 目标 = 66 `Running` + 17 `Doc-tests`、725 passed / 0 failed**（706 → 725，**恰 +19 = 本轮 demo 单测**；新增 `crates/nai-kernel/tests/demo.rs` 目标，`Running` 65 → 66） |
| `pnpm test:ui`（`tests/ui`） | **34 项全通过**（15 既有 + **19 本轮**） |
| `scripts/frontend-gate.mjs`（`check --profile full` 的前端步骤） | 六步全通过（两包 typecheck、两包 build、桌面包单测、`tests/ui`） |
| `openspec validate --all --strict` | **38 passed / 0 failed** |
| `openspec validate --all --strict --archived` | 见下（归档后） |

**平台边界与 W48 相反**：本轮的 kernel 判定与前端呈现层**没有任何 `#[cfg(target_os = ...)]`**，
portable 全程可测——**本机门控即可证其成立**，不依赖 CI 的某个平台 job。

## 四、如实披露

- **O01 与 O05 都保持 `pending`**。本轮交付的是**判定机制**与**呈现层**，**不是**两者的验收。
- **O01 的判定机制不替代真值**：授权的演示形象**内容**、示例知识与场景**正文**、一键连通的**真实接线**、
  约 10 分钟首句演练与约 30 分钟真实输出验证的**人工执行**，全部仍是外部条件。
  真实链路 W36/W37 的证据（`docs/reports/evidence/`）**不因本模块而升级**任何登记状态。
- **O05 的呈现层未接入界面**：没有成本来源可接（控制面未暴露升级计划），故它是**已断言的逻辑**而非**已渲染的界面**；
  `tests/ui` 证明的是**呈现口径**。真实升级 UI 的渲染与生产接线未做。
- **`readiness` 与 registry `status` 是两套词汇**：前者是**描述符**的就绪度（`Blocked`/`Defective`/`Ready`），
  后者是**轮次交付**的状态（`pending`）。二者不互相推导。
- **未做**：不建真实演示包内容；不跑人工演练；不接控制面升级状态来源；不改 `nai-upgrade-core` 与任何既有 `src/`
  （除 `nai-kernel/src/lib.rs` 挂模块）；不加 xtask 命令与约定外证据目录；不改任何既有验收 `status`。

## 五、登记与剩余

- 新增规格能力 **`demo-package`**（**9 条需求**）；**`local-control-surface`** 新增 1 条需求
  （升级成本逐项列示的呈现口径）。归档后并入 `openspec/specs/`。
- **O01** notes 追加判定机制与剩余项；**O05** notes 追加呈现层、**未满足项**（成本来源缺失 ⇒ 未接入界面）
  与「不因呈现层出现而升级」。
- **剩余（均需外部条件）**：O01 的演示包内容与人工演练；O05 的真实渲染与生产接线（取决于控制面暴露
  `nai-upgrade-core` 的 `plan`/成本来源）；O02 的「显示最终文本」（取决于存储侧保存可展示文本并同步契约）。
