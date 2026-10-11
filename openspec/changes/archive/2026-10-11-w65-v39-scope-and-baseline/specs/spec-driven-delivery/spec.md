# spec-driven-delivery 变更（w65）

## MODIFIED Requirements

### Requirement: Specification before implementation

开发流程 SHALL 在实现每轮 Dxx 任务前创建 OpenSpec proposal、需求场景、design 和 tasks，引用**当前设计基线**（`docs/design.md` 指明的版本）及相关 ADR；基线变更必须有 ADR，OpenSpec 不得隐式覆盖基线。

#### Scenario: Start a development task
- **WHEN** 开始一个包含功能或工程行为变化的任务
- **THEN** 存在可校验的变更规格及未勾选的实现/验证清单，再开始编码

#### Scenario: A specification pins a stale baseline version
- **WHEN** 某规格把某一具体历史版本（如 V3.5）写成「统一设计基线」，而该版本已被后续 ADR 取代
- **THEN** 该陈述 MUST 被就地订正为**不写死版本号**的形式（以 `docs/design.md` 与最新基线 ADR 为准）；
  「纯追加」MUST NOT 被用来让这类陈述长期存续
