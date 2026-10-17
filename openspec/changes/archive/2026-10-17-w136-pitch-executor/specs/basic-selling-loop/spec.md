# Spec delta: W136 — 讲品循环的执行体

## ADDED Requirements

### Requirement: A running selling loop is judged one entry at a time, and a refused entry never advances it

运行中的讲品循环 MUST 在**每一步进入**上判定，判定 MUST 使用与批次判定**同一处**定义的规则；
被拒的那一步 MUST NOT 记录进入、MUST NOT 移动游标。忽略返回错误的调用方 MUST NOT 因此跑到规则前面。
段落级冷却 MUST 以**该段落自己的上一次进入**为基准，MUST NOT 只在相邻两次进入同段时成立。

#### Scenario: A refused entry does not consume the step

- **WHEN** 循环刚进入某段 50ms 后又请求进入下一段，而最小切换间隔是 100ms
- **THEN** 具名拒绝「切换过近」，且循环的已进入计数与下一段**都不变**

#### Scenario: A repeat is measured from the segment's own last entry

- **WHEN** 卖点在 0ms 进入、规格在 600ms 进入、卖点在 900ms 再次进入，段落冷却为 1000ms
- **THEN** 具名拒绝「冷却未到」——相邻两次进入不同段不构成豁免；一个只比较相邻同段对的实现会放行，故不满足本需求

### Requirement: A resume requires all five §5.7 faces to be re-checked, and an unmeasurable face is a refusal

插问后归位 MUST 携带「产品、价格、活动、素材、授权」**五面**的重校验结论，五面 MUST 各自取值；
任一面过期 MUST 具名拒绝并**逐个点名**，任一面不可测 MUST 具名拒绝，MUST NOT 读作通过。
归位 MUST NOT 被视为一次新的段落进入。本能力 MUST NOT 声称自己完成了这五面的重校验。

#### Scenario: Nobody can re-check

- **WHEN** 调用方对五个面都给出「不可测」
- **THEN** 具名拒绝并点出全部五个面——把一个没测过的面当成新的事实，等于把未测量写成授权

#### Scenario: One face is stale

- **WHEN** 五个面里只有素材过期
- **THEN** 具名拒绝并**只**点名素材，其余四面 MUST NOT 出现在拒绝名单里
