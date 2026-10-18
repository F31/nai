# Proposal: W150 — 归档增量与主规格的对账

## Why

W149 的 ADR 0123 把一条残余写在了「本轮不做」里：**归档记录里 `specs/` 那半边没有任何判据**。
本轮做了候选勘定，先把那条残余里已经齐备的部分量掉，再量它剩下的部分：

| 事实 | 值 | 出处 |
| --- | --- | --- |
| 归档变更目录 | **146** | `openspec/changes/archive/` 实测 |
| 有 `tasks.md` / 有 `specs/` | **146 / 146** | 两项都零缺失 ⇒ 「结构齐备」这一半今天**零信号** |
| 归档 delta 文件 | **178** | `archive/<change>/specs/<cap>/spec.md` |
| 主能力 | **43** | `openspec/specs/*/spec.md` |
| 主规格 `### Requirement` | **497** | 重名 0、形态异常 0 |
| delta 段标记 | `ADDED` 164 处、`MODIFIED` 34 处、`REMOVED` **0** 处 | 非标记的 `##` 标题只有 `Purpose`（53 处） |
| delta 里的 Requirement | **539** | 全在主规格里（缺 **0**）；主规格 497 条也全有出处（无出处 **0**） |
| 逐能力比对 | **43 / 43 相等**，`deltas_only` 0、`main_only` 0 | 顺序重放后的集合与主规格逐能力相同 |
| `validate --all --strict` | **43 / 0** | 手工把一条 Requirement 改名后仍是 43 / 0 |
| `validate --archived` | **146 / 0** | 同一次探针，仍是 146 / 0 |

最后两行的意义是：**「归档了」与「合入了」之间的这段距离对现有门控零信号**。`openspec archive`
把 delta 挪进归档目录，但主规格那一侧没人问；`--archived` 判「归档记录能不能解析」，
`--all --strict` 判「主规格自身形状」。把一条 `### Requirement:` 改名（等价于「归档了但没合入」）
之后两层全绿，这就是本轮要补的那一格。

## What Changes

- 新增 `scripts/contracts/spec-merge.mjs`：`DELTA_MARKS` / `DELTA_SECTION_TITLES`（段标记闭世界）、
  `deltaRefsOf` / `mainSpecRefsOf`（两个域 + **重放顺序**）、`deltaSections` / `mainRequirements`
  （两个解析器）、`specMergeErrors`（纯函数，入参可注入）。
- 判据形状：按归档目录名（日期前缀）顺序重放 `ADDED` / `MODIFIED` / `REMOVED`，
  逐能力与主规格比对，**两个方向各自具名**——`delta − 主规格` 是「归档了但没合入」，
  `主规格 − delta` 是「主规格里有一条从没经过归档的 Requirement」，`REMOVED` 的另一方向
  单独措辞（它有出处，只是出处的结论是删掉）。
- 新增 `tests/contracts/spec-merge.test.mjs`（5 条）：真实语料基线 + 非空洞五项 + 重放顺序；
  合成样本覆盖「顺序重放 vs 集合并集 / 两方向 / `MODIFIED` 不判先存 / 段标记闭世界 /
  段外 Requirement / 空 delta / 缺 delta 文件 / 能力整体缺席 / 主规格无 `## Requirements` / 空集」；
  域本身（重放顺序与两个域只由归档布局决定）。
- 门控接线：`xtask/src/lib.rs` 的 `engineering_checks` 增加一步；`TEST_REGISTRATION_SITES` 增加
  一条 `tier: portable` 的站点 `xtask-spec-merge`；`tests/registry.test.mjs` 的逐字清单加到十九个 id。
- ADR 0124 记录四条决定（顺序重放、两方向具名、不判 `MODIFIED` 先存性、能力整体缺席只报一条）
  与两处**零样本**分支的如实登记。

## Impact

- **门控多一步纯静态 Node 测试**（读 `.git/index` 与 `openspec/` 下的文本，不需要 protoc、不编译）。
- **不改任何规格文本**：今天 43 个能力、497 条 Requirement 全部对得上，判据首跑为绿。
- **不改归档流程**：不加 `archive.js` 的阻塞路径（外部工具语义不在本仓库），判据落在门控里。
- 与两层现有校验**不重叠**：`--strict` 判主规格自身形状、`--archived` 判归档记录可解析，
  本判据判的是两者**之间**的一致；实测把一条 Requirement 改名后两层仍全绿，即证。
- **本判据的盲区如实登记**：`REMOVED`（语料 0 处）与「主规格没有 `## Requirements` 段」
  （语料 0 份）两条分支只由合成用例覆盖，不由真实语料验证。
