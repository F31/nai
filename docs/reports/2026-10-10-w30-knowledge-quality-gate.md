# W30 实测报告：黄金集知识质量门槛与回归门禁（B01）

- 日期：2026-10-10
- 变更：`openspec/changes/w30-knowledge-quality-gate/`
- 决策：[ADR 0032](../adr/0032-knowledge-quality-gate-boundary.md)；指南：[docs/golden-set.md](../golden-set.md)
- 判据：`tests/registry.yaml` B01

## 结论

B01 的**真值**（100 道贴近真实直播的问题、真实模型在这些题上的表现、真实检索质量）需要外部场景包与真实模型评估，本轮不假装有。但它的**判据口径**此前没有落点，于是「100 题黄金集未建（阶段 2）」被执行成了「什么都不用做」。本轮把 §6 的五条门槛与 §5.9 的三条生命周期语句落成 `crates/nai-kernel/src/golden.rs` 里的可执行判据，**零新增依赖**（kernel 仍只依赖 contracts），18 包与依赖方向不变，全部在 portable 下可测。

最关键的三条是：

1. §6 的「Recall@5 ≥90%」此前没有分母：本轮把它钉成**题级**判定（全部所需证据进前 5 才算命中），文档粒度召回作为**对照**同时报告，两个口径分开呈现。
2. §6 的「检索延迟、证据正确率、最终答案正确率分别报告」是一句禁止令：本轮把它落成三个**独立字段**，没有任何合成方法。
3. §5.9 的「必须通过相关回归才生效」是一道门禁：本轮把它落成**逐题**比较，最小退化形态（平均分不降而单题翻转）也会被拒。

registry B01 由 `pending` 升为 `implemented`，**不标 `tested`**：真实题集内容、真实模型评估与真实检索质量未接。

## 目标行为

| 判据 | 实现 | 关键设计 |
| --- | --- | --- |
| 黄金集随场景版本（§5.9） | `GoldenSet::scenario_version` | 回归门禁在版本不一致时直接拒绝比较 |
| 覆盖八类情形（§5.9） | `Category::ALL`（八枚举） | `audit_coverage` 逐类报未覆盖 |
| 至少 100 题（§6） | `MINIMUM_CASES = 100`、`shortfall` | `below_minimum_cases` 具名报出 |
| 用例配置缺陷与系统失败分开 | `Category::accepts` | `expectation_category_mismatch` 在运行前报出，指向用例 id |
| 有证据问题 Recall@5 ≥90%（§6） | `evidence_recall_at_5` + `RECALL_AT_5_TARGET_PERMILLE = 900` | 题级判定；文档级仅作对照 |
| 高风险事实不得自动播报错误（§6） | `Expectation::HighRiskFact` | 只对自动播报生效；澄清/转人工/拒答都是正确行为 |
| 无答案必须澄清或转人工（§6） | `Expectation::MustClarifyOrEscalate` | 自动播报与「拒答但没澄清」分开报 |
| 越权/注入/工具返回攻击不得有副作用 | `Expectation::MustNotAct` | 副作用逐条列出 |
| 热索引检索 P95 ≤300ms（§6） | `RETRIEVAL_P95_LIMIT_MS = 300` + `LatencySamples` | 复用 W29 分位；恰好等于判达标；未测量计为未达标 |
| 三份正确率分别报告（§6） | `QualityReport` 三个独立字段 | 无任何合成方法；`Unknown` 从分母排除 |
| 改动必须通过回归才生效（§5.9） | `regression` | 四条拒绝：版本不一致 / 候选门槛失败 / 逐题回退 / 指标回退 |
| 坏案例经人工确认才可入集（§5.9） | `BadCasePool::promote` | 具名确认人；`CaseOrigin::Promoted` 保存确认人 |

## 测试

`cargo test -p nai-kernel --test golden`：**24 passed / 0 failed**。

| 组 | 覆盖 |
| --- | --- |
| 结构（4） | 八类标签稳定；期望与类别相容性正反例；重复 id → `CONFLICT`；短集报题数不足 + 逐类未覆盖 + 不匹配期望指向用例 id |
| 召回（4） | 全部所需证据进前 5 才算命中（题级 0/1 vs 文档级 1/2）；第 5 位命中 / 第 6 位不命中；900‰ 不触发而 800‰ 触发；无证据题 → 未测量 + `no_evidence_cases` |
| 三份正确率（3） | 检索失败而答案全对；答案错而召回不动；未判定从分母排除并在全未判定时为未测量 |
| 否定式判据（4） | 高风险播报三态（含禁断言 / 转人工 / 其它断言）；无答案三态（自动播报 / 澄清或转人工 / 拒答）；越权与注入的副作用；取消类正反例 |
| 检索门槛（1） | 19 样本 → 未测量且开门禁；20 样本 300ms → 达标；301ms → 超标 |
| 完整性（1） | 缺用例 / 未知用例 / 重复提交 → `INVALID_ARGUMENT`（部分运行被拒而非按子集计分） |
| 门禁（5） | 干净基线放行；单题翻转被拒（1000‰ → 989‰ 且指名 `normal-0`）；非绿候选不被基线放行；正确率回退在无题级翻转时也被拒；跨版本 → `VERSION_INCOMPATIBLE` |
| 回流（2） | 空确认人 → `PERMISSION_DENIED`、越界 → `INVALID_ARGUMENT`、具名提升记录确认人；改写候选保留用例 id |
| 审计（1） | 干净报告无命中；`cases_run_exceeds_total`、`recall_exceeds_unity`、`unmeasured_field_reported`、`measured_field_unmarked` 四条各自可直接构造 |

## 门控

- `cargo xtask check --profile portable`（`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=4`）：**EXIT=0**，78 个测试目标（61 `Running` + 17 `Doc-tests`；较 W29 的 77 多 1，即本轮新增的 `tests/golden.rs`），0 失败，无 `LNK1104`。
- `cargo test -p nai-kernel --test golden`：**24 passed / 0 failed**。
- `cargo clippy -p nai-kernel --all-targets --locked -- -D warnings`：通过（零告警）；`cargo fmt --all` 已跑。
- `cargo xtask check-deps`：18 包，依赖方向不变（`nai-kernel` 仍只依赖 contracts；本模块复用 W29 的 `LatencySamples`/`Percentile`/`Ratio`，未重复分位实现）。
- `cargo xtask check --registry`：通过（B01 证据与状态一致）。
- `openspec validate --all --strict --archived --no-interactive`：**36 passed / 0 failed**；`w30-knowledge-quality-gate --strict` 归档前为 `is valid`。

> 注：本轮宿主 `LNK1104` 出现在预链阶段（逐次落在不同包上），处置仍为「重试 + 按报错路径定向清理被锁 exe」的循环预链，第 8 次收敛后完整门控一次通过（`LNK=0`）。属宿主事实，不写进 portable 检查。

## 未覆盖边界

- 100 道真实问题、真实模型回答、真实检索结果与真实检索时延**均未接**：测试里的 100 题集合是构造出来的，只为证明覆盖审计与门槛判定可执行。
- 答案正确性由调用方判定后注入（`AnswerVerdict`），本轮不做自动判分。
- 不做评估 UI；不改 `knowledge-pipeline`（它管检索，本模块管判定）。
- 检索时延采样由宿主提供，本轮不接真实探针。

## 环境边界

- portable（Windows 上的 MSVC 构建）下全量可测；不打开真实设备、不连真实 Provider。
- registry B01 记为 `implemented` 而非 `tested`，理由同上。
