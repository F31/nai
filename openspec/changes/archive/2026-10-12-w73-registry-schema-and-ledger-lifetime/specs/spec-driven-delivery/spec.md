# spec-driven-delivery 变更（w73）

## ADDED Requirements

### Requirement: Registered deviations do not outlive a baseline version

「如实登记之后 MUST 紧接着订正现行规范」中的**「紧接着」SHALL 被读作「不晚于下一次基线修订轮次」**：
已登记的偏差 MUST NOT 跨基线版本存活，其判定形式是台账条目的 `since_baseline` MUST 等于当前基线版本。
MUST NOT 被读作「同一轮内」（不可行），也 MUST NOT 被读作「N 轮之内」（轮数是外部观察值）。
跨版处置只有两种：**收录**，或**显式结转**（改版本 + 记结转轮次 + 非空理由）。

#### Scenario: An infeasible deadline is not enforced as written

- **WHEN** 某偏差在实现轮次内登记，而该轮**不是**升基线轮次（基线被 ADR 批准后不可变，
  就地订正又有 MUST 1:1 行替换的硬约束 ⇒ 改 §57.3 只能发生在一次专门的升基线轮次里）
- **THEN** MUST NOT 因「未在同一轮内订正」而判为违规——那条读法没有可执行的落点；
  判据 MUST 是「**下一次**基线修订轮次是否处置了它」

#### Scenario: A deviation is carried across a baseline upgrade in silence

- **WHEN** 一次升基线之后，台账里仍有 `since_baseline` 指向旧版本的条目，而该条目既未被收录、
  也未被显式结转
- **THEN** 门控 MUST 失败并指名该条目与两侧版本；正确做法是把「这一版故意不收它」**写下来**
  （改 `since_baseline` + 记结转轮次 + 非空理由）——沉默与记录是两个不同的状态，
  本条只禁止前者
