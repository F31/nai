# basic-selling-loop Specification

## Purpose
为 nai 提供**可验证的基础讲品循环**（V3.9 §53，验收 B07）：把「持续、自然地讲清楚一件商品」所依赖的
**确定性脚本与规则**落成判据——段落编排与顺序、插问后归位、段落级冷却与切换、事实约束的行动提示。
使「这个编排计划是否合规」可被机器回答，而不是靠读代码与会议。
本能力只判**规则**：段落内容与话术、TTS 时长与回放阈值属内容创作与运行时；
§5.7 的「恢复前重新校验产品、价格、活动、素材与授权」由守门与知识层执行。

## Requirements

### Requirement: The selling loop follows a fixed segment order

基础讲品循环的段落编排 MUST 恰为 §53 的固定四段与顺序（卖点 → 规格 → 适用场景 → 售后说明）。
未定义的段落类型、缺段、乱序 MUST 各自**具名拒绝**；缺段 MUST 指名缺了哪些，MUST NOT 只报数量。
判定 MUST NOT 只比较段落**集合**——一个排序后比较集合的实现不满足本需求。

#### Scenario: A segment is missing

- **WHEN** 编排序列缺少售后说明
- **THEN** 具名拒绝并**指名**缺失的段落，MUST NOT 只报「数量不符」

#### Scenario: The right segments in the wrong order

- **WHEN** 四段齐备但顺序为「规格 → 卖点 → 适用场景 → 售后说明」
- **THEN** 具名拒绝；一个先排序再比较集合的实现会放行，故不满足本需求

#### Scenario: An undefined segment name

- **WHEN** 编排序列里出现四段之外的段落名
- **THEN** 具名拒绝，且该名 MUST NOT 被悄悄忽略或当成已知段落

### Requirement: An interjection return resumes at the checkpoint's segment, and the checkpoint holds no unspoken text

检查点 MUST 只保存**所指段落**与**已完成句引用**，MUST NOT 保存未播出的生成文本；
恢复位置 MUST 是检查点所指段落。检查点携带未播出文本、恢复位置与检查点不符，
MUST 各自具名拒绝。恢复前对产品、价格、活动、素材与授权的重校验由守门与知识层执行，
MUST NOT 被读作已由本判定完成。

#### Scenario: The checkpoint carries generated text that was never played

- **WHEN** 一个检查点里存了尚未播出的生成文本
- **THEN** 具名拒绝——「只保存段落与已完成句」是本需求的判据，不是注释

#### Scenario: The answer returns somewhere else

- **WHEN** 插问处理完毕，恢复位置不是检查点所指的段落
- **THEN** 具名拒绝，MUST NOT 以「落在某个合法段落」放行

### Requirement: Segment cooldown and switching intervals gate re-entry

同一段落的重复进入 MUST 不早于段落级冷却，任意相邻两次进入 MUST 不早于最小切换间隔；
两者 MUST **分开**判定。进入时刻序列 MUST 非降序，否则具名拒绝。阈值由调用方给定，
判定 MUST 只回答「按给定阈值成立与否」，MUST NOT 内建一套数字，也 MUST NOT 采用外部观察值。

#### Scenario: The same segment comes back too early

- **WHEN** 同一段落的两次进入间隔小于冷却阈值
- **THEN** 具名拒绝，且该拒绝 MUST NOT 与「切换过近」混为一条

#### Scenario: Entrance times are not monotonic

- **WHEN** 进入时刻序列出现下降
- **THEN** 具名拒绝——趋势不得靠重排制造

### Requirement: Constrained claims bind to a currently-effective fact revision

行动提示中的受约束断言（价格、库存、优惠与活动条件、承诺与售后政策、个体化承诺）
MUST 绑定事实引用，且引用的修订 MUST 是**当前有效版本**。无绑定与引用过期 MUST **分开**具名拒绝；
只校验「引用非空」的实现不满足本需求。

#### Scenario: A price with no fact behind it

- **WHEN** 行动提示断言了价格，却没有绑定任何事实引用
- **THEN** 具名拒绝——语义相似不构成证据

#### Scenario: A stale price

- **WHEN** 受约束断言绑定了事实引用，但引用的修订不是当前有效版本
- **THEN** 具名拒绝；一个只检查「引用非空」的实现会放行，故不满足本需求
