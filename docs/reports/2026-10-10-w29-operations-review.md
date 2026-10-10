# W29 实测报告：运营度量与场次复盘（B04 / R06 / B06）

- 日期：2026-10-10
- 变更：`openspec/changes/w29-operations-review/`
- 决策：[ADR 0031](../adr/0031-operations-review-boundary.md)；指南：[docs/operations.md](../operations.md)
- 判据：`tests/registry.yaml` B04、R06、B06

## 结论

B04 / R06 / B06 的**真值**需要真实设备、长时运行与真实场次数据，本轮不假装有；但它们的**判据口径**此前没有落点，于是「设备里程碑」被执行成了「什么都不用做」。本轮把三件事的口径落成 `crates/nai-kernel/src/metrics.rs` 与 `review.rs` 里的可执行判据，**零新增依赖**（kernel 仍只依赖 contracts），18 包与依赖方向不变，全部在 portable 下可测。

最关键的一条是 §12.5 那句写死的口径：**「整条路径实测分位数，不把各阶段 P95 相加」**。本轮把「端到端」与「阶段之和」做成两个不同的调用，并让后者只能作为**具名对照**出现，差距由 `stage_sum_gap()` 显式算出。

registry B04 / R06 / B06 由 `pending` 升为 `implemented`，三项均**不标 `tested`**：真实长播、真实设备时延采样与真实场次数据未接。

## 目标行为

| 判据 | 实现 | 关键设计 |
| --- | --- | --- |
| 整链路分位（§12.5） | `LatencySamples::percentile` | 只从**端到端样本**算；阶段分位另算；阶段之和是具名对照 |
| 分位可复现 | `nearest_rank` | 不插值：返回值必是真实样本，报告可与原始数据逐条对照 |
| 小样本不冒充分位 | `MIN_PERCENTILE_SAMPLES = 20` | 低于门槛 `CAPABILITY_NOT_READY`，没有数字可误读 |
| §12.5 目标判定 | `DESIGN_TARGETS` + `evaluate_target` | 样本不足返回 `Err` 而非达标；边界值判 `Meets` |
| 长播窗口是下界（R06） | `SoakWindow` + `assess` | 跨度或样本不足 → `Insufficient` 并如实报出观察值 |
| 判定抗噪 | 双半**中位数** + `max_drift_pct` | 末端尖峰不误判、起始低值不掩盖增长；样本时间必须非降序 |
| 报告含 §5.9 字段（B06） | `SessionReview::build` | 七类字段齐全；延迟分位**复用** B04 的聚合器（一个真相） |
| 报告自洽 | `build` 的计数校验 | 回答多于入选、处理多于入选、拒答未被原因解释 → 拒绝构造 |
| 缺测不是零 | `unmeasured` + `Option` + `CostView::Unknown` | 分母为 0 的比例与未定价成本留空并列入清单 |
| 可构造的自洽审计 | `audit_review`（纯函数） | 十条稳定标记，字段公开使每条规则可在测试里构造 |

## 实测数字

- `cargo test -p nai-kernel --test metrics`：**13 passed / 0 failed**
  - 分位 5：零值被拒、门槛边界（19 拒 / 20 过）、nearest-rank 对齐定义（1..=100 → P50=50 / P95=95 / P99=99）、**端到端忽略阶段样本**（端到端 100 vs 6×40 相加 240）、阶段自身样本门槛与阶段之和不可得。
  - 目标 1：达标 / 恰好等于限值达标 / 超标 / 样本不足被拒。
  - 长播 7：样本乱序被拒、短跑 `Insufficient`、窗口是下界（2h 判 8h 为不足）、平稳 `Stable`、上升 `Growing`、末端尖峰不误判、起始低值不掩盖增长。
- `cargo test -p nai-kernel --test review`：**8 passed / 0 failed**
  - 报告 6：比例保留分子分母与千分比、四类不自洽计数被拒、零互动 → 未测量（非 0）、未知成本 → `Unknown`、已知成本与预算的剩余与超支、守门命中只计非通过（42 = 30 + 12，500 个 pass 不计）。
  - 审计 2：干净报告无违规、十条矛盾各自的稳定标记（含仅报出该类）。
- `cargo clippy -p nai-kernel --all-targets --locked -- -D warnings`：通过（零告警）。

## 逐项验收映射

| 项 | 条款 | 覆盖 |
| --- | --- | --- |
| B04 | 整链路分位数达到 §12.5 设计值 | 端到端分位口径 + 目标判定（`evaluate_target`） |
| B04 | 不把各阶段 P95 相加 | `percentile` 只读端到端样本；`sum_of_stage_percentiles` 仅作对照 + `stage_sum_gap` |
| R06 | 2h / 8h / 24h 无持续资源增长 | `SoakWindow`（下界）+ 双半中位数判定 + `Insufficient` 诚实报出 |
| B06 | 报告可生成 | `SessionReview::build`（构造即自洽） |
| B06 | 含 §5.9 所列字段 | 互动量/入选率、覆盖率、批准·修改·拒答比例与原因、延迟分位、故障与降级、成本与预算、守门命中分类（§5.4 四阶段 × 四结果） |

## 门控

- `cargo xtask check --profile portable`（`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=4`）：**EXIT=0**，77 个测试目标（60 `Running` + 17 `Doc-tests`；较 W28 的 75 多 2，即本轮新增的 `tests/metrics.rs` 与 `tests/review.rs`），0 失败。
- `cargo test -p nai-kernel --test metrics --test review`：**13 + 8 = 21 passed / 0 failed**。
- `cargo clippy -p nai-kernel --all-targets --locked -- -D warnings`：通过（零告警）；`cargo fmt --all` 已跑。
- `cargo xtask check-deps`：18 包，依赖方向不变（`nai-kernel` 仍只依赖 contracts）。
- `cargo xtask check --registry`：通过（B04/R06/B06 证据与状态一致）。
- `openspec validate --all --strict --archived --no-interactive`：**35 passed / 0 failed**；`w29-operations-review --strict` 归档前为 `is valid`。

> 注：本轮宿主 LNK1104 比 W28 更顽固——首轮预链**连续 6 次**落在不同包上（`pipeline` → `nai_engine` → `engine` → `nai_control_api` → `accounting` → `nai_budget_core`）。处置顺序：①单包链接验证环境正常（`cargo test -p nai-budget-core --no-run` 5.22s 链完）→ 确认是瞬时锁而非环境损坏；②改用「重试 + 按报错路径定向清理被锁 exe」的循环预链，第 1 次即 `rc=0`；③完整门控一次通过。属宿主事实，不写进 portable 检查。

## 未覆盖边界

- **真实采样未接**：RSS / 显存 / 时延样本与 `at_ms` 由宿主提供，`metrics.rs` 不读系统指标也不读时钟。
- **真实长播未做**：2h / 8h / 24h 的真值需设备与长时运行；本轮只落地判定规则。
- **真实场次数据未接**：报告从调用方给的计数投影，未接真实决策日志导出。
- **报告呈现与质量评测不在范围**：报告 UI 属 UI 验收（保持 pending）；B01 的 100 题黄金集是独立主线。
- **阈值为常量/参数**：`MIN_PERCENTILE_SAMPLES`、`MIN_SOAK_SAMPLES`、`max_drift_pct` 取自保守约定，未由真实数据校准。
