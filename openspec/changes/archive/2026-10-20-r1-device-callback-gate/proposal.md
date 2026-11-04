# Proposal

## Why

设备后端尚未接入；现有 MediaGate 只在控制线程消费时复检，已交给回调的 PCM 没有独立撤权边界。

## What Changes

- 增加只能经当前 Preplay 与队列消费产生的有界 PCM 回调对象。
- 回调逐帧检查 P0 与代次，按单调期限静音；租约续期不延长旧对象。
- 明确样本复制不是设备完成，不产生 PlaybackAck。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `media-basics-timeline`: 回调侧租约与撤权。

## Impact

仅 nai-media-core 内部 Rust API，无跨语言消息、配置或 HTTP 变更，无新增依赖。不接入设备或改变 registry 状态。
