# Spec delta: W150 — 归档增量与主规格的对账

## ADDED Requirements

### Requirement: Archived spec deltas are replayed in order and reconciled with the main specification

每份归档 delta（`openspec/changes/archive/<变更>/specs/<能力>/spec.md`）MUST 按归档目录名顺序重放
（`ADDED` / `MODIFIED` 加入，`REMOVED` 删除），重放结果 MUST 与主规格
（`openspec/specs/<能力>/spec.md` 的 `## Requirements` 段）逐能力相等。
delta 有而主规格无 MUST 具名失败（「归档了但没合入」）；主规格有而无任何 delta 出处 MUST 具名失败
（「没有归档出处」）。一个能力整体缺席主规格时 MUST 只报**一条**（逐条报会把「这份主规格不存在」
稀释成 N 个数字）。判据 MUST NOT 要求 `MODIFIED` / `REMOVED` 的目标此前存在：实测有两条合法的历史
记录改的是各自 delta 之前就存在的 Requirement。

#### Scenario: A change was archived but its requirement never reached the main spec

- **WHEN** 某份归档 delta 里有 `### Requirement: X`（`ADDED` 或 `MODIFIED`），而主规格的
  `## Requirements` 段里没有 `X`
- **THEN** 判据具名失败并指出变更名与 Requirement 名：归档这个动作不等于应用，中间那一步今天
  没有任何判据在看，而人在记忆里已经把「归档了」记成「做完了」

#### Scenario: The main spec carries a requirement no archive ever wrote

- **WHEN** 主规格里有 `### Requirement: Y`，而全仓库 146 份归档 delta 都没有写过 `Y`
- **THEN** 判据具名失败，措辞与上一场景不同：这一条的处置是补记录（谁、为什么），不是补合入。
  两个方向合成一句「重放 == 主规格」会让两种不同的失败看起来是同一件事

#### Scenario: A removal was archived and never applied

- **WHEN** 某份归档 delta 用 `REMOVED Requirements` 删掉了 `Z`，而主规格仍然带着 `Z`
- **THEN** 判据具名失败，且 MUST NOT 用「没有归档出处」那句措辞：`Z` 有出处，只是那份出处的
  结论是「删掉」。用前一句报它，读者会以为文档写错了，而实际错的是「删了却没删」

#### Scenario: A capability exists on only one side

- **WHEN** 某能力有归档 delta 但没有主规格，或有主规格但没有任何 delta 写过它
- **THEN** 每种情形各报**一条**，并给出能力名与期望路径。逐条报会让同一个缺口看起来像 N 个缺口，
  而读者看到的是 N 这个数字，不是「一个能力整个没合入」

### Requirement: A section mark or a requirement the replay cannot place is named, never skipped

delta 的 `##` 段标记是**闭世界**（`ADDED` / `MODIFIED` / `REMOVED` Requirements 与 `Purpose`）。
落在段标记之前、落在 `## Purpose` 之下、或落在不认识段里的 `### Requirement:` MUST 具名失败，
MUST NOT 静默跳过；主规格里落在 `## Requirements` 之外的 Requirement 同样 MUST 具名失败。
delta 文件读不到时 MUST 具名失败并**跳过该能力的比对**，MUST NOT 拿残缺的重放集去比；
判据收到的 delta 集或主规格集为空时 MUST 具名失败。

#### Scenario: A requirement sits where the replay cannot place it

- **WHEN** delta 里出现 `## CHANGED Requirements` 这类不认识的段标记，或有 `### Requirement:`
  落在第一个段标记之前、落在 `## Purpose` 之下；或主规格里有 Requirement 落在 `## Requirements`
  之外
- **THEN** 判据具名失败。静默跳过等于把那条 Requirement 藏起来：它读起来像生效了，
  而重放里根本没有它的位置。「不认识」与「没有」在判定里是两件事

#### Scenario: The replay cannot be established, or there is nothing to judge

- **WHEN** 归档目录列出了某能力但 `spec.md` 读不到；或调用方给了空的 delta 集 / 主规格集
- **THEN** 前一种情形具名失败并且跳过该能力的比对——拿残缺的重放集去比，会造出「主规格里有一条
  没出处」这种假结论，那是把「不知道」写成「没有」；后一种情形 MUST 具名失败，MUST NOT 把
  「没东西可判」写成「都合入了」。判据的域 MUST 由 `.git/index` 派生，未跟踪的归档目录不在域内
  ——这条边界如实登记
