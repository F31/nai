# basic-selling-loop Specification

## Purpose
为 nai 提供**可验证的基础讲品循环**（V3.11 §53，验收 B07）：把「持续、自然地讲清楚一件商品」所依赖的
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

### Requirement: Engine owns one finite assisted program per epoch
The engine SHALL bind one four-segment assisted program to the current knowledge snapshot and epoch. Start SHALL perform no Provider calls. Explicit next SHALL generate each segment at most once, in selling/spec/scenario/after_sales order, using existing guards, approval and durable budgets. A failure SHALL terminate the program without automatic retry. The program SHALL expire within fifteen minutes. A second start in the same epoch SHALL be refused.

#### Scenario: Duplicate or premature next
- **WHEN** generation is in flight or the current segment has not been confirmed
- **THEN** another next is refused without another Provider dispatch; replay of the identical command returns its original ACK.

#### Scenario: Missing timed evidence
- **WHEN** a program segment retrieves evidence without declared active validity
- **THEN** generation fails before Provider dispatch and no automatic retry occurs.

### Requirement: Assisted confirmation cannot become hardware completion
The engine SHALL accept operator confirmation only for the current request's exact candidate hash after its local device submission. Preview rendering alone SHALL NOT qualify. Confirmation SHALL NOT produce PlaybackAck or completed-sentence evidence. Epoch, snapshot and program deadline SHALL be rechecked on every mutation; stale programs SHALL NOT resume automatically.

#### Scenario: Wrong candidate or preview only
- **WHEN** confirmation references another candidate or only an offline preview exists
- **THEN** confirmation is refused and the program does not advance.

#### Scenario: Takeover before next
- **WHEN** takeover changes the epoch
- **THEN** old program commands are refused and cannot dispatch another generation; no five-face Fresh result is fabricated.

### Requirement: Desktop consumes authoritative finite program progress
The desktop SHALL start and advance the engine program explicitly, using the published four-part outline and current session identity. Progress SHALL come from engine reads, never local increments. Generation SHALL be submitted once and polled within a bounded deadline; timeout SHALL offer read-only refresh without automatic retry. Reconnect SHALL read progress without starting, generating or replaying.
#### Scenario: An ambiguous command result
- **WHEN** a start, next or confirm response is lost
- **THEN** the desktop disables repeat mutation until a successful state refresh and offers a read-only refresh action.
#### Scenario: Candidate association
- **WHEN** another candidate is approved or played, or an offline preview is rendered
- **THEN** the current program candidate is not replaced and its confirm action cannot advance locally.

### Requirement: Four-part editing preserves explicit migration and revocation boundaries
The desktop SHALL offer fixed selling/spec/scenario/after_sales outline fields with the engine's text limits. Legacy outlines SHALL require explicit conversion and publication before program start. Pause/end SHALL request mute, and an unconfirmed mute SHALL not be presented as success. Stale or failed programs SHALL not offer checkbox-based resume or automatic regeneration.
#### Scenario: Existing legacy outline
- **WHEN** the saved outline is not four parts
- **THEN** the desktop explains the conversion and does not silently truncate or change published content.
#### Scenario: Pause during generation
- **WHEN** the operator pauses while a task is pending
- **THEN** mute remains available and late task callbacks cannot update or advance the program display.

### Requirement: Human segment confirmation is durable and bound to playback evidence
人工确认 SHALL 复核本轮精确候选的原生播放报告摘要、独立预期设备帧数、已观测设备尾部、有效审批/事实与当前epoch快照。确认 SHALL 先critical持久保存具名人工复核记录再推进，失败不前进、不重试。该记录 MUST NOT 产生PlaybackAck、机器已完成句、场次记忆或恢复授权。
#### Scenario: Report replacement or storage failure
- **WHEN** 报告缺失/被替换/缺进度，或critical写入失败
- **THEN** 拒绝确认，原段落不前进且不调用Provider。
#### Scenario: Revocation during confirmation
- **WHEN** 持久提交后epoch或权限改变
- **THEN** 保留审计记录但暂停节目，不发下一段，不恢复旧授权。

### Requirement: Recent confirmations survive restart as read-only history
节目响应 SHALL 可展示当前工作区最近256条审计内最新人工复核记录及哈希链引用；重启 SHALL 默认无活跃节目，历史不重建runner/审批/候选或消费生成预算。
#### Scenario: Restart with a saved confirmation
- **WHEN** 同工作区重启引擎并读取节目
- **THEN** 活跃节目为absent，近期人工记录可见，读取不生成语音或自动前进。
