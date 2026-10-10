## Why

`tests/registry.yaml` 里 **B02**（写实 Gate G0–G3，R2）与 **D12**（模型准入）都是 `blocked`：仓库里没有合法完整的 ONNX 模型，也没有 Windows/CUDA 目标机证据，§8.2 的四门验收**不可能在本轮达成**。这一部分真值必须留在外面，本轮不假装有。

但 §8.2 表里四行的口径本身，是**可以现在落成代码的**，不落就会在下面几处悄悄含糊过去：

- **「写实 Gate G0–G3」是一个四门合取，不是四个百分比。** §8.2 把 G0 授权、G1 逐段对齐、G2 目标机实测、G3 七种稳定性场景列成四行独立证据。若把它压成一个进度分或让一门的结果去补另一门，就无法回答「卡在哪一门」。
- **「实测帧率、延迟、显存、CPU、温度」是四个都要实测。** 只测了帧率就宣称实时性达标，是把**未测量**读成了通过（同 W26 的 `Unmeasured` 容量、W29 的 `unmeasured` 字段、W30 的 `None` 分位）。
- **「不以 CUDA EP 注册成功宣称全 GPU」（§26.5）是一句禁止令。** 全 GPU 是**放置记录**的性质，不是运行时注册的回执；把它写成 `cuda_ep_registered: bool` 然后当结论用，正是原文点名要否决的做法。
- **「自动回退 2D 不等于写实验收通过」（§8.2）是一条硬边界。** 回退是**运营动作**（§8.4：需预先授权、在句间切换、写日志），不是验收结论；若回退能补足一门未过的证据，写实 Gate 就失去意义。
- **「没有合法完整模型时，写实认证标 Blocked」（§1311）与「NotApplicable 不能代替未完成的目标能力」是同一条纪律。** 缺模型必须是 Blocked，不能是「不适用」，也不能被豁免。

## What Changes

- **四门独立判定（`Gate` / `GateVerdict`，B02）**：`Gate::{SourceAndRights, Correctness, Realtime, Stability}` 各自给出 `Passed | Failed | Unmeasured`；总体准入由四门**派生**（`Acceptance::{Accepted, NotAccepted, Unmeasured, Blocked, NotApplicable}`），**没有任何单分数**。任一 `Unmeasured` 即阻断 `Accepted`。

- **G0 来源与授权要五类权利 + 完整链路（`RightsEvidence`，B02）**：权重、代码、训练素材、肖像、声音五类各自有 `Established { basis } | Unverified | Absent`。任一 `Absent`、完整链路不可取得、或存在**被留在未声明 Python 服务中的缺组件** → `Failed`。**空白依据不算成立**——「许可引用存在」不等于证据通过（D12 报告自身结论），此类权利使 G0 为 `Unmeasured` 并被审计报出。

- **G1 正确性逐段对齐且精度有序（`CorrectnessEvidence`，B02）**：预处理、音频特征、生成、后处理四段在**固定输入**下各自对齐；任一段发散 → `Failed`，任一段未运行或固定输入未指明 → `Unmeasured`。精度按 §26.5 先 FP32 再 FP16，**未过 FP32 就认证 FP16 → `Failed`**；**INT8 无量化与校准报告 → `Failed`**。

- **G2 实时性绑定目标机与并发条件（`RealtimeEvidence`，B02）**：目标机未指明 → `Unmeasured`；未与直播工具同时运行 → `Failed`；帧率判是否稳定达 `MIN_STABLE_FPS = 25`，音画偏差复用 W29 的 `LatencySamples::percentile(P95)` 判是否 ≤ `FRAME_SKEW_P95_LIMIT_MS = 80`（<20 样本 → `Unmeasured`，绝不产生一个数）；显存/CPU/温度任一未测得 → 不为 `Passed`。

- **全 GPU 是放置记录的性质（`PlacementReport`，B02）**：`is_full_gpu()` 仅当放置记录非空**且全部为 GPU** 时为真，**与 `cuda_ep_registered` 无关**；`cuda_ep_registered = true` 而无放置记录 → 审计报出**注册不等于放置**；宣称全 GPU 而存在 CPU 回落 → `Failed`。

- **G3 稳定性覆盖七场景且需证据（`StabilityEvidence`，B02）**：静音、取消、句边界、长播、OOM、进程重启、形象切换与输出恢复；任一 `Failed` → `Failed`，任一未运行 → `Unmeasured`，**声明通过而无证据引用 → `Failed`**。

- **2D 回退是一个运营记录，不是验收结论（`FallbackRecord`，B02）**：回退**不改变任何门的判定**；「以 2D 回退构成写实验收」的声明永远违规（`fallback_claimed_as_acceptance`）；回退缺运营授权/不在句间/未写日志逐项被审计报出。

- **缺模型 → Blocked，且不可豁免（`acceptance` / `audit_gate`，B02）**：报告标记模型不完整时，总体准入强制为 `Blocked`；此时若任一门口径为 `Passed`，审计报出 `gates_evaluated_without_model`。「不适用」仅在档位**不要求写实**（Lite）时成立；Standard/Pro 声明「不适用」被审计报出。

- **审计是报告字段的纯函数（`audit_gate`，B02）**：`full_gpu_claimed_with_cpu_fallback`、`full_gpu_claimed_without_placement_records`、`cuda_ep_registered_without_placement`、`established_right_without_basis`、`precision_without_fp32_baseline`、`int8_without_calibration`、`undeclared_component`、`scenario_passed_without_evidence`、`gates_evaluated_without_model`、`fallback_claimed_as_acceptance`、`fallback_without_authorization`、`fallback_outside_sentence_boundary`、`fallback_unlogged`、`not_applicable_for_target_capability`。每条都可直接构造，不被判定函数架空。

## Capabilities
### New Capabilities
- `realistic-gate`：§8.2 四门（G0 来源与授权 / G1 正确性 / G2 实时性 / G3 稳定性）的独立判定、缺测不等于通过、精度认证顺序、全 GPU 的放置记录口径、2D 回退不构成验收、缺模型 Blocked、NotApplicable 只用于不要求写实的档位、报告自洽审计。覆盖 registry **B02**（并回指 **D12**）。

### Modified Capabilities
- 无。既有 `model-package-integrity`（D12 前置）**不改**：它管的是**声明与内容完整性**（子图闭包、权重哈希、运行时矩阵），本变更管的是**完整性通过之后 G0–G3 的判定**；两者是「完整性 ≠ 认证」的前后两段，`model-package-integrity` 的第四条需求已明确「成功结果不授予 G0–G3」。本变更**消费**一份完整性已通过的模型所对应的外部证据，不读取也不校验模型文件。既有 `operations-review`（W29）**不改**：本变更只复用其分位实现（`LatencySamples`），语义是 G2 的帧偏差门槛。

## Impact

- 代码：`crates/nai-kernel/src/` 新增 `realistic.rs`；`tests/` 新增 `realistic.rs`；`lib.rs` 增一个 `pub mod`。`golden.rs` 的 `Finding::new` 由 `fn` 提升为 `pub fn`（复用同一报告词汇，不新增令牌类型）。**不新增 crate，不改 workspace 依赖**（维持 18 包与依赖方向；kernel 仍只依赖 contracts）。
- 规格：新增 capability `realistic-gate`；新增 `docs/realistic-gate.md`、ADR 0034、报告 `docs/reports/2026-10-10-w32-realistic-gate-verdict.md`。
- 依赖：**零新增**。仅用 `nai_contracts::v1::ErrorCode` 与 `std`；分位复用同在 kernel 的 `metrics`，报告词汇复用同在 kernel 的 `golden::Finding`（不复制实现）。
- registry：**B02 保持 `blocked`**（§655 的「证据齐全」是外部真值，本轮不可达），notes 补记 W32 已交付判定机制与阻塞边界；**D12 保持 `blocked`**。不新增编号（registry id 正则为恰好两位数字）。
- 明确不做：不提供任何真实权重、推理、目标机测量；不接 Windows/CUDA/ORT；不做 UI；不改 `model-package-integrity` 的完整性检查；不引入 `check --profile portable` 的新命令（证据目录无 §30.1 指定位置，不臆造）。

## Non-Goals

- 不用机制替代真实证据：本轮证明的是「四门口径可执行、缺测不被读成通过、回退不被读成验收」，**不宣称任何真实模型已过 Gate**。B02 与 D12 均保持 Blocked。
- 不给写实进度分：四门各自可见，不提供任何加权求和或单一「认证度」字段。
- 不让未测量变成零或通过：缺测一律 `Unmeasured` 且阻断 `Accepted`。
- 不让 CUDA EP 注册冒充全 GPU：全 GPU 只由放置记录得出。
- 不让 2D 回退冒充验收：回退是运营记录，与准入门控正交。
