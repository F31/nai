# spec-driven-delivery 变更（w72）

## MODIFIED Requirements

### Requirement: Corrections distinguish stale statements from provenance

基线与其活文档的就地订正 SHALL 只作用于**与当前事实不符的陈述**，并按 W65 先例分**四种情形**：
① 声明「**当前基线是哪一份**」的位置（`docs/design.md`、`README.md`、`AGENTS.md`、`openspec/config.yaml`、
输出串与界面文案）MUST 随基线更新，或改为**不写死版本号**（§56-C9 先例）；
② **`§NNN` 的版本前缀是行号契约的锚点**（解析为**当前基线**），故 MUST 随基线更新；
③ **关于某一版的陈述**（升基线链条、该版「收缩/新增/晚了什么」、按该版实测的数字）与**该版专属的版本章节**
MUST NOT 随之改写；④ 代码 doc-comment 的版本前辍 MUST NOT 改。
②与④的分界是**载体**：doc-comment 记归属，输出串与错误消息是**当前值**。
四情形 MUST 被分别识别，MUST NOT 以「版本号出现即改」的规则批量替换。

#### Scenario: A baseline version bump sweeps live documents

- **WHEN** 基线由 Vx 升为 V(x+1)，活文档中同时存在「唯一实现基线为 Vx」「该条款见 Vx §NNN」与「Vx 收缩了什么」三类陈述
- **THEN** 情形①与② MUST 被改写（或去掉版本号），情形③ MUST 保留原文；
  MUST NOT 只做前两者，也 MUST NOT 把三者一并替换

#### Scenario: A provenance reference is swept into a false claim

- **WHEN** 一次「订正」把**关于某一版**的陈述（情形③）一并改写——例如「V3.9 收缩」→「V3.10 收缩」，
  或「源自 V3.8 新增第四部分」→「源自 V3.10」
- **THEN** 该改写 MUST 被拒绝：归属变成了不实陈述，而订正的目的恰恰是消除不实陈述
- **AND** 与之相邻的**章节指针**（情形②，如「V3.9 §51.3」）MUST 反而被更新为当前基线——
  两者 MUST 被分别识别，MUST NOT 以「这一段里有版本号」为理由一并处理

#### Scenario: An earlier draft of this rule is still quoted as the rule

- **WHEN** 某处（规格、活文档或 ADR 的追加段）仍写着「**版本归属引用**（形如「V3.9 §51.3」）MUST NOT 随基线号变更」
  这类**已被 W65 先例否证**的表述——W71 的 E5 首版判据即此形状，其残留散落在规格与两处 ADR 追加段
- **THEN** 该表述 MUST 被视为**待订正的陈述**并按上列四情形改写：
  规格（活的主规范）就地订正；`docs/reports/`、`docs/adr/`、`openspec/changes/archive/` 保留原文 + **追加勘误段**
