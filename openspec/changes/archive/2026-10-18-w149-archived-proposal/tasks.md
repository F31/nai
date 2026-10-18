# Tasks: W149 — 归档记录的结构判据与具名台账

## 1. 勘定（先量，再判）

- [x] 量归档语料：145 个目录、145 份 `proposal.md`、143 合规、2 例外（`## What` 1 份；全中文标题 1 份）
- [x] 量标题变体：`Why` 只有一种写法；`What Changes` / `What changes` 两种 ⇒ 归一化只需大小写不敏感
- [x] 量空段：0 份（判据仍要判它——「标题在」≠「这件事付过账」）
- [x] 证实现有门控零信号：`openspec validate --archived` 对这 2 份都是 ✓（**145 / 0**）

## 2. 判据

- [x] `scripts/contracts/archived-proposal.mjs`：`REQUIRED_SECTIONS`（归一化唯一出处）
- [x] `archivedChangeNames`：域 = 归档目录下**任意**已跟踪文件（不只 `proposal.md`）
- [x] `sectionsOf`：行首 `## ` 切段、同名取第一处、正文 `trim`
- [x] `archivedProposalErrors`：缺段 / 空段 / 无 proposal / 台账相称 / 台账自清理 / 台账无主体 / 空集
      七类各自具名失败；例外**抑制**它盖住的那几条报错（否则它不是例外）

## 3. 判别性样本（3 条）

- [x] 真实语料基线：零错 + 非空洞四件（域非空、逐份判、台账全在用、规则不是只被台账满足）
- [x] 合成样本：缺段 / 空段 / `what changes` 接受 / `## What` 拒绝 / 中文标题拒绝 / 无 proposal /
      台账相称 / 台账陈旧 / 台账描述错 / 台账无主体 / 空集
- [x] 域本身：`b/tasks.md` 让 `b` 进入域；无斜杠路径与非 `archive/` 路径不进域

## 4. 注入验证（逐次 sha256 还原，基线 `316d5cfd…92a8`）

- [x] W1 关掉台账查询 ⇒ **2 红**（`#1` `#2`）
- [x] W2 归一化放宽成 `## What` ⇒ **3 红 / 0 绿**（语料与合成样本同时红）
- [x] W3 去掉空段判据 ⇒ **只红 `#2`**（空段恰是它那一层）
- [x] W4 域只认 `proposal.md` ⇒ **只红 `#3`**（「目录在、proposal 不在」恰是它那一层）

## 5. 接线与收口

- [x] `xtask/src/lib.rs` 的 `engineering_checks` 增加一步（注释写明它补的是哪条零信号）
- [x] `TEST_REGISTRATION_SITES` 增加 `xtask-archived-proposal`（`tier: portable`）
- [x] ADR 0123；轮次报告；`docs/development-plan.md` W149 段；`docs/engineering.md` 一句
- [x] 门控 `cargo xtask check --profile full`；两条 validate（含 `--archived`）在推送前绿
- [x] 一次 commit + push；CI 双平台观测；回填 §六/§七
