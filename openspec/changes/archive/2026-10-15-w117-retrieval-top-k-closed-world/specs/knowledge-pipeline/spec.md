## ADDED Requirements

### Requirement: Remote search truncates evidence to top_k preserving order
远端检索结果 SHALL 截断为不超过 `top_k` 项，且保留远端返回序列的前 `top_k` 项及其相对顺序。

#### Scenario: Evidence is truncated to the first top_k in order
- **WHEN** 远端 `/search` 返回 5 个有效证据项且以 `top_k = 2` 调用检索
- **THEN** 结果恰为前 2 个证据项且顺序不变（`[e0, e1]`）

### Requirement: Remote search applies top_k after dropping empty-ID evidence
远端检索结果 SHALL 在丢弃 `evidence_id` 为空的证据项**之后**再按 `top_k` 截断，即 `top_k` 统计的是有效证据项数量而非原始回包项数。

#### Scenario: top_k counts valid evidence after dropping empty IDs
- **WHEN** 远端 `/search` 返回 `[e0, "", e1, e2]`（含 1 个空 id 项）且以 `top_k = 2` 调用检索
- **THEN** 结果为 `[e0, e1]`（长度 2，空 id 项不占用 `top_k` 名额）

### Requirement: Remote search with zero top_k yields empty evidence
远端检索结果在 `top_k = 0` 时 SHALL 返回空证据集合，不得静默视为「不截断」。

#### Scenario: top_k zero truncates to empty
- **WHEN** 以 `top_k = 0` 调用检索（远端返回非空证据）
- **THEN** 结果证据集为空

### Requirement: Remote search with top_k exceeding reply keeps all evidence
远端检索结果在 `top_k` 大于有效证据项数时 SHALL 保留全部有效证据项，不得补齐占位项、不得报错。

#### Scenario: top_k greater than evidence count keeps all
- **WHEN** 远端 `/search` 返回 5 个有效证据项且以 `top_k = 10` 调用检索
- **THEN** 结果含全部 5 个证据项且顺序不变
