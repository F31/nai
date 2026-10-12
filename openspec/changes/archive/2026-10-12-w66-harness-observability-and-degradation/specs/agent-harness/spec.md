## ADDED Requirements

### Requirement: Harness observations are decidable before they reach the log

harness 的五类观测（装配／意图／记忆／工具／缓存）SHALL 各有一组固定必需记录项，取自 §47.1 且不增不减。
一条记录进入只追加的哈希链决策日志之前 MUST 先被判定：必需项齐备、值内不含明文凭据（INV-11）、
观众原文与昵称已按 §5.8 保留期脱敏。任一项不满足 MUST **具名拒绝**并给出可落日志的已脱敏载荷，
MUST NOT 原样透传，也 MUST NOT 凭字段名猜测敏感性。五类 MUST 恰为五类，缺任一类 MUST NOT 被读作齐全。
判定的是「能否进日志与以何形态进」；**真正追加进哈希链是 runtime 接线**，本需求 MUST NOT 被读作已完成。

#### Scenario: A record carries a plaintext credential

- **WHEN** 某条观测记录的**值**内出现凭据字面量（如 `sk-…`）
- **THEN** 该记录被具名拒绝且不可落日志，MUST NOT 以「字段名不含 key」为由放行

#### Scenario: Only a field name looks sensitive

- **WHEN** 某字段名含 `key` 但其值不含任何凭据字面量
- **THEN** 该记录 MUST 被放行；按字段名黑名单拒绝的实现不满足本需求

#### Scenario: A required field is missing

- **WHEN** 某一层记录缺少 §47.1 为该层规定的必需项之一
- **THEN** 该记录被具名拒绝并指名缺失项，MUST NOT 以空值补齐

#### Scenario: Viewer text outlives its retention

- **WHEN** 观众原文或昵称未脱敏，或其保留期已过仍在记录中
- **THEN** 分别具名拒绝，MUST NOT 让「说不清」被读成「已脱敏」

### Requirement: Every harness layer failure lands on a named, ordered and visible degradation

harness 每层失败 SHALL 落到 §12.2 降级顺序（模板 → 已批准缓存 → 人工 → 静音待机）中的确定一级：
先在 §47.4 指定的首选级起取第一个可用级，静音待机恒可用。每层 MUST 有互异的具名错误码，
降级 MUST 在能力视图与界面可见（**不存在不可见的降级**）。装配层失败 MUST 拒绝该次调用，
且不因落点变化而解除拒绝；MCP/A2A 对端不可用 MUST NOT 影响已有会话。
当本次调用存在任一降级且装配后上下文失去策略或人设约束时，MUST 判为违规并 MUST NOT 继续生成（INV-32）。

#### Scenario: Assembly fails while a template is available

- **WHEN** 装配层失败（不可裁剪段缺失）且模板可用
- **THEN** 落**模板**级并**拒绝该次调用**；MUST NOT 生成无策略约束的回答

#### Scenario: Intent or cache fails while the template is available

- **WHEN** 意图不可用或低置信，或缓存不可用／失效，且模板与人工均可用
- **THEN** 落**人工**级；MUST NOT 沿用旧缓存引用，也 MUST NOT 因为模板可用就落模板

#### Scenario: The preferred level is unavailable

- **WHEN** 工具失败且模板不可用、人工可用
- **THEN** 落**人工**级；全部不可用时落**静音待机**；把一切映射到同一级的实现不满足本需求

#### Scenario: A degraded call would generate free-form

- **WHEN** 本次调用存在任一降级，且上下文已失去策略或人设约束
- **THEN** 判为具名违规并 MUST NOT 继续生成；仅给出错误码却仍继续生成同样不满足
