## ADDED Requirements

### Requirement: Harness mechanisms ramp through stages, and supervised automatic mode is gated

harness 机制 SHALL 按 §5.9／§47.3 的四段爬坡（演练 → 影子 → 人工辅助 → 受监督自动）推进；**前三段默认启用**，
受监督自动是唯一被门控的阶段。晋升到受监督自动 MUST 同时满足：相关回归已通过（MUST 声明受影响的类别、
这些类别均已通过、且回归结论绑定**当前** harness 修订），以及启用记录已备（指名同一机制与目标阶段、值内无明文凭据）。
任一条不满足 MUST **具名拒绝**并列出全部缺失项。判定的是「这次晋升是否被允许」；按阶段真正启用机制、
以及把启用记录追加进只追加哈希链，是 runtime 接线，MUST NOT 被读作已完成。

#### Scenario: A new mechanism arrives

- **WHEN** 一个 harness 新机制被引入
- **THEN** 它 MUST 只在演练、影子与人工辅助下启用，MUST NOT 默认进入受监督自动

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

#### Scenario: A ramp stage has no contract landing point

- **WHEN** 询问各爬坡阶段对应的会话模式落点
- **THEN** 演练、人工辅助与受监督自动各有落点，而**影子在首版门槛内没有落点** MUST 被如实暴露，
  MUST NOT 被假装为已有
