## ADDED Requirements

### Requirement: A baseline section is parsed inside its own bounds

按章节号取表的 `sectionTable` SHALL 把搜索域封闭在该节标题与**下一个层级不高于它的标题**之间，
表与「表后首段」MUST 落在域内。三种情形 MUST 分开具名：①该节标题不存在；②该节存在但**本节内没有表**；
③解析成功。情形② MUST NOT 回落到**邻节**的表，也 MUST NOT 用「解析出一张空表」代替。

#### Scenario: A section whose own table was removed or moved away

- **WHEN** §57.3 的标题仍在，而本节内的表被删除，或被一个更浅的标题挡开（表仍在文档里）
- **THEN** 对账 MUST 报「§57.3 has no table inside its section」，
  MUST NOT 把紧随其后那一节的表当作 §57.3 自己的表

#### Scenario: A deeper heading inside the section

- **WHEN** 该节内部出现层级更深的标题（如 `####`）
- **THEN** 它 MUST NOT 截断该节的搜索域，本节内的表仍然 MUST 被取到

#### Scenario: The three parsed sections still resolve

- **WHEN** 对基线正文解析 §48.5／§51.3／§57.3
- **THEN** 每一节的表 MUST 落在该节边界内——边界若被收得过紧，三节都会报「本节内无表」

#### Scenario: A prose section is stated as such, not remembered

- **WHEN** 对 §51.2 取表（W78 的规格刻意不读它）
- **THEN** MUST 得到「本节内无表」，使「刻意不读 §51.2」从**记性**变成**会被检查的前提**
