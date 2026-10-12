## ADDED Requirements

### Requirement: The capability-pack split is a partition stated by §51.3

§51.3 的「能力包清单」SHALL 被**解析**并与在册对账，MUST NOT 只被引用。表的行数、**逐行标签与编号**、
以及每行「依据」非空，MUST 逐项判定。该表 SHALL 是一**划分**：任一编号 MUST NOT 同时属于两个能力包。
表后「保留在 R1 的基础纪律」列出的编号 MUST 非空且与能力包并集**不相交**。
`§51.3 的并集`、registry 的 `capability_pack` 行、门控常量，三者 MUST 一致。

#### Scenario: A pack row silently shrinks

- **WHEN** §51.3 的某一行把编号改小（如 `M01–M12` 改成 `M01–M02`），而行的数量不变
- **THEN** 对账 MUST 失败并**同时给出该行的标签与两侧的编号集合**，MUST NOT 因为「行数没变」而通过

#### Scenario: An identifier belongs to two packs

- **WHEN** 某个编号同时出现在 §51.3 的两行里（并集因此看起来仍然完整）
- **THEN** 对账 MUST 失败并**同时给出该编号与它所在的两个包**——打包粒度是一个划分，不是一个并集

#### Scenario: A pack is stated without a reason

- **WHEN** §51.3 的某一行「依据」列为空
- **THEN** 对账 MUST 失败并指名该包——没有写下来理由的能力包，就是一个没人复核过的能力包

#### Scenario: The R1-retained discipline list is emptied or overlaps the packs

- **WHEN** §51.3 表后「保留在 R1 的基础纪律」那段列不出任何编号，或列出的编号与能力包并集相交
  （例如把 `H18` 从该段移进包行）
- **THEN** 对账 MUST 失败：分界判据（*一个确定性的上下文装配函数能不能满足它？*）必须两侧都非空且互斥，
  MUST NOT 因为「解析结果为空集」而**通过**

#### Scenario: The gate constant drifts from the baseline it mirrors

- **WHEN** 门控用的能力包常量集合与 §51.3 表格展开后的并集不再相同（常量漏一条或增一条）
- **THEN** 对账 MUST 失败并具名给出差异的编号与两侧来源，MUST NOT 只在「常量与 registry 相符」时通过
