# 写实模型准入 Gate 的判定骨架（B02）

代码：`crates/nai-kernel/src/realistic.rs`；测试：`crates/nai-kernel/tests/realistic.rs`；
决策：[ADR 0034](adr/0034-realistic-gate-verdict-boundary.md)。

本模块把 §8.2「写实模型准入 Gate G0–G3」的**判定口径**落成在 portable 下可执行、可测试的骨架。它**不产生**任何真实权重、推理或目标机测量——那些由外部条件（合法完整模型 + Windows/CUDA 目标机 + 直播工具并发环境）提供；它负责的是「给定一份证据，四门各自如何判、总体如何派生、缺测与回退如何表达」。

因此 `tests/registry.yaml` 的 **B02 与 D12 保持 `blocked`**：§655 的验收原文是「写实 Gate G0–G3 与肖像、声线授权证据齐全」＝证据齐全并实测，外部证据不存在时不可能达成。本文档描述的是**判定机制**，不是认证结论。

## 四门是合取，且各自独立

§8.2 的表只有四行，每行一门：

| 门 | 枚举 | §8.2 证据 |
| --- | --- | --- |
| G0 来源与授权 | `Gate::SourceAndRights` | 权重、代码、训练素材、肖像、声音的使用权利成立，完整流程可取得 |
| G1 正确性 | `Gate::Correctness` | 原实现与 Rust 流程在固定输入下逐段对齐：预处理、音频特征、生成、后处理 |
| G2 实时性 | `Gate::Realtime` | 目标机上与直播工具同时运行，实测帧率、延迟、显存、CPU、温度 |
| G3 稳定性 | `Gate::Stability` | 静音、取消、句边界、长播、OOM、进程重启、形象切换与输出恢复 |

每门给出 `GateVerdict::{Passed, Failed, Unmeasured}`。总体准入 `Acceptance` 由四门**派生**，**没有任何单分数方法**：

| 条件（按序） | `Acceptance` |
| --- | --- |
| 档位不要求写实且声明未启用 | `NotApplicable` |
| 模型不完整 | `Blocked` |
| 任一门 `Failed` | `NotAccepted` |
| 任一门 `Unmeasured` | `Unmeasured` |
| 四门全 `Passed` | `Accepted` |

档位 `Profile::{Lite, Standard, Pro}` 中，`requires_realistic()` 仅对 Standard/Pro 为真（§8.1：2D 是 Lite 默认，写实是 Standard 的增量）。

## 缺测是第三种状态

**「没测」既不是通过，也不是零。** 帧率无窗口、音画偏差样本不足 `MIN_PERCENTILE_SAMPLES`、温度未读——一律 `Unmeasured`，且任何 `Unmeasured` 门都会把总体挡在 `Accepted` 之外。这与 W26 的 `Unmeasured` 容量、W29 的 `unmeasured` 字段、W30 的 `None` 分位同形。

## G0：五类权利 + 完整链路，依据必须非空

`RightKind::{Weights, Code, TrainingData, Portrait, Voice}` 五类各自有状态：

- `RightStatus::Established { basis }`：依据**必须非空白**才算成立。**「许可引用存在」不等于证据通过**（D12 报告自身的结论）——空白依据使该类权利不成立，G0 判 `Unmeasured`，并由审计报出 `established_right_without_basis`。
- `RightStatus::Unverified` / `Absent`：未核实 / 明确缺失。`Absent`（或根本没记录）使 G0 判 `Failed`，`missing()` 逐项指名。
- `undeclared_components` 非空（§8.2「不得把缺失组件留在未声明的 Python 服务里」）→ `Failed`。
- `pipeline_obtainable = false`（完整链路不可取得）→ `Failed`。
- 五类全部成立且链路可取得 → `Passed`。

## G1：四段对齐，精度有序

`Stage::{Preprocess, AudioFeatures, Generation, Postprocess}` 四段各有 `StageAlignment::{Aligned, Diverged{max_abs_delta}, NotRun}`：

- 任一段 `Diverged` → `Failed`；
- 缺组件留未声明 Python 服务 → `Failed`；
- **精度顺序**（§26.5「先用 FP32 与原流程逐段对齐，再认证 FP16」）：认证任何非 FP32 精度（FP16 或 INT8）而 FP32 未通过 → `Failed`（`precision_without_fp32_baseline`）；
- **量化**（§26.5「INT8 必须有量化与校准及质量报告」）：认证 INT8 而无 `int8_calibration_report`（或报告空白）→ `Failed`（`int8_without_calibration`）；
- 固定输入未指明（`fixed_inputs` 空白）或任一段 `NotRun` → `Unmeasured`；
- 四段全 `Aligned`、固定输入已指明、精度合规 → `Passed`。

## G2：目标机 + 并发条件 + 放置记录

- **目标机**：`target_machine` 空白 → `Unmeasured`；
- **并发条件**（§8.2「与直播工具同时运行」）：`concurrent_with_live_tool = false` → `Failed`；
- **帧率**：`FrameRate` 是一组 `FrameWindow{frames, elapsed_ms}`；§12.5 要求「**稳定** 25fps」，故判据是**窗口的下限** `min_fps_permille()`，不是会被突发拉高的平均；低于 `MIN_STABLE_FPS = 25` → `Failed`，无窗口 → `Unmeasured`；
- **音画偏差**：复用 W29 的 `LatencySamples::percentile(P95)`（同一个 nearest-rank 实现，一个真相）；> `FRAME_SKEW_P95_LIMIT_MS = 80` → `Failed`，样本不足 → `Unmeasured`（绝不产生一个数）；恰好 80ms 判达标；
- **显存 / CPU / 温度**：任一未读 → `Unmeasured`（§8.2 四项都要**实测**）；
- **全 GPU 是放置记录的性质**（§26.5「不以 CUDA EP 注册成功宣称全 GPU」）：`PlacementReport::is_full_gpu()` 仅当放置记录**非空且全部为 GPU**；`cuda_ep_registered` 只是审计线索。宣称全 GPU 而无放置记录、或存在 CPU 回落 → `Failed`。**如实记录** CPU 回落（`claims_full_gpu = false`）且指标达标 → 仍可 `Passed`。

## G3：七场景，通过须有证据

`StabilityScenario` 七枚举即 §8.2 G3 的七项。`ScenarioResult::{Passed{evidence}, Failed{reason}, NotRun}`：

- 任一 `Failed` → `Failed`；
- **声明 `Passed` 而证据引用空白** → `Failed`（`scenario_passed_without_evidence`）——「引用存在不等于证据通过」在场景层的推广；
- 任一未运行/缺失 → `Unmeasured`；
- 七场景全 `Passed` 且各有证据 → `Passed`。

## 2D 回退是运营记录，与准入正交

§8.2「**自动回退 2D 不等于写实验收通过**」；§8.4 回退「需运营预先授权，在句间切换并写日志」。落法：

- `FallbackRecord` **不进入** `verdict()` 或 `acceptance()` 的任何计算——回退既不能补足一门未过的证据，也不会本身使报告无效；
- 声明「以 2D 回退构成写实验收」（`acceptance_claimed_via_fallback`）**永远违规**：审计报出 `fallback_claimed_as_acceptance`；
- 回退缺运营授权 / 不在句间 / 未写日志 → 分别报出 `fallback_without_authorization` / `fallback_outside_sentence_boundary` / `fallback_unlogged`。

## 缺模型 → Blocked；不适用只用于不要求写实的档位

§1311「没有合法完整模型时，写实认证标 Blocked」与「NotApplicable 不能代替未完成的目标能力」：

- `model_present = false` → 总体 `Blocked`；此时若某门判 `Passed`，审计报出 `gates_evaluated_without_model`；
- 档位**不要求**写实（Lite）且声明未启用 → `NotApplicable`；
- 档位**要求**写实（Standard/Pro）而声明不适用 → 审计报出 `not_applicable_for_target_capability`，且总体**不得**为 `NotApplicable`；
- **无豁免路径**：没有任何 API 能把一个未过的门转成通过。

## 审计是报告字段的纯函数

`audit_gate(&GateReport)` 对报告自身字段做纯函数，报出**全部**命中项，使每条规则都能被直接构造（不被 `verdict()`/`acceptance()` 的构造保证架空）：

| token | 含义 |
| --- | --- |
| `established_right_without_basis` | 某类权利标为成立但依据空白 |
| `undeclared_component` | 有组件留在未声明的 Python 服务 |
| `precision_without_fp32_baseline` | 非 FP32 精度在 FP32 之前被认证 |
| `int8_without_calibration` | 认证 INT8 而无量化/校准报告 |
| `cuda_ep_registered_without_placement` | 只有 EP 注册、无放置记录 |
| `full_gpu_claimed_without_placement_records` | 宣称全 GPU 但无放置记录 |
| `full_gpu_claimed_with_cpu_fallback` | 宣称全 GPU 但存在 CPU 回落 |
| `scenario_passed_without_evidence` | 稳定性场景声明通过而无证据 |
| `gates_evaluated_without_model` | 模型缺失却有门判通过 |
| `fallback_claimed_as_acceptance` | 声明以 2D 回退构成验收 |
| `fallback_without_authorization` | 回退未经运营预先授权 |
| `fallback_outside_sentence_boundary` | 回退不在句间 |
| `fallback_unlogged` | 回退未写日志 |
| `not_applicable_for_target_capability` | 要求写实的档位声明不适用 |

## 未做边界

- **不含任何真实证据**：真实权重（含肖像、声线授权文件）、Rust/ORT 推理、Windows/CUDA 目标机实测、与直播工具的并发测量都由外部提供。测试里的证据是**构造出来的**，只为证明四门口径与派生可执行。
- **不接 ORT / CUDA / 直播工具**：不加载模型、不做推理、不读 GPU；`PlacementReport` 与各项采样均来自调用方。
- **不新增 xtask 命令与证据目录**：§30.1 只为 RAG 黄金集指定了 `tests/rag-golden`；为写实 Gate **臆造**一个证据目录属于过度绑定。G2 的真值来源是目标机实测，不是仓库内载荷。待真实模型轨立项并确定证据落点后，再补载体，**无需改本模块判定**。
- **不改 `model-package-integrity`**（D12 前置）：它管**声明与内容完整性**（子图闭包、权重哈希、运行时矩阵），本模块管**完整性通过之后 G0–G3 的判定**；其第四条需求已明确「成功结果不授予 G0–G3」。
- **不给写实进度分**：四门各自可见，不提供加权求和或单一「认证度」。
- 注册表 **B02 / D12 保持 `blocked`**：判定机制（W32）已交付并在 notes 中保留。
