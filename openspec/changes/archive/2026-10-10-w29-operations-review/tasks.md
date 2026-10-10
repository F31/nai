## 1. 立项与骨架

- [x] 1.1 OpenSpec 变更 `w29-operations-review`（proposal/design/tasks + `operations-review` 增量），`validate --strict` 通过。
- [x] 1.2 落点确认：放进 `crates/nai-kernel`（`metrics.rs` + `review.rs`；kernel 只允许依赖 contracts，`xtask/src/deps.rs`），不放进 shared 层、不新开 crate——18 包与依赖方向不变；复用 `MetricsError{code, reason}`（稳定 token，与 `UpgradeError`/`MultiplexError` 同形）。

## 2. 延迟分位与目标判定（B04）

- [x] 2.1 `Percentile`（`P50|P95|P99`，`ALL` / `rank` / `label`）与 `Stage`（`Ingest|Retrieve|Decision|Guard|Synthesis|FirstAudio`）；nearest-rank 分位（`ceil(p/100·n)`，不插值）。
- [x] 2.2 `LatencySamples`：`record`（端到端，零值 `INVALID_ARGUMENT`）、`record_stage`（阶段，零值同拒）；端到端样本少于 `MIN_PERCENTILE_SAMPLES = 20` 时 `percentile` 以 `CAPABILITY_NOT_READY` 拒绝。
- [x] 2.3 `stage_percentile` 各算各的（阶段样本不足即拒，MUST NOT 由端到端顶替）；`sum_of_stage_percentiles` 作为**具名对照**存在，`stage_sum_gap` 显式给出差距。
- [x] 2.4 `DESIGN_TARGETS`（§12.5 的模板开口 1500ms / API 开口 3000ms / 控制响应 50ms，均 P95）与 `evaluate_target` → `Meets | Exceeds`；样本不足返回 `Err` 而非达标；边界值等于限值判 `Meets`。

## 3. 长播资源增长判定（R06）

- [x] 3.1 `SoakWindow`（`TWO_HOURS` / `EIGHT_HOURS` / `RELEASE_CANDIDATE` + `SOAK_WINDOWS`）与 `SoakPolicy { window, max_drift_pct, min_samples }`（`MIN_SOAK_SAMPLES = 8`，`Default` = 2h/10%）。
- [x] 3.2 `SoakSamples`：`record(at_ms, value)` 要求时间非降序（否则 `INVALID_ARGUMENT`，杜绝靠重排制造趋势）、`span_ms`、`half_values`（按时间中点分半后各自排序）。
- [x] 3.3 `assess`：样本不足或跨度小于窗口 → `Insufficient { span_ms, samples }`（**如实报出**观察值，MUST NOT 报 `Stable`）；否则比较两半的**中位数**，漂移超过阈值 → `Growing`，否则 `Stable`；`is_conclusive` 区分有无结论。
- [x] 3.4 测试 `tests/metrics.rs`：零值被拒、门槛边界、nearest-rank 对齐定义、**端到端忽略阶段样本**（100 vs 6×40）、阶段自身样本门槛与阶段之和不可得、目标达标/恰好达标/超标/未测量、样本乱序被拒、短跑 `Insufficient`、平稳 `Stable`、上升 `Growing`、末端尖峰不误判、起始低值不掩盖增长、窗口是下界（2h 判 8h 为 `Insufficient`）。

## 4. 场次复盘报告（B06）

- [x] 4.1 `GuardStage`（4）/`GuardVerdict`（4，`is_hit` 定义非通过即命中）对齐 §5.4；`SessionEntries`（互动/入选/回答/批准/修改/拒答 + 原因 + 守门 + 故障/降级 + 可选成本与预算）。
- [x] 4.2 `SessionReview::build` 自洽校验：入选≤互动、回答≤入选、处理之和≤入选、拒答必须被原因完全解释（否则 `INVALID_ARGUMENT`）。
- [x] 4.3 `Ratio { numerator, denominator }`（`permille` 在分母为零时返回 `None`，`exceeds_unity`）；分母为零的比例为 `None` 并进 `unmeasured`（**不写 0**）。
- [x] 4.4 延迟分位复用 §2 的聚合器（一个真相）：样本不足则该字段留空并进 `unmeasured`；`stage_sum_p95` 作为对照另列。`CostView::Known{cost,budget,remaining,over_budget} | Unknown`（未知成本不写 ¥0）。
- [x] 4.5 `audit_review`：对上报报告值的纯函数，检出 `ratio_without_denominator`、`ratio_exceeds_unity`、`field_measured_but_marked_unmeasured`、`coverage_exceeds_admission`、`rejections_without_reasons`、`reasons_do_not_match_rejections`、`unknown_cost_reported_as_zero`、`unknown_cost_not_declared`、`partial_latency_not_declared`、`percentiles_out_of_order`，报出**全部**。
- [x] 4.6 测试 `tests/review.rs`：正常报告的比例与千分比、四类不自洽计数被拒、零互动→未测量（非 0）、延迟样本不足→未测量、未知成本→`Unknown`、已知成本与预算的剩余与超支、守门命中只计非通过、审计干净报告与每条矛盾的稳定标记。

## 5. 规格、文档与提交

- [x] 5.1 新增 `docs/operations.md`（端到端分位口径与阶段对照、样本门槛、§12.5 目标判定、长播窗口与双半中位数判定、报告投影与自洽校验、未测量与未知成本、守门分类、审计标记、未做边界）。
- [x] 5.2 新增 ADR 0031，记录落进 kernel 的理由与七项决策的否决方案（插值分位 / 只给端到端 / 低置信标注 / 首尾两点或回归斜率 / 名义窗口标签 / 浮点百分数 / 只靠 `build` 不做审计）。
- [x] 5.3 报告 `docs/reports/2026-10-10-w29-operations-review.md`：实测数字、逐项验收映射（B04/R06/B06）、未覆盖边界与环境边界。
- [x] 5.4 `tests/registry.yaml`：B04、R06、B06 补证据并按实现 `pending → implemented`（三项均**不标 `tested`**——真实长播、真实设备时延与真实场次数据未接）；`docs/development-plan.md` 与 `AGENTS.md` 记录本轮。`check --registry` 通过。
- [x] 5.5 归档 `w29-operations-review`（归档名 `2026-10-10-w29-operations-review`，新 capability `operations-review` 生效），跑 `validate --all --strict`（含 `--archived`）。
- [x] 5.6 全量 `cargo xtask check --profile portable` 通过并留档；提交 + 推送。

## 里程碑切分

本轮为单个可独立验证的切片：分位与目标判定（§2）、长播判定（§3）、报告投影（§4）共用同一个 `operations-review` capability，且 §4 直接复用 §2 的聚合器——拆开提交会让报告的第一版无法验证「延迟分位来自同一真相」。文档、ADR、registry、归档与门控（§5）为纯声明变更，沿用同一次门控留档。
