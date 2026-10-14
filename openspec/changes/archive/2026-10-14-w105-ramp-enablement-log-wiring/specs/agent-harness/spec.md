## MODIFIED Requirements

### Requirement: Harness mechanisms ramp through stages, and supervised automatic mode is gated

harness 机制 SHALL 按 §5.9／§47.3 的四段爬坡（演练 → 影子 → 人工辅助 → 受监督自动）推进；
**默认启用集合 MUST 按爬坡中的位置判定**——受监督自动**之前**的全部阶段默认启用，受监督自动是唯一被门控的阶段。
该集合 MUST NOT 以**枚举阶段名**的方式表述：某条发布线未包含的阶段（如 R1 的影子）即**不存在**，
MUST NOT 被读作缺陷，也 MUST NOT 为对齐枚举而新增该阶段。
晋升到受监督自动 MUST 同时满足：相关回归已通过（MUST 声明受影响的类别、这些类别均已通过、且回归结论绑定**当前** harness 修订），
以及启用记录已备（指名同一机制与目标阶段、值内无明文凭据）。
任一条不满足 MUST **具名拒绝**并列出全部缺失项。判定的是「这次晋升是否被允许」；
把放行的启用记录追加进只追加哈希链由 `runtime-composition` 承担，本需求 MUST NOT 被读作「已落链」。

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
