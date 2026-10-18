# Proposal: W149 — 归档记录的结构判据与具名台账

## Why

W143 §八第 1 条登记了 5 轮：**2 份归档 proposal 结构性缺段**。它一直没被接手，因为
「修」等于改已发布的历史记录，而「不修」会让任何未来的「全量 proposal 可解析」判据都必须
先给它们开例外。本轮把它量成了数字，并确认**现有门控对这条零信号**：

| 事实 | 值 | 出处 |
| --- | --- | --- |
| 归档变更目录 | **145** | `openspec/changes/archive/` 实测，每个目录都有 `proposal.md` |
| 有 `## Why` + `## What Changes` | **143** | 标题归一化后（大小写不敏感、`Changes` 的 `s` 可选） |
| 缺 `## What Changes` | **1** | `2026-10-09-engine-assembly`（写的是 `## What`，Why 235 字符在） |
| 两段都缺 | **1** | `2026-10-10-w31-golden-set-payload`（全中文标题：`为什么`/`变更`/`影响`/`否决方案`） |
| `openspec validate --archived` | **145 / 0** | 上面两份都是 ✓ —— 现有门控**零信号** |
| 段存在但正文为空 | **0** | 今天没有；判据仍要判它，因为「标题在」≠「这件事付过账」 |

还有一条**没有门控**的事实：`archive.js` 的 `validateChange()` 是 **non-blocking**（ADR 0117）。
一份新归档的 proposal 缺 Why，只会在归档时打一行警告，之后没有任何门控会再提 —— 缺口被
「归档」这个动作永久地固化了。

## What Changes

- 新增 `scripts/contracts/archived-proposal.mjs`：`REQUIRED_SECTIONS`（标题归一化的唯一出处）、
  `PROPOSAL_EXCEPTIONS`（具名台账）、`archivedChangeNames`（域 = 归档目录下**任意**已跟踪文件）、
  `sectionsOf`、`archivedProposalErrors`（纯函数，入参可注入）。
- 新增 `tests/contracts/archived-proposal.test.mjs`（3 条）：真实语料基线 + 台账与语料对账；
  合成样本逐条覆盖「缺段 / 空段 / 归一化边界两侧 / 无 proposal / 台账相称 / 台账自清理 / 台账无主体 /
  空集」；域本身（含「目录在、proposal 不在」）。
- 门控接线：`xtask/src/lib.rs` 的 `engineering_checks` 增加一步；`TEST_REGISTRATION_SITES` 增加一条
  `tier: portable` 的站点登记。
- ADR 0123 记录三条决定（判全部归档、归一化只到大小写、空段等于缺段）与台账三项纪律。

## Impact

- **门控多一步纯静态 Node 测试**（读 `.git/index` 与归档目录文本，不需要 protoc、不编译）。
- **不动历史记录**：2 份例外进台账，判据不要求改已发布的 proposal。
- **不改进程**：不加 `archive.js` 的阻塞路径（那是外部工具的语义），判据落在 CI 与本机门控里。
- 与 `openspec validate --archived` **不重叠**：那条只回答「能不能解析」，本判据回答「有没有把话说完」；
  实测前者对这两份是 ✓，证明两层判的不是同一件事。
