# Proposal

## Why

V3.6 §29.4 规定 `dh-runtime`（此处 `crates/nai-runtime`）为 composition root：装配具体实现、可依赖插件，而 kernel 不得依赖它。当前仓库没有任何装配根：存储 owner、会话/值守租约、审批后端、决策日志、媒体门控与 keeper/render-worker 子进程各自存在却从未被接成一个受监督的整体，`apps/render-worker` 也从未建立（`deps.rs` 已预留该路径与 `media` 层）。没有装配根就无法验证 Slice-0 的「故障→撤权→记录→待机」闭环。

## What Changes

- 新增 `crates/nai-runtime`（layer `runtime`，同步实现，不引入 tokio/hyper）：装配存储在 owner、runtime 配置装载与校验、`Session` + 值守租约、审批后端（复用 `nai-control-api::ApprovalBackend`）、决策日志写入（critical/telemetry 分级）、`MediaGate` 与子进程监督。
- 新增 `apps/render-worker`（layer `media`，显式 `--render-drill`）：以 `nai-avatar-2d` 真实生成 RGBA 像素并写入继承的池 FD，事件经 stdin/stdout 帧通道；复检 epoch/取消/期限，不授予 Ready 或播放 ACK。
- 监督语义：runtime spawn/回收 keeper 与 render-worker，续租、解析状态帧、检测退出或强杀；故障 MUST 触发撤销输出授权（epoch 递增）、置待机并写决策日志，链可校验。
- 明确不做（保持未实现）：`apps/engine` 可执行装配与 loopback HTTP 控制服务启动、broker 在装配中的接线、真实窗口/音频设备、PlaybackAck 生产端、Windows 认证。

## Capabilities

### New Capabilities

- `runtime-composition`: 装配根与子进程监督语义（依赖方向、故障→撤权→日志→待机）。

### Modified Capabilities

- `media-frame-slot-ownership`：新增 render-worker 侧生产装配要求（真实像素写入 + 事件上报）。

## Impact

- 新增 1 crate + 1 app，登记 `members`/`default-members`/`workspace.dependencies`/`package.metadata.nai.layer`；无新第三方依赖（复用现有 workspace 依赖）。
- `tests/registry.yaml`：R01/R03/A01/S01 notes 补装配证据；状态不变（不标 tested）。
- docs/media.md、新增 docs/runtime.md、ADR 0019、docs/development-plan.md、AGENTS.md 同步。
