# mobile-monitoring Specification

## Purpose
把 §3.1 L5 与 §10.2「手机监控包」的**判定口径**落成在 portable 下可执行、可测试的骨架：监控面只发命令、看状态，不持有媒体时钟与密钥；最小形态是看板、审批、接管、紧急静音；它不是手机开播；预览按需开启且不得挤占正式出画；中继终止 TLS 时不得宣称端到端加密；连接路线 A/B 未定案是一个**诚实状态**而非缺陷。本能力只判定**一份声明的监控面是否越界**，不产生任何真实移动端应用、分发、证书或真机测量；这些真值由外部条件提供，未提供时 readiness 保持 `AwaitingRouteDecision`。

## Requirements

### Requirement: The L5 surface carries state and commands only
手机监控面 SHALL 只发送命令、查看状态，MUST NOT 取得或转出**媒体时钟**、**密钥/凭据**、**未编码的原始音视频**与**开播控制**。状态投影 MUST 由白名单字段构成（结构上不含上述类别），MUST NOT 依赖一份手工维护的黑名单来事后剔除。

#### Scenario: The projection cannot leak a secret
- **WHEN** 把一份含媒体时钟、Provider 密钥、原始 PCM 与原始帧的完整运行状态投影为监控视图
- **THEN** 投影结果的字段集合不含任何禁止类别，且与白名单一致

#### Scenario: A forbidden exposure is reported
- **WHEN** 声明的监控面暴露媒体时钟（或密钥、原始音视频、开播控制）
- **THEN** 审计按类别逐项报出稳定标记

### Requirement: The minimum form is four capabilities
手机监控包 SHALL 至少交付**看板、审批、接管、紧急静音**四项；预览与推送 MAY 后置。缺少最小形态中任一项 MUST 被审计报出，并逐项指名缺失的能力。

#### Scenario: Emergency mute absent
- **WHEN** 声明的监控面具备看板、审批与接管，但缺紧急静音
- **THEN** 审计报出最小形态不完整并指名紧急静音

#### Scenario: Preview deferred is not a missing capability
- **WHEN** 四项最小形态齐备，预览与推送后置
- **THEN** 最小形态判定为完整，不因预览后置而报出缺陷

### Requirement: The monitoring package is not a broadcast channel
手机监控包 MUST NOT 作为开播通道，也 MUST NOT 暴露开播控制（§10.2「它不是手机开播」；§742 手机只做监控管理）。

#### Scenario: Broadcast control declared
- **WHEN** 声明的监控面包含开播控制
- **THEN** 审计报出 `exposes_broadcast_control`

### Requirement: Preview is on-demand, encoded, releasing and non-preempting
启用预览时，预览 SHALL 按需开启、使用真正编码的低码率流、关闭时立即释放编码器与带宽，且 MUST NOT 挤占正式出画。预览**未启用**时，预览策略各字段 MUST NOT 产生缺陷。

#### Scenario: Preview always on
- **WHEN** 预览启用但不是按需开启
- **THEN** 审计报出 `preview_not_on_demand`

#### Scenario: Preview may preempt the official output
- **WHEN** 预览启用且声明可能挤占正式出画
- **THEN** 审计报出 `preview_may_preempt_output`

#### Scenario: Preview not enabled
- **WHEN** 预览未启用但预览策略字段不合规
- **THEN** 审计不报出任何预览缺陷

### Requirement: Relay-terminated TLS is not end-to-end encryption
中继终止 TLS 时中继可读取业务内容，声明端到端加密 MUST 被审计报出。如实声明「中继可读取、不宣称端到端」MUST NOT 被判为缺陷；声明端到端加密而所用协议未经审核 MUST 被审计报出。

#### Scenario: End-to-end claimed on a relay that terminates TLS
- **WHEN** 声明中继终止 TLS 且同时宣称端到端加密
- **THEN** 审计报出 `e2e_encryption_claimed_with_relay_terminated_tls`

#### Scenario: Honest relay disclosure
- **WHEN** 声明中继终止 TLS 且不宣称端到端加密
- **THEN** 审计为空，该组合不构成缺陷

#### Scenario: End-to-end claimed without a reviewed protocol
- **WHEN** 宣称端到端加密但所用协议未经审核
- **THEN** 审计报出 `e2e_encryption_claimed_without_reviewed_protocol`

### Requirement: Route prerequisites are recorded with the decision
连接路线 SHALL 在 M0b PoC 后定案。路线 A（中继 + PWA）MUST 具备有效 HTTPS 域名与证书（PWA Service Worker 需安全上下文）；路线 B（原生壳 + 局域网直连 + 证书固定）MUST 具备应用分发渠道且 Tauri 2 移动端成熟度已验证。声明某路线而缺其前置条件 MUST 被审计逐项报出。

#### Scenario: Route A without a secure context
- **WHEN** 声明路线 A 但缺有效 HTTPS 域名或证书
- **THEN** 审计报出 `relay_pwa_without_secure_context`

#### Scenario: Route B without distribution
- **WHEN** 声明路线 B 但缺应用分发渠道
- **THEN** 审计报出 `native_shell_without_distribution`

#### Scenario: Route B with unverified Tauri maturity
- **WHEN** 声明路线 B 而 Tauri 2 移动端成熟度未验证
- **THEN** 审计报出 `native_shell_tauri_maturity_unverified`

### Requirement: An undecided route is an honest state, not a defect
路线未定案时，readiness MUST 为 `AwaitingRouteDecision`，MUST NOT 记为缺陷。已声明路线且无任何违规时 readiness 为 `Ready`；存在任一违规时 readiness MUST 为 `Defective`，且该判定 MUST 优先于 `AwaitingRouteDecision`。

#### Scenario: Undecided but otherwise clean
- **WHEN** 路线未定案，且最小形态齐备、无禁止暴露、无契约缺陷
- **THEN** readiness 为 `AwaitingRouteDecision` 且审计为空

#### Scenario: Defect outranks the undecided route
- **WHEN** 路线未定案，但声明的监控面暴露媒体时钟
- **THEN** readiness 为 `Defective`，不得因路线未定案而掩盖该缺陷

#### Scenario: A decided route with prerequisites met
- **WHEN** 路线 A 具备 HTTPS 域名与证书，且无其它违规
- **THEN** readiness 为 `Ready`

### Requirement: Remote commands carry the S09 contract and never open high-risk to observers
经监控面下发的远程命令 SHALL 携带身份、会话与 epoch、序号、TTL 并返回实际执行 ACK（§86 INV-14、§636 S09）。任一缺失 MUST 被审计报出并指名该命令。高危命令 MUST NOT 允许观察者角色执行。

#### Scenario: An incomplete command contract
- **WHEN** 某远程命令缺序号
- **THEN** 审计报出 `command_contract_incomplete` 并指名该命令

#### Scenario: High-risk command open to an observer
- **WHEN** 某高危命令的最低角色被声明为观察者
- **THEN** 审计报出 `high_risk_command_open_to_observer` 并指名该命令

#### Scenario: High-risk command requires the operator role
- **WHEN** 高危命令要求操作员角色且契约完整
- **THEN** 不报出命令相关缺陷

### Requirement: Emergency mute is reachable and enable status is provided
监控面 SHALL 使紧急静音可达，并提供启用状态（§10.2 最小形态；§37.3 手机端提供启用状态与紧急停用）。不可达或缺失 MUST 被审计逐项报出。知识速补 SHALL 进入审核发布流程；未接入该流程即声称为可用 MUST 被审计报出。

#### Scenario: Emergency mute unreachable
- **WHEN** 声明的监控面含紧急静音能力但不可达
- **THEN** 审计报出 `emergency_mute_unreachable`

#### Scenario: Enable status absent
- **WHEN** 声明的监控面不提供启用状态
- **THEN** 审计报出 `enable_status_missing`

#### Scenario: Knowledge patch bypasses review
- **WHEN** 声明知识速补可用但未接入审核发布流程
- **THEN** 审计报出 `knowledge_patch_without_review_flow`

### Requirement: Surface audit is a pure function of the descriptor
审计 SHALL 是对所声明监控面字段的**纯函数**，报出全部命中项，使每条规则都能被直接构造与验证。

#### Scenario: A clean surface audits empty
- **WHEN** 直接构造一份满足全部约束的监控面描述
- **THEN** 审计为空

#### Scenario: Every finding is constructible
- **WHEN** 直接构造一份含某违规的描述
- **THEN** 审计报出对应的稳定标记，无需经由任何运行时测量
