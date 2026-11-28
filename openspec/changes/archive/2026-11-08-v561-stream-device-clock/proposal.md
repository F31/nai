# 分句流式设备时钟

## Why

新CosyVoice分句预取链路尚未把认证设备时钟交给帧调度。旧一次性PCM诊断不能直接当新生产链，先补同源时钟接线，防止以已提交样本或墙钟代替播放进度。

## What Changes

streamaudio协议升2并强制包含签名PlaybackClockObservation及设备观察年龄；旧协议拒绝。独立设备提供真实WASAPI轮询的同源快照，根校验epoch/设备代次/序号/进度/年龄后用固定有界观察通道连接Scheduler。新增Windows静音帧调度与真实CosyVoice验证，不新增依赖，不修改模型、数据库或声称FlashHead/OBS/M0通过。
