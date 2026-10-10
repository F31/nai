## Why

`tests/registry.yaml` 里 B01 的 notes 写着「100 题黄金集未建（阶段 2）」，是这一轮之前剩余四项 `pending` 里的第一项。§6 与 §5.9 对它写得很具体：

| 出处 | 原文口径 |
| --- | --- |
| §6 尾 | 至少 100 个贴近真实直播的问题；有证据问题 Recall@5 ≥90%；高风险事实不允许自动播报错误；无答案问题正确澄清或转人工；热索引检索 P95 ≤300ms。**检索延迟、证据正确率、最终答案正确率分别报告**。 |
| §5.9 | 黄金集**随场景版本**，覆盖正常回答、事实更新、注入、冲突、越权、无答案、取消、工具返回攻击。修改提示词、人设、规则、模型或知识后，**必须通过相关回归才生效**。被改写或驳回的候选进入坏案例池，**经人工确认后**扩充黄金集。 |

这些句子的**真值**（100 道题的内容、真实模型在这些题上的表现）确实需要外部条件——本轮不假装有。但其中至少有四件事是**判据口径**，而口径不落成代码就会在下面几个地方悄悄含糊过去：

- **「Recall@5 ≥90%」没说分母是什么。** 是「题级命中数 / 有证据题数」，还是「命中文档数 / 应命中文档数」？两个口径在同一个 run 上可以差出十几个百分点。更糟的是，若没有任何有证据用例，`0/0` 会被程序算成 0% 或 100%——**「没测过」与「测到 0」是两回事**（同 W26 的 `Unmeasured` 容量、W15 的 `Unknown` 成本、W29 的 `unmeasured` 字段）。
- **「检索延迟、证据正确率、最终答案正确率分别报告」是一句禁止令。** 它禁止把三者压成一个「质量分」。一个分数会把「检索坏了但答案侥幸对」与「检索好了但生成坏了」变成同一件事，而两者的修法完全不同。
- **「必须通过相关回归才生效」是一道门禁，不是一次比较。** 平均分不降而某一题由通过变失败，是最典型的**静默退化**；只在平均值上判定的门禁会把这种退化放行。同样，基线本身没达标时，用「不比基线差」放行候选也是错的。
- **「经人工确认后扩充」是一条授权边界。** 若坏案例能直接写进黄金集，黄金集就变成了「系统自己给自己出题」，回归门禁随之失去意义。

## What Changes

- **黄金集是有版本、有覆盖、有规模的对象（`GoldenSet`，B01）**：`scenario_version` 绑定场景（§5.9）；`Category` 八枚举即 §5.9 列举的八类情形；`shortfall()` 与 `audit_coverage()` 把「不足 100 题」「某类未覆盖」报成**具名条目**而不是静默通过。用例的 `Expectation` 与 `Category` 必须相容——`NoAnswer` 配一条证据期望是用例本身坏了，运行前就该报出来（`expectation_category_mismatch`），而不是当成系统失败计分。

- **Recall@5 是题级判定，文档级仅作对照（`evaluate`，B01）**：一题命中当且仅当它要求的**全部**证据都在前 `RECALL_K = 5` 条里；集级 `evidence_recall_at_5` 的分母是**有证据题数**。`evidence_doc_recall_at_5` 以文档粒度同时给出，两个口径**分开呈现**，谁也不能替代谁。有证据题数为零 → `Ratio.permille()` 为 `None` + `FIELD_EVIDENCE_RECALL` 进 `unmeasured`。

- **高风险事实、无答案、越权/注入/工具返回攻击各自有判据（B01）**：`Expectation::HighRiskFact { forbidden_claims }` 只对**自动播报**生效——澄清/转人工/拒答都是正确行为，播报了但没播被禁断言也不算违规。`MustClarifyOrEscalate` 把「自动播报」与「拒答但没澄清」分开报（两个稳定 token）。`MustNotAct` 逐条列出触发的副作用。这些都是**一票否决**项，不是比例项——§6 说的是「高风险事实不允许自动播报错误」，不是一个百分比。

- **检索 P95 是门槛且可未测量（B01）**：复用 W29 的 `LatencySamples::percentile`（同一个 nearest-rank 实现，一个真相），<20 样本 → `None` + `FIELD_RETRIEVAL_P95` 进 `unmeasured` + 门槛项 `retrieval_p95_unmeasured`。恰好等于 300ms 判达标。**未测量计为未达标**。

- **三项分别报告，不合成（`QualityReport`，B01）**：`evidence_recall_at_5`、`answer_accuracy`、`retrieval_p95_ms` 是三个独立字段，没有任何方法把它们合并。`AnswerVerdict::Unknown`（未判定）从正确率分母中**排除**而不是计为错误。

- **回归门禁拒绝四种放行（`regression`，B01）**：版本不一致 → `VERSION_INCOMPATIBLE`（拒绝比较）；候选任一门槛失败 → `candidate_gate_failed`；逐题由通过变失败 → `case_regressed`（**不因平均分未降而放行**）；证据召回或答案正确率低于基线 → `recall_regressed` / `accuracy_regressed`。

- **提升是一次人工授权（`BadCasePool::promote`，B01）**：候选入池开放、出池必须具名确认；空确认人 → `PERMISSION_DENIED`；提升后的 `CaseOrigin::Promoted { confirmer }` 随用例保存，**未被确认的候选无法伪装成 authored**。

- **自洽审计对上报值做纯函数（`audit_report`，B01）**：`cases_run_exceeds_total`、`recall_exceeds_unity`、`accuracy_exceeds_unity`、`unmeasured_field_reported`、`measured_field_unmarked`。报告字段公开，每条规则都能在测试里直接构造，不会被 `evaluate` 的保证架空。

## Capabilities
### New Capabilities
- `knowledge-quality-gate`：黄金集的版本/覆盖/规模约束、题级 Recall@5 与文档级对照、高风险事实与无答案题的否定式判据、越权/注入/工具返回攻击的副作用判据、检索 P95 门槛与未测量表达、三项分别报告、回归门禁、坏案例池的人工确认提升、报告自洽审计。覆盖 registry **B01**。

### Modified Capabilities
- 无。既有 `knowledge-pipeline`（中文分词、切块、词法/向量候选、远端知识契约）**不改**：它管的是**怎么检索**，本变更管的是**怎么判定检索质量**。本变更只**消费**「一次运行产出的证据序列与行为结果」，不改变检索管线本身。既有 `operations-review`（W29）**不改**：场次复盘是**在线**报告，黄金集回归是**离线**门禁，二者共用分位实现但语义不同。

## Impact

- 代码：`crates/nai-kernel/src/` 新增 `golden.rs`；`tests/` 新增 `golden.rs`；`lib.rs` 增一个 `pub mod`。**不新增 crate，不改 workspace 依赖**（维持 18 包与依赖方向；kernel 仍只依赖 contracts）。
- 规格：新增 capability `knowledge-quality-gate`；新增 `docs/golden-set.md`、ADR 0032、报告 `docs/reports/2026-10-10-w30-knowledge-quality-gate.md`。
- 依赖：**零新增**。仅用 `nai_contracts::v1::ErrorCode` 与 `std`；分位复用同在 kernel 的 `metrics`，比例复用同在 kernel 的 `review::Ratio`（不复制实现）。
- registry：B01 补证据并按实现 `pending → implemented`（**不标 `tested`**：真实题集内容、真实模型评估与真实检索质量未接）。
- 明确不做：不做题目的**内容**（100 道真实问题由场景包提供）；不接真实模型评估（答案正确性由调用方判定后注入）；不做评估 UI；不改检索管线。

## Non-Goals

- 不用机制替代真实评估：本轮证明的是「口径可执行、缺测不被读成通过、退化不被平均值掩盖」，不宣称任何真实场景包已达到 §6 门槛。
- 不给「质量分」：三个正确率各自可见，不提供任何加权求和或总分字段。
- 不让未测量变成零：缺测字段一律进 `unmeasured` 并留空，且未测量的门槛**计为未达标**。
- 不让坏案例自行入集：候选入池开放、出池必须具名人工确认。
