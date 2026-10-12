# spec-driven-delivery 变更（w76）

## ADDED Requirements

### Requirement: Pending decisions carry a disposition and a basis

基线各章「待决策与待确认」的条目 SHALL 被登记进一份机器可读台账（`tests/pending-decisions.yaml`）。
每条 MUST 有**封闭枚举的处置**与**一条非空洞依据**：`answered_by_baseline` MUST 给 `basis`，
其中每个 `§NNN` MUST 真实存在于当前基线；`decidable_in_repo` MUST 给 `recommendation`；
`needs_precondition` MUST 给一组**真实存在**的前置（验收 id 或工作项编号）；
`needs_external` MUST 给 `closure`（取到什么算闭合）；`out_of_scope` MUST 给 `note`。
「待裁定」MUST NOT 被读作「无人负责」。

#### Scenario: A pending decision is answered by the baseline itself

- **WHEN** 某条待裁定项在基线正文里已有答案（如打包粒度已由 §51.3 的能力包清单回答）
- **THEN** 它 MUST 被登记为 `answered_by_baseline` 并给出该 `§NNN`；MUST NOT 被重新裁定，
  MUST NOT 继续留在「待裁定」里悬空

#### Scenario: A basis points at a section that does not exist

- **WHEN** 某条 `answered_by_baseline` 的 `basis` 引用一个当前基线里不存在的章节号
- **THEN** 校验 MUST 失败并指名该条目与那个章节号——指向不存在章节的依据与没有依据是同一件事

#### Scenario: An external item has no closure condition

- **WHEN** 某条 `needs_external` 缺 `closure`，或 `closure` 为空
- **THEN** 校验 MUST 失败——外部条件无法被「完成」，只能被「以什么为标志闭合」

#### Scenario: A precondition names something that does not exist

- **WHEN** 某条 `needs_precondition` 的前置写成 registry 里没有的验收 id，或基线里没有的工作项编号
- **THEN** 校验 MUST 失败并指名它——把待裁定项挂在不存在的东西上，等于把它挂在了空气上

#### Scenario: The ledger silently loses an item

- **WHEN** 台账少登记了基线列的条目（总数或某一章的新增数低于基线事实）
- **THEN** 校验 MUST 失败并给出两侧计数——覆盖性 MUST 由「基线各章的新增条数」**逐章写死**保证，
  MUST NOT 由「台账自己说自己全」保证
