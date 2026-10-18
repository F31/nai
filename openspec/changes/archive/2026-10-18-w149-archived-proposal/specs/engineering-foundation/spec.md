# Spec delta: W149 — 归档记录的结构判据与具名台账

## ADDED Requirements

### Requirement: Every archived proposal states why it exists, and the ledger names the exceptions

每一份归档 proposal（`openspec/changes/archive/<变更>/proposal.md`）MUST 含 `## Why` 与
`## What Changes` 两段且正文非空；标题归一化只到大小写不敏感与 `Changes` 的 `s` 可选。
归档目录下**任意**已跟踪文件都把该变更带进域，因此「目录在、proposal 不在」也 MUST 被判。
不合规的 MUST 具名失败；唯一的放行方式是**具名台账**（`{change, missing, sinceBaseline, reason}`），
且台账 MUST 相称（声明的缺口 == 实测缺口）、MUST 自清理（文档补齐后条目仍在 ⇒ 失败）。

#### Scenario: A new archive omits the Why section

- **WHEN** 一份新归档的 proposal 只有 `## What Changes`
- **THEN** 判据具名失败并指出缺的是 `## Why`：归档之后不会再有第二个地方记录「这次为什么做」，
  而 `archive.js` 的 `validateChange()` 是 non-blocking（ADR 0117），警告过后就再没人问

#### Scenario: The heading is there but the body is empty

- **WHEN** `## Why` 存在，其下没有任何内容
- **THEN** 判据具名失败，且措辞与「缺段」不同：留着标题删掉句子，是让缺口看起来像已经答过

#### Scenario: The heading is a near miss

- **WHEN** 归档 proposal 用 `## What` 或全中文标题（`## 为什么` / `## 变更`）
- **THEN** 判据 MUST NOT 接受。把近义词并进必需段等于用宽容替代一致；这两份正是台账存在的理由，
  而不是归一化该放行的样本

#### Scenario: The ledger no longer describes the document

- **WHEN** 台账声明某变更缺 `[why]`，而该 proposal 实际缺 `[what]`，或已经补齐两段
- **THEN** 两种情形**分别**具名失败：前者是「例外盖住的不是这份文档」，后者是「例外比理由活得久」。
  一条描述不准的例外等于没有例外，还把真正的缺口挡在报告之外

#### Scenario: The ledger names a change that is not there

- **WHEN** 台账里某条指向一个不存在的归档变更
- **THEN** 判据具名失败：没有主体的例外让台账不可读——下一位读者无法区分「仍生效的例外」
  与「忘了删的旧账」

#### Scenario: The domain is empty or the shipped corpus drifts

- **WHEN** 调用方给了空集，或真实语料里出现了台账之外的缺段文档
- **THEN** 空集 MUST 具名失败（把「没东西可判」写成「都合规」是把不知道写成没有）；语料漂移 MUST
  具名到具体变更。判据的域 MUST 由 `.git/index` 派生，未跟踪的归档目录不在域内——这条边界如实登记
