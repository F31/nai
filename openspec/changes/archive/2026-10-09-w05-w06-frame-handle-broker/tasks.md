# Tasks

## 1. 规格与校验
- [x] 1.1 完成 proposal/design/tasks/specs 并通过 `pnpm exec openspec validate w05-w06-frame-handle-broker --strict --no-interactive`。

## 2. 契约（provisional）
- [x] 2.1 `schemas/domain/media.proto` 增加 `OutputWatchdogCommand.submit_frame = 8`、`OutputWatchdogStatus.presenting = 13`、`standby_reason = 14`（仅新增字段）。
- [x] 2.2 `cargo xtask schema` 重新生成 Rust/TS/C++，`schema --check` 无 drift；互通测试覆盖新字段。

## 3. media-core broker
- [x] 3.1 `frame_slots::broker`：UDS + `SO_PEERCRED` 身份校验、授权门控、有界 accept、私有 socket 权限与 unlink、非 Linux CapabilityNotReady。
- [x] 3.2 抽出 `read_committed_frame` 供 owner 与消费者共用；`SharedFramePool::read_frame` 复用且既有测试保持通过。
- [x] 3.3 测试：未授权拒绝、role/pid 不符拒绝、accept 上限与期限、授权后写入/读回、真实子进程接受与错误 pid 拒绝。

## 4. media-core consumer
- [x] 4.1 `frame_slots::consumer`：`FrameReader`、`FrameConsumer::submit` 与 `StandbyReason` 全集。
- [x] 4.2 测试：lease false 不读像素、write_started/error/stale epoch/篡改/长度变化/超时 → 对应待机原因、成功投递与 epoch 记录。

## 5. keeper
- [x] 5.1 `FramePath` + `Watchdog::apply` 的 `SubmitFrame`、`poll` 租约失效切待机、`status` 新字段。
- [x] 5.2 `--frame-drill` 启动模式（stderr 继承池 FD、`/proc/self/fd/2` 只读重开），未显式启用时不建帧路径。
- [x] 5.3 测试：真子进程写入 → presenting；租约到期 → standby(LeaseLost)；池退休 → standby(Retired)；`output_available` 恒 false。

## 6. 收尾
- [x] 6.1 `tests/registry.yaml` A01/R03 notes 补本轮证据与仍缺项；docs/media.md、ADR 0018、docs/development-plan.md、AGENTS.md 更新。
- [x] 6.2 运行 `cargo test --workspace --offline`、`cargo xtask test --suite contracts`、`check --registry`、`check-deps`、`check --profile portable`、`schema --check`、`pnpm -r typecheck`。
- [x] 6.3 写 `docs/reports/2026-10-09-w05-w06-frame-handle-broker.md`，归档并 `pnpm exec openspec validate --archived --no-interactive`。

## Workflow follow-up

- 主规格 `media-frame-slot-ownership`、`output-watchdog-process` 增量生效。
- 后续：SCM_RIGHTS/Windows 命名管道句柄传递、独立 render-worker 生产装配、PlaybackAck 生产端、真实设备与性能认证。
