# spec-driven-delivery 变更（w87）

## ADDED Requirements

### Requirement: Historical records are protected by the commit boundary

对**历史记录**（`docs/reports/`、`docs/adr/`、`openspec/changes/archive/`）的处置是
「保留原文 + 追加勘误段」，其**保护对象是「已发布的原文」**——判据 = 该原文**是否已进入提交**
（即是否曾在**工作区之外**可见）。**同一轮内在工作区内新建、尚未提交的产物**（例如本轮创建并归档的
change 目录）**MUST 就地订正**，不产生勘误段（它从未发布，没有「已发布的原文」需要保护）；
**已发布**的原文 **MUST NOT 被就地改写**，只能**追加勘误段**。该判据 MUST NOT 依赖「轮次」——
「本轮」在多人协作下无定义，只有**提交边界**可判。

#### Scenario: A same-round archive is edited in place, a published one is not

- **WHEN** 一份变更目录在本轮被**创建并归档**且**尚未进入任何提交**（同一次未提交工作区内），
  随后发现其中的转述有误
- **THEN** 该产物 MUST 被**就地订正**——它从未发布，没有「已发布的原文」需要保护；
  MUST NOT 因「它已在 `openspec/changes/archive/` 路径下」就把它当作冻结的已发布原文

#### Scenario: A published archive is rewritten instead of corrected

- **WHEN** 一份**已进入提交**的 `docs/reports/`、`docs/adr/` 或 `openspec/changes/archive/` 产物
  的**原文**被直接改写
- **THEN** 该改写 MUST 被拒绝，正确处置是**追加勘误段**——保护的对象是「已发布的原文」，
  与它此刻是否仍在工作区无关
