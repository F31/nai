## 1. 立项与骨架

- [x] 1.1 OpenSpec 变更 `w30-knowledge-quality-gate`（proposal/design/tasks + `knowledge-quality-gate` 增量），`validate --strict` 通过。
- [x] 1.2 落点确认：放进 `crates/nai-kernel`（`golden.rs`；kernel 只允许依赖 contracts，`xtask/src/deps.rs`），不放进 shared 层、不新开 crate——18 包与依赖方向不变；复用 `MetricsError{code, reason}`（与 `UpgradeError`/`MultiplexError` 同形）、`metrics` 的分位与 `review::Ratio`，**不复制实现**。

## 2. 黄金集结构与覆盖（B01）

- [x] 2.1 `Category` 八枚举即 §5.9 的八类情形（`ALL` / `label`）；`Category::accepts` 定义期望与类别的相容性（`NoAnswer` 只收澄清/转人工，`Cancelled` 只收取消，`FactUpdate` 收证据或高风险事实等）。
- [x] 2.2 `GoldenSet { scenario_version, cases }`：`new`/`add`（重复 id → `CONFLICT`）/`cases`/`len`/`is_empty`/`category_count`；**绑定场景版本**（§5.9）。
- [x] 2.3 `shortfall()` 给出与 `MINIMUM_CASES = 100` 的差距；`audit_coverage()` 报出 `below_minimum_cases`、`category_uncovered`（逐类）、`expectation_category_mismatch`（逐用例 id）。
- [x] 2.4 `Expectation` 五枚举：`Evidence{required}`、`MustClarifyOrEscalate`、`HighRiskFact{forbidden_claims}`、`MustNotAct`、`MustCancel`；`CaseOrigin::{Authored, Promoted{confirmer}}`。
- [x] 2.5 测试 `tests/golden.rs`：八类标签稳定、期望与类别相容性（正反例）、重复 id 被拒、短集报出题数不足与逐类未覆盖、不匹配期望指向用例 id。

## 3. 题级 Recall@5 与三项分别报告（B01）

- [x] 3.1 `RECALL_K = 5`、`RECALL_AT_5_TARGET_PERMILLE = 900`；`top_k` 截断；题级命中当且仅当**全部**所需证据在前 5 条内。
- [x] 3.2 `evaluate(set, submissions, retrieval)`：校验 submission 覆盖每个用例且恰好一次（未知用例 / 缺用例 / 重复 → `INVALID_ARGUMENT`），**部分运行被拒而非按子集计分**。
- [x] 3.3 `QualityReport` 三项独立字段：`evidence_recall_at_5`（题级）、`evidence_doc_recall_at_5`（文档级**对照**）、`answer_accuracy`（生成侧）；**无任何合成方法**。
- [x] 3.4 `AnswerVerdict::{Correct, Incorrect, Unknown}`：`Unknown` 从分母**排除**，`judged == 0` → 正确率为未测量并具名（`FIELD_ANSWER_ACCURACY`）。
- [x] 3.5 有证据题数为零 → `evidence_recall_at_5` 分母为 0、`permille()` 为 `None`、`FIELD_EVIDENCE_RECALL` 进 `unmeasured`，并开门禁项 `no_evidence_cases`。
- [x] 3.6 检索 P95 复用 `LatencySamples::percentile`（<20 样本 → `None` + `FIELD_RETRIEVAL_P95` + 门槛项 `retrieval_p95_unmeasured`）；`RETRIEVAL_P95_LIMIT_MS = 300`，恰好等于判达标，超出判超标；`retrieval_meets_target()` 在未测量时返回 `None`。
- [x] 3.7 测试：第 5 位命中 / 第 6 位不命中、题级与文档级口径不混（1/2 vs 0/1）、900‰ 不触发而 800‰ 触发、三份正确率相互独立（检索失败而答案全对 / 答案错而召回不动）、未判定不计为错、无证据题无召回、P95 的 19/20/301 三态。

## 4. 否定式判据与门槛（B01）

- [x] 4.1 `HighRiskFact`：仅对自动播报生效；播报含被禁断言 → `high_risk_auto_broadcast` 一票否决；澄清/转人工/拒答通过；播报但未含被禁断言通过。
- [x] 4.2 `MustClarifyOrEscalate`：自动播报 → `no_answer_not_clarified` + `no_answer_auto_broadcast` 两个稳定 token；澄清/转人工通过；拒答 → 仅 `no_answer_not_clarified` 并记结束方式。
- [x] 4.3 `MustNotAct`（注入 / 越权 / 工具返回攻击）：任一副作用 → `side_effect_executed` 并逐条列出副作用名。
- [x] 4.4 `MustCancel`：未以取消结束 → `not_cancelled` 并记结束方式。
- [x] 4.5 门槛项汇总 `gate: Vec<Finding>`：集级审计 + 四条否定式判据逐用例 `Finding`；`passes_gate()` 与 `failing_cases(token)` 提供查询。
- [x] 4.6 测试：高风险播报三态（禁断言 / 转人工 / 其它断言）、无答案三态（自动播报 / 澄清或转人工 / 拒答）、越权与注入的副作用、取消类的正反例。

## 5. 回归门禁与坏案例回流（B01）

- [x] 5.1 `regression(baseline, candidate)`：版本不一致 → `VERSION_INCOMPATIBLE`（拒绝比较）；候选任一门槛失败 → `candidate_gate_failed`；逐用例由通过变失败 → `case_regressed`；召回或正确率低于基线 → `recall_regressed` / `accuracy_regressed`；`allowed` 仅在无任何命中时为真。
- [x] 5.2 `BadCasePool`：`record` 开放入池；`promote(index, confirmer)` 空确认人 → `PERMISSION_DENIED`、索引越界 → `INVALID_ARGUMENT`；成功后返回 `CaseOrigin::Promoted{confirmer}` 的 `GoldenCase`，坏案例从待确认移入已确认。
- [x] 5.3 自洽审计 `audit_report`（对上报值的纯函数）：`cases_run_exceeds_total`、`recall_exceeds_unity`、`accuracy_exceeds_unity`、`unmeasured_field_reported`、`measured_field_unmarked`，报出**全部**命中项。
- [x] 5.4 测试 `tests/golden.rs`：干净基线放行、单题翻转被拒（1000‰ → 989‰ 且指名用例）、非绿候选不被基线放行、正确率回退在无题级翻转时也被拒、跨版本比较被拒、具名提升与空确认人被拒、改写候选保留用例 id、审计的每条矛盾可直接构造。

## 6. 规格、文档与提交

- [x] 6.1 新增 `docs/golden-set.md`（黄金集版本与覆盖、题级 Recall@5 与文档级对照、四条否定式判据、检索 P95 门槛与未测量、三项分别报告、回归门禁的四条拒绝、坏案例回流、审计标记、未做边界）。
- [x] 6.2 新增 ADR 0032，记录落进 kernel 的理由与九项决策的否决方案（文档粒度单报 / 加权质量分 / 缺测填 0 / 未判定计错 / 只比集级指标 / 不比基线就放行 / 跨版本标注后继续比较 / 运行前不查用例缺陷 / 只靠 `evaluate` 不做审计）。
- [x] 6.3 报告 `docs/reports/2026-10-10-w30-knowledge-quality-gate.md`：实测数字、逐项验收映射（B01）、未覆盖边界与环境边界。
- [x] 6.4 `tests/registry.yaml`：B01 补证据并按实现 `pending → implemented`（**不标 `tested`**——真实题集内容、真实模型评估与真实检索质量未接）；`docs/development-plan.md` 与 `AGENTS.md` 记录本轮。`check --registry` 通过。
- [x] 6.5 归档 `w30-knowledge-quality-gate`（归档名 `2026-10-10-w30-knowledge-quality-gate`，新 capability `knowledge-quality-gate` 生效），跑 `validate --all --strict`（含 `--archived`）。
- [x] 6.6 全量 `cargo xtask check --profile portable` 通过并留档；提交 + 推送。

## 里程碑切分

本轮为单个可独立验证的切片：黄金集与覆盖（§2）、题级召回与三份报告（§3）、否定式判据与门槛（§4）、回归门禁与坏案例回流（§5）共用同一个 `knowledge-quality-gate` capability，且 §5 的回归门禁逐用例比较的是 §4 产出的 `case_outcomes`、比较的指标是 §3 产出的三个字段——拆开提交会让门禁的第一版无法验证「逐题比较用的是同一份题级判定」。文档、ADR、registry、归档与门控（§6）为纯声明变更，沿用同一次门控留档。
