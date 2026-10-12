# spec-driven-delivery 变更（w71）

## ADDED Requirements

### Requirement: Corrections distinguish stale statements from provenance

基线与其活文档的就地订正 SHALL 只作用于**与当前事实不符的陈述**；**版本归属引用**——形如「V3.9 §51.3」「V3.9 收缩」——
陈述的是**该条款由哪一版引入**，MUST NOT 随基线号变更：把它一并改写，等于用一次「订正」制造一批**新的**不实陈述。
**当前基线指针**（形如「唯一实现基线为 ……V3.9.md」「不自行拼接旧条款覆盖 V3.9」）SHALL 随基线升版同步。
两类陈述 MUST 被分别识别，MUST NOT 以「版本号出现即改」的规则批量替换。
版本号 SHALL 只出现在 `docs/design.md` 与最新基线 ADR 处作为**当前值**；其余活文档 MUST NOT 把版本号当作当前事实声明。

#### Scenario: A baseline version bump sweeps live documents

- **WHEN** 基线由 Vx 升为 V(x+1)，活文档中同时存在「唯一实现基线为 Vx」与「该条款见 Vx §NNN」两类陈述
- **THEN** 只有**当前基线指针**被改写；**版本归属引用** MUST 保留——除非该条款确实是在新版本引入

#### Scenario: A provenance reference is swept into a false claim

- **WHEN** 一次「订正」把「V3.9 §51.3」（该条款由 V3.9 引入）改写为「V3.10 §51.3」
- **THEN** 该改写 MUST 被拒绝：归属变成了不实陈述，而订正的目的恰恰是消除不实陈述
