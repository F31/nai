# 手机监控面的判定骨架（O03）

代码：`crates/nai-kernel/src/monitoring.rs`；测试：`crates/nai-kernel/tests/monitoring.rs`；
决策：[ADR 0035](adr/0035-mobile-monitoring-surface-boundary.md)。

本模块把 §3.1 L5 与 §10.2「手机监控包」的**判定口径**落成在 portable 下可执行、可测试的骨架。它**不产生**任何移动端应用、证书、分发渠道或 S09 真机集成——那些由外部条件（M0b PoC 定路线 + 真机最小形态通过 S09）提供；它负责的是「给定一份**声明的监控面**，它是否越过 L5 与 §10.2 的边界、未定案的路线如何表达、如实披露与虚假声明如何区分」。

因此 `tests/registry.yaml` 的 **O03 保持 `blocked`**：§663 的验收原文是「路线 A/B 定案；最小形态（看板、审批、接管、紧急静音）通过 S09」，这些是外部真值。本文档描述的是**判定机制**，不是交付结论。

## 白名单投影：L5 不持有媒体时钟与密钥

§3.1 原文：L5 体验层（桌面控制台、**手机监控包**、团队控制台）**只发命令、看状态，不持有媒体时钟与密钥**。

落法不是"把敏感字段挡住"，而是**投影结构上只含白名单字段**：

| 类型 | 作用 |
| --- | --- |
| `LiveState` | 完整运行状态：十个业务字段 **加上** `media_clock`、`secret`、`raw_audio`、`raw_video`、`broadcast_control` |
| `MobileView` | 跨 L5 边界的投影，**只有十个白名单字段**——媒体时钟与密钥在它之上没有落脚处 |
| `Exposure::{ALLOWED, FORBIDDEN}` | 白名单十项与禁止五项的**同一份词汇表** |

两条断言把它钉成结构性质，而不是一句承诺：

- `MobileView::field_names()` 的集合**等于** `Exposure::ALLOWED` 的标签集合——**给视图加字段而漏登记即测试失败**；
- `LiveState::sensitive_field_names()` 与 `MobileView::field_names()` **不相交**，且前者等于 `Exposure::FORBIDDEN` 的标签。

黑名单方案的失效方式是**加字段时默认流出**（例如日后给 `LiveState` 添一个 token）；白名单方案的失效方式是**加字段时忘了登记**，后者会被测试当场抓住。

## 最小形态是四项，预览与推送可后置

§10.2「最小形态：看板、审批、接管、紧急静音；预览与推送可后置」；§663 同。`MobileCapability::MINIMUM_FORM` 恰为这四项，缺项**逐条指名**报出（`minimum_form_incomplete`）。**预览/推送未交付不构成缺陷**。

## 它不是手机开播

§10.2「**它不是手机开播**」；§742「手机只做监控管理」。开播控制建为 `Exposure::BroadcastControl`——与媒体时钟、密钥同族的**禁止类别**，声明即报出 `exposes_broadcast_control`。这样"可以看状态"、"可以下发命令"、"不能开播"三件事落在同一份词汇表上。

## 预览四条件**仅在启用时受检**

§10.2「预览按需开启，用真正编码的低码率流；关闭立即释放编码器与带宽，不得挤占正式出画」：

| 条件 | 字段 | 违反时 |
| --- | --- | --- |
| 按需开启 | `on_demand` | `preview_not_on_demand` |
| 真正编码的低码率 | `truly_encoded_low_bitrate` | `preview_not_encoded` |
| 关闭立即释放编码器与带宽 | `releases_encoder_and_bandwidth_on_close` | `preview_does_not_release_on_close` |
| 不得挤占正式出画 | `may_preempt_official_output = false` | `preview_may_preempt_output` |

**`enabled = false` 时这些字段一律不产生缺陷**——§10.2 允许预览后置，对未启用的特性报"不是按需开启"是把**不适用**读成**不合规**。`PreviewPolicy` 的 `Default` 正是这个"已后置"的状态。

## 中继终止 TLS 不得宣称端到端

§10.2「中继终止 TLS 时可读取业务内容，**不宣称端到端加密**；需要时使用经审核的端到端协议」。两条独立规则：

- `relay_terminates_tls && claims_end_to_end` → `e2e_encryption_claimed_with_relay_terminated_tls`；
- `claims_end_to_end && !protocol_reviewed` → `e2e_encryption_claimed_without_reviewed_protocol`；
- **如实披露**（`relay_terminates_tls = true && claims_end_to_end = false`）**不算缺陷**。

这与 W32「不以 CUDA EP 注册宣称全 GPU」同形：**声明**越界即缺陷，**如实记录**恰恰合格。

## 路线 A/B：前置条件与「未定案是诚实状态」

§10.2 连接路线（M0b PoC 后定案）：

| 路线 | 前置条件 | 缺失时报出 |
| --- | --- | --- |
| A 中继 + PWA | 有效 HTTPS 域名 + 证书（Service Worker 需安全上下文） | `relay_pwa_without_secure_context` |
| B 原生壳 + 局域网直连 | 应用分发渠道 | `native_shell_without_distribution` |
| B | Tauri 2 移动端成熟度已验证 | `native_shell_tauri_maturity_unverified` |

`MobileSurface::readiness()` 返回三态：

| 条件（按序） | `SurfaceReadiness` |
| --- | --- |
| 审计非空（任一违规） | `Defective` |
| 路线为 `Undecided` | `AwaitingRouteDecision` |
| 已声明路线且无违规 | `Ready` |

**`Defective` 优先于 `AwaitingRouteDecision`**——路线未定案不得掩盖越界；而 `AwaitingRouteDecision` 本身**不是缺陷**，它是 O03 现在**诚实**的阻塞状态。

## 命令契约与角色（回指 S09）

§86 INV-14 / §636 S09：远程命令带**身份、会话与 epoch、序号、TTL**，服务端去重防重放，返回**实际执行 ACK**；**观察者执行高危命令被拒**。

- `RemoteCommand` 五项字段逐一校验，缺项 → `command_contract_incomplete` 并**指名该命令**；
- `risk == High && !min_role.at_least(Operator)` → `high_risk_command_open_to_observer` 并指名；
- 本轮可判的是**描述符里字段是否齐备**与**最低角色声明**；真实防重放、去重、ACK 语义与吊销即时生效属 S09 的**集成**验收（已在 loopback 面 `tested`），不在本模块。

## 运营项

| 要求 | 出处 | 违反时 |
| --- | --- | --- |
| 紧急静音可达（能力已声明却不可达） | §10.2 最小形态、§37.3 紧急停用 | `emergency_mute_unreachable` |
| 提供启用状态 | §37.3 手机端提供启用状态 | `enable_status_missing` |
| 知识速补进入审核发布流程 | §10.2 | `knowledge_patch_without_review_flow` |

知识速补一项**只在声明了 `KnowledgePatch` 能力时**才判——未声明该能力而"未接审核流"不是缺陷。

## 审计是描述符的纯函数

`audit_surface(&MobileSurface)` 对声明字段做纯函数，报出**全部**命中项，使每条规则都能被直接构造：

| token | 含义 |
| --- | --- |
| `exposes_media_clock` | 暴露媒体时钟 |
| `exposes_secret` | 暴露密钥/凭据 |
| `exposes_raw_media` | 暴露未编码的原始音频或视频 |
| `exposes_broadcast_control` | 暴露开播控制（它是监控包，不是开播通道） |
| `minimum_form_incomplete` | 最小形态缺项（逐项指名） |
| `emergency_mute_unreachable` | 紧急静音不可达 |
| `enable_status_missing` | 未提供启用状态 |
| `knowledge_patch_without_review_flow` | 知识速补未接入审核发布流程 |
| `preview_not_on_demand` | 预览不是按需开启 |
| `preview_not_encoded` | 预览流不是真正编码的低码率 |
| `preview_does_not_release_on_close` | 预览关闭未释放编码器与带宽 |
| `preview_may_preempt_output` | 预览可能挤占正式出画 |
| `relay_pwa_without_secure_context` | 路线 A 缺 HTTPS 域名或证书 |
| `native_shell_without_distribution` | 路线 B 缺应用分发 |
| `native_shell_tauri_maturity_unverified` | 路线 B 的 Tauri 2 成熟度未验证 |
| `e2e_encryption_claimed_with_relay_terminated_tls` | 中继终止 TLS 却宣称端到端 |
| `e2e_encryption_claimed_without_reviewed_protocol` | 宣称端到端而协议未经审核 |
| `command_contract_incomplete` | 命令契约缺字段（指名命令） |
| `high_risk_command_open_to_observer` | 高危命令开放给观察者（指名命令） |

`every_finding_token_is_constructible` 断言这**十九条**恰好等于三份构造描述符的审计并集——既保证每条可构造，也保证没有未被登记的额外标记。

## 未做边界

- **不含任何真实交付**：移动端应用（PWA 或原生壳）、证书、分发渠道、中继、真机与 S09 真机集成都由外部提供。测试里的描述符是**构造出来的**，只为证明越界口径与 readiness 派生可执行。
- **不替 M0b 做路线决策**：路线 A/B 二选一是外部 PoC 的结论，本模块只校验**已声明**路线的前置条件。
- **不引入传输层**：命令契约的字段齐备性可判；真实防重放、去重、ACK 与吊销即时生效属 S09 的集成验收。
- **不新增 xtask 命令与证据目录**：§30.1 为 UI 层指定 `tests/ui`、为 RAG 黄金集指定 `tests/rag-golden`；手机监控面的真值来自**真机与 PoC**，不是仓库载荷。**臆造** `tests/mobile-monitoring` 属过度绑定。待 M0b 定案并立项真机交付后再补载体，**无需改本模块判定**。
- **不改 `platform` 模块**：`nai-kernel::platform` 管**官方事件门**（事件来源、房间、权限快照、连接代次），本模块管**监控面的越界面**，两者不重叠。
- 注册表 **O03 保持 `blocked`**：判定机制（W33）已交付并在 notes 中保留；**S09 保持 `tested`**。
