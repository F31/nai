## ADDED Requirements

### Requirement: A tested acceptance names a non-empty archived evidence report
注册表中状态为 `tested` 的验收项 SHALL 携带**非空**的 `evidence`，且其值 MUST 指向归档报告目录（相对 `docs/` 记作 `reports/…`）。

#### Scenario: An empty evidence field on a tested item is rejected
- **WHEN** 某个 `tested` 验收项的 `evidence` 为空
- **THEN** 判据以该 id 具名失败，指出 `evidence` 为空

### Requirement: A tested acceptance's evidence report is a repository member
`tested` 验收项的证据报告 SHALL 是**仓库成员**（由 `.git/index` 派生），MUST NOT 仅以「磁盘上存在」为凭。

#### Scenario: An untracked report is rejected even if it exists on disk
- **WHEN** 某个 `tested` 验收项的 `evidence` 指向一个未被 git 跟踪的报告路径
- **THEN** 判据以该 id 具名失败，指出该路径不是仓库成员

### Requirement: The tested-evidence scan is closed over the whole tested set
对 `tested` 证据的扫描 SHALL 覆盖**全部在册 `tested` 项**，MUST NOT 静默跳过任何一项；且 MUST 带有非空洞下限，使扫描不会因集合为空而恒真。

#### Scenario: Every tested item is scanned and the set is non-vacuous
- **WHEN** 执行证据可核验判据
- **THEN** 被扫描的 `tested` 项数 MUST 等于 registry 中 `tested` 项总数，且不少于声明的下限
