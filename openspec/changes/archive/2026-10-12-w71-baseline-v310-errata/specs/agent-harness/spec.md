# agent-harness 变更（w71）

## MODIFIED Requirements

### Requirement: Harness layers fail with named degradations and are observable

harness 每层 SHALL 产出可回放记录并进入只追加的哈希链决策日志：装配（片段来源与预算、裁剪与原因）、
意图（枚举、置信度、依据、与守门的冲突）、记忆（读写键、作用域、命中、删除完成状态）、工具（每步技能 ID 与版本、
参数哈希、结果摘要与大小、终止原因、成本）、缓存（命中与失效原因）。所有记录 MUST NOT 含明文凭据。
每层失败 MUST 有具名错误码与降级路径，并落到既有的降级顺序（模板 → 已批准缓存 → 人工 → 静音待机），
MUST NOT 静默降级为「空上下文的自由发挥」；降级 MUST 在能力视图与界面可见。
harness 新机制 MUST 默认只在**受监督自动以外的阶段**启用（默认启用集合按爬坡**位置**判定，MUST NOT 按阶段名枚举），
进入受监督自动前 MUST 通过**相关**黄金集回归。

#### Scenario: Assembly fails at runtime

- **WHEN** 装配层失败（例如不可裁剪段缺失）
- **THEN** 拒绝调用并退到模板，且在界面可见该降级；MUST NOT 生成无策略约束的回答

#### Scenario: A silent degradation happens

- **WHEN** 任一层的失败没有产生具名错误码或没有在能力视图体现
- **THEN** 视为不满足本需求

#### Scenario: A new harness mechanism is enabled in auto mode

- **WHEN** 某 harness 机制（装配、意图、循环、缓存的新行为）首次进入受监督自动模式
- **THEN** MUST 先通过相关黄金集回归，并在决策日志留下启用记录

### Requirement: Harness mechanisms ramp through stages, and supervised automatic mode is gated

harness 机制 SHALL 按 §5.9／§47.3 的四段爬坡（演练 → 影子 → 人工辅助 → 受监督自动）推进；
**默认启用集合 MUST 按爬坡中的位置判定**——受监督自动**之前**的全部阶段默认启用，受监督自动是唯一被门控的阶段。
该集合 MUST NOT 以**枚举阶段名**的方式表述：某条发布线未包含的阶段（如 R1 的影子）即**不存在**，
MUST NOT 被读作缺陷，也 MUST NOT 为对齐枚举而新增该阶段。
晋升到受监督自动 MUST 同时满足：相关回归已通过（MUST 声明受影响的类别、这些类别均已通过、且回归结论绑定**当前** harness 修订），
以及启用记录已备（指名同一机制与目标阶段、值内无明文凭据）。
任一条不满足 MUST **具名拒绝**并列出全部缺失项。判定的是「这次晋升是否被允许」；按阶段真正启用机制、
以及把启用记录追加进只追加哈希链，是 runtime 接线，MUST NOT 被读作已完成。

#### Scenario: A new mechanism arrives

- **WHEN** 一个 harness 新机制被引入
- **THEN** 它 MUST 只在受监督自动**之前**的阶段启用，MUST NOT 默认进入受监督自动

#### Scenario: A ramp stage has no contract landing point

- **WHEN** 询问各爬坡阶段对应的会话模式落点
- **THEN** 演练、人工辅助与受监督自动各有落点，而**影子在首版门槛（R1）内没有落点** MUST 被如实暴露，
  MUST NOT 被假装为已有；同时按本需求，**该缺失 MUST NOT 被读作缺陷**——默认启用集合由爬坡**位置**判定，
  不由阶段名枚举，故 R1 内不需要为影子新增 `SessionMode` 取值，也不需要把该阶段从 §5.9 的四段里删掉

#### Scenario: Promotion without the relevant regression

- **WHEN** 晋升到受监督自动，但受影响的回归类别未全部通过，或根本未声明受影响的类别
- **THEN** 具名拒绝并列出**全部**缺失类别，MUST NOT 只报第一个，也 MUST NOT 让「未声明」被读成「无相关回归」

#### Scenario: The harness changed after the regression passed

- **WHEN** 回归通过后 harness 修订发生变化（提示词、装配顺序、阈值、路由或技能清单）
- **THEN** 该回归不再使晋升生效并具名拒绝，MUST NOT 以「曾经通过过」放行

#### Scenario: Promotion without an enablement record

- **WHEN** 晋升到受监督自动却没有启用记录，或记录指名了别的机制或别的阶段
- **THEN** 具名拒绝；允许时 MUST 产出一条可落日志的启用记录载荷（含机制与目标阶段），
  MUST NOT 只返回「允许」而不产出记录

#### Scenario: The enablement record carries a credential

- **WHEN** 启用记录的**值**内出现凭据字面量
- **THEN** 具名拒绝；字段名含 `key` 而其值干净的记录 MUST 被放行——按字段名黑名单拒绝的实现不满足本需求
