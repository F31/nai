# Proposal

## Why

R1 已能让操作者批准最终文本，但批准后的文本仍不会变成声音或形象。真实 TTS、PCM、2D 渲染部件各自存在，缺少从「已批准候选」到「可核验本地预览」的应用接线。

## What Changes

- 允许操作者对仍有效的已批准候选发起一次有界预览：真实 API TTS → 有界 PCM → 真实 RGBA 帧。
- 预览写出可试听的 WAV 与帧摘要到本机数据根下的显式目录；不向设备播放，也不伪造 PlaybackAck。
- 复用候选哈希、epoch、TTL、审批有效性与持久预算；静音/接管取消在途合成，不留预览。
- 桌面在批准成功后提供预览入口，显示真实样本数、帧数与产物引用。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `local-control-surface`: 已批准候选的 TTS 与 2D 预览执行面。

## Impact

复用现有 Provider 适配器、媒体核心与 2D 插件；需要让语音适配器暴露已计费的音频字节，并把 engine 的帧循环抽成可复用路径。新增 `POST/GET /sessions/{id}/previews` 控制路由与本地场景 `render` 配置。不新增验收状态，不宣称设备播放或 TTS 真实账户认证。
