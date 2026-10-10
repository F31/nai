## 1. 立项与骨架

- [x] 1.1 OpenSpec 变更 `w32-realistic-gate-verdict`（proposal/design/tasks + `realistic-gate` 增量），`validate --strict` 通过。
- [x] 1.2 落点确认：放进 `crates/nai-kernel`（`realistic.rs`；kernel 只允许依赖 contracts，`xtask/src/deps.rs`），不放进 plugin-sdk/shared 层、不新开 crate——18 包与依赖方向不变；复用 `MetricsError{code, reason}`、`metrics::LatencySamples`（G2 音画偏差分位）与 `golden::Finding`（报告词汇），**不复制实现**。

## 2. 四门与总体派生（B02）

- [x] 2.1 `Gate::{SourceAndRights, Correctness, Realtime, Stability}`（`ALL` / `label`，标签 `g0_source_and_rights` … `g3_stability`）；`GateVerdict::{Passed, Failed, Unmeasured}`。
- [x] 2.2 `Acceptance::{Accepted, NotAccepted, Unmeasured, Blocked, NotApplicable}`；`GateReport::acceptance()` 由四门派生——缺模型强制 `Blocked`、档位不要求写实且声明未启用 → `NotApplicable`、任一 `Failed` → `NotAccepted`、任一 `Unmeasured` → `Unmeasured`、否则 `Accepted`；**无任何单分数方法**。
- [x] 2.3 `GateReport::verdict(gate)` 逐门返回三态；`Profile::{Lite, Standard, Pro}` 与 `Profile::requires_realistic()`（Lite 为假、Standard/Pro 为真）。
- [x] 2.4 测试：三门通过一门缺测 → `Unmeasured`；任一失败 → `NotAccepted`；四门全通过 → `Accepted`；缺模型 → `Blocked`；Lite 未启用 → `NotApplicable`。

## 3. G0 来源与授权

- [x] 3.1 `RightKind::{Weights, Code, TrainingData, Portrait, Voice}`（`ALL` / `label`）；`RightStatus::{Established{basis}, Unverified, Absent}`。
- [x] 3.2 `RightsEvidence`：五类权利状态 + `pipeline_obtainable` + `undeclared_components`；`established(kind)` 要求 `Established` **且依据非空白**。
- [x] 3.3 G0 判定：任一 `Absent` → `Failed`；`!pipeline_obtainable` → `Failed`；`undeclared_components` 非空 → `Failed`；五类全部 `established` → `Passed`；否则 `Unmeasured`。
- [x] 3.4 测试：五类齐全且链路可取得 → 通过；缺肖像 → 失败并指名；空白依据 → 不通过且审计报出；留组件在未声明 Python 服务 → 失败。

## 4. G1 正确性与精度

- [x] 4.1 `Stage::{Preprocess, AudioFeatures, Generation, Postprocess}`（G1 四段，`ALL` / `label`）；`StageAlignment::{Aligned, Diverged{max_abs_delta}, NotRun}`。
- [x] 4.2 `CorrectnessEvidence`：四段对齐 + `fixed_inputs` + `certified_precisions` + `int8_calibration_report` + `undeclared_components`；`Precision::{Fp32, Fp16, Int8}`。
- [x] 4.3 G1 判定：任一 `Diverged` → `Failed`；缺组件留未声明 Python 服务 → `Failed`；FP16 已认证而无 FP32 → `Failed`；INT8 认证而无校准报告 → `Failed`；任一段 `NotRun` 或 `fixed_inputs` 空白 → `Unmeasured`；四段全 `Aligned` → `Passed`。
- [x] 4.4 测试：四段对齐且 FP32 → 通过；生成段发散 → 失败；后处理未运行 → 缺测；仅认证 FP16 → 失败；认证 INT8 无校准 → 失败；固定输入空白 → 缺测。

## 5. G2 实时性与放置

- [x] 5.1 `FrameRate{frames, elapsed_ms}` 与 `fps_permille()`；`MIN_STABLE_FPS = 25`、`FRAME_SKEW_P95_LIMIT_MS = 80`。
- [x] 5.2 `Device::{Gpu, Cpu}`、`NodePlacement{node, device}`、`PlacementReport{cuda_ep_registered, node_placements, gpu_copies, claims_full_gpu}`；`cpu_fallbacks()`、`is_full_gpu()`（**非空且全 GPU**，与注册无关）。
- [x] 5.3 `RealtimeEvidence{target_machine, concurrent_with_live_tool, frame_rate, frame_skew: LatencySamples, vram_mb, cpu_pct, temperature_c, placement}`。
- [x] 5.4 G2 判定：虚假全 GPU 声明 → `Failed`；未与直播工具并发 → `Failed`；帧率低于 25fps 或音画偏差 P95 超 80ms → `Failed`；目标机空白、帧率未测、分位样本不足、显存/CPU/温度任一未测 → `Unmeasured`；否则 `Passed`。
- [x] 5.5 测试：全达标 → 通过；非目标机 → 缺测；未并发 → 失败；24fps 与 81ms → 失败；恰好 25fps 与 80ms 判达标；缺温度 → 缺测；`cuda_ep_registered` 无放置记录却宣称全 GPU → 失败；有 CPU 回落却宣称全 GPU → 失败；有 CPU 回落但如实记录且指标达标 → 通过。

## 6. G3 稳定性与 2D 回退

- [x] 6.1 `StabilityScenario` 七枚举（`Mute`/`Cancel`/`SentenceBoundary`/`LongBroadcast`/`Oom`/`ProcessRestart`/`AvatarSwitchAndOutputRecovery`，`ALL` / `label`）；`ScenarioResult::{Passed{evidence}, Failed{reason}, NotRun}`；`StabilityEvidence`。
- [x] 6.2 G3 判定：任一 `Failed` → `Failed`；任一 `Passed` 而证据引用空白 → `Failed`；任一未运行/缺失 → `Unmeasured`；七场景全 `Passed` 且各有证据 → `Passed`。
- [x] 6.3 `FallbackRecord{reason, at_sentence_boundary, operator_authorized, logged}`；报告字段 `fallback` 与 `acceptance_claimed_via_fallback`；回退**不进入** `verdict`/`acceptance` 的任何计算。
- [x] 6.4 测试：七场景全过 → 通过；OOM 失败 → 失败；形象切换未跑 → 缺测；声明通过无证据 → 失败；有 `Failed` 时发生回退 → 准入仍 `NotAccepted`；声明以回退构成验收 → 审计报出且准入非 `Accepted`；回退缺授权/不在句间/未记日志 —— 三条分别可构造。

## 7. 审计（B02）

- [x] 7.1 `audit_gate(&GateReport) -> Vec<Finding>` 报出全部命中：`established_right_without_basis`、`undeclared_component`、`precision_without_fp32_baseline`、`int8_without_calibration`、`cuda_ep_registered_without_placement`、`full_gpu_claimed_without_placement_records`、`full_gpu_claimed_with_cpu_fallback`、`scenario_passed_without_evidence`、`gates_evaluated_without_model`、`fallback_claimed_as_acceptance`、`fallback_without_authorization`、`fallback_outside_sentence_boundary`、`fallback_unlogged`、`not_applicable_for_target_capability`。
- [x] 7.2 测试：每条标记可由直接构造的报告命中；干净报告审计为空；Standard 档声明不适用被报出；缺模型但某门 `Passed` 被报出。

## 8. 规格、文档与提交

- [x] 8.1 新增 `docs/realistic-gate.md`（四门与总体派生、缺测三态、G0 五权利与依据非空、G1 精度顺序、G2 目标机/并发/放置口径、G3 七场景、2D 回退正交、缺模型 Blocked、NotApplicable 边界、审计标记、未做边界）。
- [x] 8.2 新增 ADR 0034，记录落进 kernel 的理由与九项决策的否决方案（单分数认证度 / 缺测填 0 / 缺测算通过 / CUDA EP 注册即全 GPU / GPU 复制次数即全 GPU / 回退补足门 / 缺模型标不适用 / 只靠判定不审计 / 臆造证据目录）。
- [x] 8.3 报告 `docs/reports/2026-10-10-w32-realistic-gate-verdict.md`：实测数字、逐项验收映射（B02）、未覆盖边界与环境边界。
- [x] 8.4 `tests/registry.yaml`：B02 **保持 `blocked`**，notes 补记 W32 已交付判定机制与阻塞边界；D12 保持 `blocked` 并在 notes 回指 W32；`docs/development-plan.md` 与 `AGENTS.md` 记录本轮。`check --registry` 通过。
- [x] 8.5 归档 `w32-realistic-gate-verdict`（归档名 `2026-10-10-w32-realistic-gate-verdict`，新 capability `realistic-gate` 生效），跑 `validate --all --strict`（含 `--archived`）。
- [x] 8.6 全量 `cargo xtask check --profile portable` 通过并留档；提交 + 推送。

## 里程碑切分

本轮为单个可独立验证的切片：四门与派生（§2）、G0–G3 各自判据（§3–§6）、审计（§7）共用同一个 `realistic-gate` capability——§7 的审计逐条检查的正是 §2–§6 产出的报告字段，拆开提交会让审计的第一版无法验证「每条规则都直接可构造」。文档、ADR、registry、归档与门控（§8）为纯声明变更，沿用同一次门控留档。

**顺序纪律**：先跑门控 → 再勾任务项 → 最后归档（`validate --all --archived` 会检查归档 tasks 完成度）。
