## 1. 立项与骨架

- [x] 1.1 OpenSpec 变更 `w33-mobile-monitoring-surface`（proposal/design/tasks + `mobile-monitoring` 增量），`validate --strict` 通过。
- [x] 1.2 落点确认：放进 `crates/nai-kernel`（`monitoring.rs`；kernel 只允许依赖 contracts），不放进 `nai-control-api`（传输层）、不放进 `nai-media-core`（非决策层）、不新开 crate——18 包与依赖方向不变；复用 `golden::Finding`（报告词汇），**不新增令牌类型、不复制实现、零新增依赖**。

## 2. 白名单投影与暴露词汇（O03）

- [x] 2.1 `Exposure`：十个白名单变体（`Mode`/`CurrentUtterance`/`PendingSeconds`/`EventSource`/`ApiStatus`/`BudgetStatus`/`DeviceStatus`/`PendingApprovals`/`Alerts`/`EnableStatus`）与五个禁止变体（`MediaClock`/`Secret`/`RawAudio`/`RawVideo`/`BroadcastControl`）；`ALL`/`ALLOWED`/`FORBIDDEN`、`label()`、`is_forbidden()`。
- [x] 2.2 `LiveState`（含 `media_clock_ms`/`provider_secret`/`raw_audio_pcm`/`raw_video_frames`/`broadcast_control` 等）与 `MobileView`（**结构上只含十个白名单字段**）；`MobileView::project(&LiveState)`、`MobileView::field_names()`、`MobileView::exposed()`。
- [x] 2.3 不变量：`Exposure::ALL` = `ALLOWED` ∪ `FORBIDDEN`、互不相交；`MobileView::field_names()` 的集合恰等于 `Exposure::ALLOWED` 的标签集合；`LiveState` 的禁止字段名与投影字段集合不相交。
- [x] 2.4 测试：投影结果为白名单；带密钥与媒体时钟的状态投影后禁止类别仍不出现；暴露媒体时钟/密钥/原始音视频/开播控制逐条被审计报出。

## 3. 最小形态与功能清单（O03）

- [x] 3.1 `MobileCapability` 九枚举与 `MINIMUM_FORM`（`Board`/`Approval`/`Takeover`/`EmergencyMute`）；`label()`。
- [x] 3.2 `MobileSurface::minimum_form_complete()` / `missing_minimum()`；审计 `minimum_form_incomplete` 逐条指名缺失能力。
- [x] 3.3 测试：缺紧急静音 → 报出并指名；四项齐备而预览/推送后置 → 最小形态完整、无缺陷。

## 4. 预览、加密与路线（O03）

- [x] 4.1 `PreviewPolicy{enabled, on_demand, truly_encoded_low_bitrate, releases_encoder_and_bandwidth_on_close, may_preempt_official_output}`；**仅在 `enabled` 时**报出 `preview_not_on_demand`/`preview_not_encoded`/`preview_does_not_release_on_close`/`preview_may_preempt_output`。
- [x] 4.2 `EncryptionClaim{relay_terminates_tls, claims_end_to_end, protocol_reviewed}`；报出 `e2e_encryption_claimed_with_relay_terminated_tls`（中继终止 TLS 且宣称 E2E）与 `e2e_encryption_claimed_without_reviewed_protocol`。
- [x] 4.3 `ConnectionRoute::{Undecided, RelayPwa{https_domain, certificate}, NativeShellLan{app_distribution, tauri_maturity_verified}}`；报出 `relay_pwa_without_secure_context`/`native_shell_without_distribution`/`native_shell_tauri_maturity_unverified`。
- [x] 4.4 测试：预览启用时四条件逐条可命中、未启用时四字段一律不产生缺陷；中继终止 TLS 且宣称 E2E → 报出、如实披露 → 审计为空、宣称 E2E 而协议未审核 → 报出；路线 A 缺域名或证书 → 报出、路线 B 缺分发或成熟度未验证 → 报出。

## 5. readiness 三态（O03）

- [x] 5.1 `SurfaceReadiness::{AwaitingRouteDecision, Defective, Ready}`；`MobileSurface::readiness()` 先取审计（非空即 `Defective`），再按路线（`Undecided` → `AwaitingRouteDecision`）否则 `Ready`。
- [x] 5.2 测试：干净且未定案 → `AwaitingRouteDecision`；未定案但有越界 → `Defective`（优先）；路线 A 前置齐备且无违规 → `Ready`。

## 6. 命令契约与运营项（O03 / S09）

- [x] 6.1 `CommandRisk`/`Role`（`at_least`）、`RemoteCommand{name, risk, min_role, carries_identity, carries_session_epoch, carries_sequence, carries_ttl, returns_actual_ack}`、`contract_complete()`；审计 `command_contract_incomplete` 与 `high_risk_command_open_to_observer`（均指名命令）。
- [x] 6.2 运营项：`emergency_mute_reachable`、`enable_status_provided`、`knowledge_patch_routes_to_review`；审计 `emergency_mute_unreachable`/`enable_status_missing`/`knowledge_patch_without_review_flow`（后者仅在声明 `KnowledgePatch` 能力时判）。
- [x] 6.3 测试：命令五项缺失逐条可命中并指名；高危且最低角色为观察者 → 报出，高危要求操作员 → 无缺陷；紧急静音不可达/启用状态缺失/知识速补绕审逐条报出；未声明知识速补时不报该项。

## 7. 审计与文档、提交（O03）

- [x] 7.1 `audit_surface(&MobileSurface) -> Vec<Finding>` 报出全部命中；十九条稳定标记逐一可在测试里直接构造；干净描述符审计为空。
- [x] 7.2 新增 `docs/mobile-monitoring.md`（白名单投影、最小形态、非开播通道、预览条件化受检、中继 TLS 诚实性、路线前置与未定案状态、命令契约与角色、运营项、审计标记、未做边界）。
- [x] 7.3 新增 ADR 0035，记录落进 kernel 的理由与十项决策的否决方案（黑名单剔除 / 禁止清单常量 / 九项全要 / 预览无条件受检 / 用中继即缺陷 / 未定案即缺陷或即就绪 / 命令契约含网络行为 / 只靠判定不审计 / 臆造证据目录）。
- [x] 7.4 报告 `docs/reports/2026-10-10-w33-mobile-monitoring-surface.md`：实测数字、逐项验收映射（O03，回指 S09）、未覆盖边界与环境边界。
- [x] 7.5 `tests/registry.yaml`：O03 **保持 `blocked`**，notes 补记 W33 已交付判定机制并**补齐原先缺失的 blocked 原因**；S09 保持 `tested` 并回指本变更引用其契约；`docs/development-plan.md` 与 `AGENTS.md` 记录本轮。`check --registry` 通过。
- [x] 7.6 归档 `w33-mobile-monitoring-surface`（归档名 `2026-10-10-w33-mobile-monitoring-surface`，新 capability `mobile-monitoring` 生效），跑 `validate --all --strict`（含 `--archived`）。
- [x] 7.7 全量 `cargo xtask check --profile portable` 通过并留档；提交 + 推送。

## 里程碑切分

本轮为单个可独立验证的切片：投影与暴露词汇（§2）、最小形态（§3）、预览/加密/路线（§4）、readiness（§5）、命令与运营项（§6）共用同一个 `mobile-monitoring` capability——§7 的审计逐条检查的正是 §2–§6 产出的描述符字段，拆开提交会让审计的第一版无法验证「每条规则都直接可构造」。文档、ADR、registry、归档与门控（§7）为纯声明变更，沿用同一次门控留档。

**顺序纪律**：先跑门控 → 再勾任务项 → 最后归档（`validate --all --archived` 会检查归档 tasks 完成度）。
