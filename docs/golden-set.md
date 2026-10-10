# 黄金集知识质量门槛与回归门禁（B01）

代码：`crates/nai-kernel/src/golden.rs`；测试：`crates/nai-kernel/tests/golden.rs`；
决策：[ADR 0032](adr/0032-knowledge-quality-gate-boundary.md)。

本模块把 §6 的质量门槛（设计值）与 §5.9 的黄金集生命周期落成可执行判据。它**不产生**真实题集、真实模型评估或真实检索结果——那些由外部场景包与评估流程提供；它负责的是「给定一次运行的输入与结果，判定口径是什么、门槛何时打开、退化如何被拦住」。

## 黄金集的版本、覆盖与规模

`GoldenSet { scenario_version, cases }` 绑定场景版本（§5.9「黄金集随场景版本」）。`Category` 的八个枚举正是 §5.9 列举的八类情形：

| 枚举 | §5.9 原文 |
| --- | --- |
| `NormalAnswer` | 正常回答 |
| `FactUpdate` | 事实更新 |
| `Injection` | 注入 |
| `Conflict` | 冲突 |
| `PrivilegeEscalation` | 越权 |
| `NoAnswer` | 无答案 |
| `Cancelled` | 取消 |
| `ToolReturnAttack` | 工具返回攻击 |

`audit_coverage()` 在**运行之前**报出三类结构问题：

- `below_minimum_cases`：题数少于 `MINIMUM_CASES = 100`（§6 设计值），`shortfall()` 给出差距；
- `category_uncovered`：逐类报出未覆盖的类别；
- `expectation_category_mismatch`：用例的期望与其类别不相容（例如「无答案」配一条证据期望）——这是**用例配置缺陷**，指向用例 id，不计为系统失败。

## 题级 Recall@5 与文档级对照

一题命中当且仅当它要求的**全部**证据都出现在前 `RECALL_K = 5` 条检索结果中。集级 `evidence_recall_at_5` 的分母是**有证据题数**，目标为 `RECALL_AT_5_TARGET_PERMILLE = 900`（千分比，即 90%）。

`evidence_doc_recall_at_5` 以文档粒度同时给出，作为**对照**：

```
一题要求 [a, b]，前 5 条命中 [a]
  题级：0 / 1    ← 「整题失败」
  文档级：1 / 2  ← 「命中一半」
```

两个数字**分开呈现**，任何一方都不能替代另一方。文档粒度对「一题 5 个证据」与「一题 1 个证据」等权，会掩盖整题失败；题级口径则看不到「命中了一半」这种部分进展。两者都报，口径才不含糊。

有证据题数为零时，`Ratio.permille()` 返回 `None`，`FIELD_EVIDENCE_RECALL` 进 `unmeasured`，并开门禁项 `no_evidence_cases`——**没测过不等于 0%**。

## 四条否定式判据

§6 与 §5.9 的这几句都是否定式的，本模块把它们落成结构化期望：

| 期望 | 触发条件 | 违规 token |
| --- | --- | --- |
| `HighRiskFact { forbidden_claims }` | **自动播报**且断言含被禁内容 | `high_risk_auto_broadcast` |
| `MustClarifyOrEscalate` | 未以澄清或转人工结束 | `no_answer_not_clarified`；自动播报时另加 `no_answer_auto_broadcast` |
| `MustNotAct` | 触发任一副作用 | `side_effect_executed`（逐条列出副作用） |
| `MustCancel` | 未以取消结束 | `not_cancelled` |

细节上很重要的一点：`HighRiskFact` **只对自动播报生效**。澄清、转人工、拒答都是正确行为；自动播报但内容不含被禁断言也不算违规——它违反的是「不许播出**这条**断言」，不是「不许播报」。

四条判据都是**一票否决**项（逐用例产生 `Finding`），不是比例项：§6 说的是「高风险事实**不允许**自动播报错误」，不是「错误率低于某个百分比」。

## 三项正确率分别报告

`QualityReport` 保持三个**独立字段**，没有任何方法把它们合并：

| 字段 | 口径 |
| --- | --- |
| `evidence_recall_at_5` | 检索侧：题级命中率 |
| `answer_accuracy` | 生成侧：已判定答案中正确的比例 |
| `retrieval_p95_ms` | 检索延迟：`LatencySamples` 的 P95，或 `None` |

§6 的原话是「检索延迟、证据正确率、最终答案正确率**分别报告**」。一个加权「质量分」会把「检索坏了但答案侥幸对」与「检索好了但生成坏了」压成同一件事，而两者的修法完全不同。因此本模块**不提供**任何总分字段。

`AnswerVerdict::Unknown`（未判定）从 `answer_accuracy` 的分母中**排除**，不计为错误：未判定既不是对也不是错，计入错误会让人工尚未复核在报告里读成质量下降。

## 检索 P95 门槛与未测量

`RETRIEVAL_P95_LIMIT_MS = 300`：恰好等于 300ms 判**达标**，大于判**超标**。分位实现直接复用 W29 的 `LatencySamples::percentile`（nearest-rank，不插值；`MIN_PERCENTILE_SAMPLES = 20`），因此口径与 §12.5 的延迟报告完全一致——一份实现，一个真相。

样本数低于门槛时 `retrieval_p95_ms` 为 `None`，`FIELD_RETRIEVAL_P95` 进 `unmeasured`，门槛项报 `retrieval_p95_unmeasured`，且 `retrieval_meets_target()` 返回 `None` 而非 `Some(false)`/`Some(true)`——**未测量的门槛计为未达标**。

## 回归门禁的四条拒绝

`regression(baseline, candidate)` 返回 `RegressionReport { allowed, findings }`，在四种情况下拒绝放行：

1. **版本不一致** → `VERSION_INCOMPATIBLE`（直接 `Err`，不做比较）。§5.9 说黄金集随场景版本，跨版本比较没有共同基准。
2. **候选任一门槛失败** → `candidate_gate_failed`。即使基线的门槛也没达标，也**不**用「不比基线差」放行候选——那会把历史缺陷一起带过去。
3. **逐用例由通过变失败** → `case_regressed`。这是最关键的规则：平均指标不降而单题翻转是最典型的**静默退化**。测试里 1000‰ → 989‰（不到一个点）也必须被拒。
4. **召回或答案正确率低于基线** → `recall_regressed` / `accuracy_regressed`。

`allowed` 仅在无任何命中时为真。

## 坏案例回流

`BadCasePool` 区分**入池**与**出池**：`record` 开放（被改写与被驳回的候选都可入池），`promote(index, confirmer)` 必须给**具名确认人**。空确认人 → `PERMISSION_DENIED`；索引越界 → `INVALID_ARGUMENT`。提升成功返回 `CaseOrigin::Promoted { confirmer }` 的 `GoldenCase`，确认人随用例保存——**未被确认的候选在类型上无法伪装成 authored**。

§5.9 的「经人工确认后扩充黄金集」是一条授权边界：若坏案例能自行写进黄金集，黄金集就变成「系统自己给自己出题」，回归门禁随之失去意义。

## 报告自洽审计

`audit_report(&QualityReport)` 是对**上报报告值**的纯函数，报出不能同时成立的项：

| token | 含义 |
| --- | --- |
| `cases_run_exceeds_total` | 运行数多于总题数 |
| `recall_exceeds_unity` | 召回比率分子大于分母 |
| `accuracy_exceeds_unity` | 正确率分子大于分母 |
| `unmeasured_field_reported` | 未测量字段却报了值 |
| `measured_field_unmarked` | 报了值却未标注测量状态 |

报告字段全部公开，因此每条规则都能在测试里直接构造违规，不会被 `evaluate` 的构造保证架空。

## 未做边界

- **不含任何真实评估**：100 道真实问题、真实模型在这些题上的回答、真实检索结果与时延都由外部场景包与评估流程提供。测试里那 100 题集合是**构造出来的**，只为证明覆盖审计与门槛判定可执行，不代表任何真实质量水平。
- 不接真实采样器（检索时延由宿主提供）；不做评估 UI；不改 `knowledge-pipeline`（它管**怎么检索**，本模块管**怎么判定检索质量**）。
- 注册表 B01 因此为 `implemented` 而**非** `tested`：机制可测不等于真实质量已认证。
