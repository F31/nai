# Proposal

## Why

ADR 0017 明确「protected handle broker、命名管道身份、render-worker/keeper 生产装配」仍待实现：当前帧句柄由父进程直接以继承 stdio 交付，任何能连上的进程都可自称 producer/consumer；keeper 侧完全没有帧消费与待机切换路径（`apps/output-keeper` 只轮询租约，`status().output_available` 恒 false）。Slice-0 的 A01/R03 需要 keeper 在缺帧、旧 epoch、池退休或租约丢失时切待机并保持静音通道。

## What Changes

- `nai-media-core::frame_slots::broker`：Linux 私有 UDS + `SO_PEERCRED` 身份校验的句柄签发；只有 OS 层身份与声明 peer 匹配的进程才能获得该 pool 的 producer/consumer 授权，身份取自内核而非自报；未授权连接不产生任何 ticket。非 Linux 显式 CapabilityNotReady。
- `nai-media-core::frame_slots::consumer`：帧事件流的消费端——校验 epoch/ticket/完整性，输出 owned 帧或明确的待机原因；不产生 PlaybackAck，不授予输出授权。
- `apps/output-keeper`：新增帧消费与待机切换——把 consumer 与 `MediaGate` 租约、epoch、退休状态绑定；缺帧/迟到/旧 epoch/池退休/租约丢失 → 待机并保持静音通道；`--frame-drill` 显式启动。
- 真实子进程演练：身份不符被拒、正常签发并消费、租约丢失与池退休切待机。
- 明确不做（保持未实现）：SCM_RIGHTS/Windows 命名管道句柄传递（无新依赖，仍走 spawn 继承）、mmap/零拷贝、真实窗口与音频设备、PlaybackAck 生产端、独立 render-worker 生产装配。

## Capabilities

### Modified Capabilities

- `frame-slot-ownership`（新增增量）：句柄签发需 OS 级身份校验，并补充 keeper 侧消费/待机语义。
- `output-watchdog-drill`（新增增量）：keeper 增加帧消费与待机路径。

## Impact

- 新增 media-core broker/consumer 模块与测试、keeper 帧消费模块与演练；无新增 crate、依赖或常驻进程（仅 Linux `std::os::unix`）。
- `tests/registry.yaml`：A01/R03 保持 implemented，notes 补句柄签发与 keeper 待机证据及仍缺项。
- docs/media.md、docs/adr（新增边界 ADR）、docs/development-plan.md、AGENTS.md 同步。
