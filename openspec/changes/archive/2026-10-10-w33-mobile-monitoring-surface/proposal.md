## Why

`tests/registry.yaml` 里 **O03**（手机监控包，R1）是 `blocked`，且 notes 至今只有一句话：**「路线 A/B PoC 未定案；Blocked 原因待 registry 标注」**——连阻塞的原因本身都还没写成可核对的条目。

O03 的真值约束确实是外部的：**M0b PoC** 才能定路线 A（中继 + PWA，需有效 HTTPS 域名与证书）还是路线 B（原生壳 + 局域网直连，需应用分发与 Tauri 2 成熟度），真机看板/审批/接管/紧急静音要通过 **S09** 才算数。这些本轮都拿不到。

但 §3.1 L5 与 §10.2 给手机监控面**钉了一组口径**，此前**全库没有任何落点**（`nai-control-api` 是桌面 loopback 控制面，文件头自己写明不做设备认证/预检/投递；`nai-kernel::platform` 管的是平台事件门）。不落，下面几处就会含糊过去：

- **「只发命令、看状态，不持有媒体时钟与密钥」（§3.1 L5）是一句禁止令，不是一句口号。** 如果监控视图靠"把敏感字段挡住"来满足它，那是一条**会被下一次重构漏掉的黑名单**；正确形态是投影**结构上只含白名单字段**，媒体时钟与密钥根本没有落脚处。
- **「它不是手机开播」（§10.2）与「手机只做监控管理」（§742）是同一条边界。** 手机端拿到开播控制就不是"监控包"了。
- **「中继终止 TLS 时可读取业务内容，不宣称端到端加密」（§10.2）是一条诚实性要求。** 这与 W32 的「不以 CUDA EP 注册宣称全 GPU」同形：**声明**越界就是缺陷，而**如实披露**（中继可读、不宣称 E2E）恰恰是合格。
- **「预览按需开启…关闭立即释放编码器与带宽，不得挤占正式出画」（§10.2）四个条件各自可判。** 而且**预览未启用时**这些条件不构成缺陷——「不适用」不等于「不合规」。
- **「路线 A/B 定案」（§663）是外部决定。** 未定案是一个**诚实状态**，不该被伪造成一条已选路线，也不该被当成缺陷；但一旦声明了某路线，它的前置条件（HTTPS 域名与证书 / 应用分发 / Tauri 2 成熟度）就必须逐项成立。
- **「最小形态（看板、审批、接管、紧急静音）」（§10.2、§663）是一个四项清单。** 预览与推送可后置，这四项不可后置。

## What Changes

- **监控视图是白名单投影（`MobileView` / `LiveState` / `Exposure`，O03）**：完整运行状态 `LiveState` 含媒体时钟、Provider 密钥、原始 PCM 与原始帧；跨 L5 边界的 `MobileView` **结构上只含十个白名单字段**，禁止类别（`MediaClock`/`Secret`/`RawAudio`/`RawVideo`/`BroadcastControl`）在它之上没有落脚处。白名单与投影字段集合被断言一致（新增字段而漏登记会使该断言失败），**不靠事后黑名单剔除**。
- **最小形态是四项（`MobileCapability`，O03）**：`Board`/`Approval`/`Takeover`/`EmergencyMute` 为最小形态，`Preview`/`Push` 可后置。缺项逐条指名报出。
- **开着播控制即越界（`Exposure` / `audit_surface`，O03）**：暴露 `BroadcastControl` 报出 `exposes_broadcast_control`（§10.2「它不是手机开播」；§742）。
- **预览四条件仅在启用时受检（`PreviewPolicy`，O03）**：启用时按需、真正编码的低码率、关闭立即释放编码器与带宽、不得挤占正式出画；**未启用时预览策略字段一律不产生缺陷**。
- **中继终止 TLS 不得宣称端到端（`EncryptionClaim`，O03）**：中继终止 TLS 且宣称 E2E → `e2e_encryption_claimed_with_relay_terminated_tls`；宣称 E2E 而协议未经审核 → `e2e_encryption_claimed_without_reviewed_protocol`；**如实声明中继可读、不宣称 E2E 不算缺陷**。
- **路线声明绑定其前置条件（`ConnectionRoute`，O03）**：路线 A 缺 HTTPS 域名或证书 → `relay_pwa_without_secure_context`；路线 B 缺应用分发 → `native_shell_without_distribution`、Tauri 2 成熟度未验证 → `native_shell_tauri_maturity_unverified`。
- **未定案是诚实状态（`SurfaceReadiness`，O03）**：`readiness()` 返回 `AwaitingRouteDecision` / `Defective` / `Ready`；有任一违规即 `Defective` 且**优先于** `AwaitingRouteDecision`——路线未定案不得掩盖缺陷。
- **远程命令带 S09 契约（`RemoteCommand`，O03/S09）**：身份、会话与 epoch、序号、TTL、实际 ACK 五项逐一校验（§86 INV-14、§636 S09）；缺项指名报出。**高危命令不得开放给观察者**（§636「观察者执行高危命令被拒」）。
- **紧急静音可达、启用状态齐备、知识速补走审核流（O03）**：不可达/缺失/绕审逐条报出（§10.2 最小形态、§37.3、§10.2 知识速补「进入审核发布流程」）。
- **审计是描述符的纯函数（`audit_surface`，O03）**：十九条稳定标记，每条都可直接构造；干净描述符审计为空。

## Capabilities
### New Capabilities
- `mobile-monitoring`：§3.1 L5 与 §10.2 手机监控面的判定口径——白名单投影（不持有媒体时钟与密钥）、最小形态四项、非开播通道、预览四条件（仅在启用时受检）、中继 TLS 不宣称 E2E、路线 A/B 前置与「未定案是诚实状态」、S09 远程命令契约与高危不得开放给观察者、紧急静音可达与启用状态、知识速补走审核流、描述符纯函数审计。覆盖 registry **O03**（并回指 **S09**）。

### Modified Capabilities
- 无。既有 `platform` 模块（内核内）管的是**平台事件门**（官方事件来源、房间、权限快照、连接代次），本变更管的是**监控面的越界面**，两者不重叠也不改前者。既有 `control.manage@1` / S09 的 loopback 控制面**不改**：本变更只**引用**其命令契约（INV-14），不实现网络传输。

## Impact

- 代码：`crates/nai-kernel/src/` 新增 `monitoring.rs`；`tests/` 新增 `monitoring.rs`；`lib.rs` 增一个 `pub mod`。复用 `golden::Finding`（`Finding::new` 已为 `pub`），**不新增令牌类型**。**不新增 crate，不改 workspace 依赖**（维持 18 包与依赖方向；kernel 仍只依赖 contracts）。
- 规格：新增 capability `mobile-monitoring`；新增 `docs/mobile-monitoring.md`、ADR 0035、报告 `docs/reports/2026-10-10-w33-mobile-monitoring-surface.md`。
- 依赖：**零新增**。仅用 `std`。
- registry：**O03 保持 `blocked`**（路线 A/B 定案 + 真机最小形态通过 S09 是外部真值），notes 补记 W33 已交付判定机制、阻塞边界，并**补齐原先缺失的 blocked 原因**；**S09 保持 `tested`**，notes 回指本变更引用了其命令契约。不新增编号（registry id 正则为恰好两位数字）。
- 明确不做：不实现任何移动端应用（PWA 或原生壳）、不做证书申请、不做分发、不接真机、不接中继；不做 UI；不改 `platform.rs`；不引入 `check --profile portable` 的新命令（§30.1 只为 RAG 黄金集指定 `tests/rag-golden`，为监控面臆造证据目录属过度绑定）。

## Non-Goals

- 不用机制替代真实交付：本轮证明的是「越界面可判定、未定案不被读成缺陷、如实披露不被读成违规、预览未启用不等于不合规」，**不宣称手机监控包已交付**。O03 保持 `blocked`。
- 不替 M0b 做路线决策：路线 A/B 二选一是外部 PoC 的结论，本变更只校验**已声明路线的前置条件**。
- 不把「未定案」变成缺陷，也不让「未定案」掩盖缺陷。
- 不引入传输层：命令契约的**字段齐备性**可判，真实防重放/ACK 的网络行为属 S09 的集成验收。
