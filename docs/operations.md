# 运营度量与场次复盘（B04 / R06 / B06）

- 代码：`crates/nai-kernel/src/metrics.rs`（延迟分位与长播判定）、`crates/nai-kernel/src/review.rs`（场次复盘报告）
- 测试：`crates/nai-kernel/tests/metrics.rs`、`crates/nai-kernel/tests/review.rs`
- 决策：[ADR 0031](adr/0031-operations-review-boundary.md)；判据：`tests/registry.yaml` B04 / R06 / B06（V3.11 §12.5、§5.9）

本模块给三件运营事项定**口径**：整链路延迟怎么算分位、长播怎么判「无持续增长」、场次复盘报告长什么样。
它不采任何真实指标、不跑长播——样本由宿主提供，真值需要设备与长时运行。

## 端到端分位与阶段对照（B04）

§12.5 有一句写得最死：**「整条路径实测分位数，不把各阶段 P95 相加」**。原因是概率性的：各阶段的分位不在同一次请求上发生，
相加得到的数字没有对应任何一个真实请求，而且系统性**高估**。所以：

- `LatencySamples::percentile(p)` 只从**端到端样本**算。
- `stage_percentile(stage, p)` 各算各的，用于定位时间花在哪。
- `sum_of_stage_percentiles(p)` **存在**，但它是**具名对照**；`stage_sum_gap(p)` 把差距显式算出来（`端到端 − 阶段之和`），
  让报告能展示差距，而不是把两者混为一谈。

分位定义是 **nearest-rank**：`ceil(p/100·n)` 取排序后该位置的样本，**不插值**。插值会返回一个报告里不存在的值，复核时无法与任何一条样本对上；nearest-rank 返回的永远是真实样本。

样本门槛 `MIN_PERCENTILE_SAMPLES = 20`：低于门槛 `percentile()` 以 `CAPABILITY_NOT_READY` 拒绝。**十份样本里的 P95 实际上就是最大值**，但它读起来像「P95 达标」——没有数字可误读比有一个会被误读的数字好。

## §12.5 目标判定

`DESIGN_TARGETS` 收录按 P95 判定的三项：模板任务开口 1500ms、普通 API 任务开口 3000ms、负载下控制响应 50ms。
`evaluate_target` 返回 `Meets | Exceeds`；**样本不足返回 `Err`，不是 `Meets`**（「没测」不能读成「达标」）。恰好等于限值判 `Meets`。

## 长播资源增长（R06）

`SoakWindow` 是**下界**：2h / 8h / 24h（`TWO_HOURS` / `EIGHT_HOURS` / `RELEASE_CANDIDATE`）。跑 40 分钟的东西不是「2h 测试」。

`SoakPolicy { window, max_drift_pct, min_samples }`（默认 2h / 10% / `MIN_SOAK_SAMPLES = 8`）。判定规则：

1. 样本数低于 `min_samples`，或跨度小于窗口 → `Insufficient { span_ms, samples }`，**如实报出**观察到的跨度与样本数，绝不报 `Stable`。
2. 否则按**时间中点**把样本分成两半，各自排序取**中位数**。
3. `drift_pct = (后半中位数 − 前半中位数) × 100 / 前半中位数`；超过 `max_drift_pct` → `Growing`，否则 `Stable`。

**为什么用中位数而不是首尾两点**：末端一个 300 的尖峰会把首尾比较判成增长；起始一个低值会掩盖真实的上升。中位数对单点离群免疫，且比回归斜率更容易解释「多少算大」。样本时间必须非降序——否则「趋势」可以靠重排制造。

`is_conclusive()` 区分有无结论，让报告能把「不足以判定」与「稳定」分开印。

## 场次复盘报告（B06）

`SessionReview::build(entries, latency)` 从计数投影，字段覆盖 §5.9 末条：

| §5.9 字段 | 报告字段 |
| --- | --- |
| 互动量与入选率 | `interactions`、`admitted_ratio` |
| 回答覆盖率 | `coverage_ratio` |
| 批准/修改/拒答比例与原因 | `approval_ratio`、`modification_ratio`、`rejection_ratio`、`reject_reasons` |
| 延迟分位 | `latency`（复用 B04 的聚合器）、`stage_sum_p95`（对照） |
| 故障与降级事件 | `faults`、`degradations` |
| 成本与预算 | `cost`（`Known` / `Unknown`） |
| 守门命中分类 | `guard_hits`（§5.4 四阶段 × 四结果）、`guard_hit_total()` |

**构造即自洽**：`build` 拒绝不能同时成立的计数——入选多于互动、回答多于入选、批准/修改/拒答之和多于入选、拒答条数未被原因完全解释。

**比例保留分子分母**：`Ratio { numerator, denominator }` 加 `permille()`，报告能被复核，而不是只给一个圆整过的百分数。

**未测量不是零**：分母为零的比例是 `None` 并列入 `unmeasured`；延迟样本不足则该分位留空并列入清单；未定价成本是 `CostView::Unknown`，不是 ¥0。

## 报告自洽审计

`audit_review(&SessionReview)` 是对**上报报告值**的纯函数（字段公开），报出全部违规：

| 标记 | 含义 |
| --- | --- |
| `ratio_without_denominator` | 比例的分母为零 |
| `ratio_exceeds_unity` | 分子大于分母 |
| `field_measured_but_marked_unmeasured` | 既给了值又标称未测量 |
| `coverage_exceeds_admission` | 回答数超过入选数 |
| `rejections_without_reasons` | 有拒答但没有原因 |
| `reasons_do_not_match_rejections` | 原因条数与拒答条数不符 |
| `unknown_cost_reported_as_zero` | 未知成本被写成 ¥0 |
| `unknown_cost_not_declared` | 未定价成本未标注 |
| `partial_latency_not_declared` | 分位缺失但未标注 |
| `percentiles_out_of_order` | 分位次序颠倒（P50 > P99） |

`SessionReview::build` 已挡住大部分矛盾；审计是**最后一道防线**，因为字段公开后仍可被直接构造。让审计保持可构造，这些规则才是活的。

## 未做边界

- **不采真实指标**：RSS / 显存 / 时延样本由宿主提供，`at_ms` 也由调用方给，故结果可复现但**不是**真实测量。
- **不做长播**：2h / 8h / 24h 得真跑；本轮只把判定规则落地。
- **不做报告 UI**：呈现属 O02/O05 一类的 UI 验收，保持 pending。
- **不做模型质量评测**：B01 的 100 题黄金集是独立主线。
- **registry 状态**：B04 / R06 / B06 升 `implemented`，均**不标 `tested`**。
