# D11 媒体基础与样本时轴

## Why
V3.5 第 21/27 章与 `dh-media-core` 目录要求音频格式/时钟/缓冲/排期、样本时轴、录制与实际播放 ACK。
当前 media-core 只有授权门控与 ACK 校验，没有可移植的样本时轴、单调时钟、有界缓冲、录制接口与实际完成语义。
Windows 设备窗口/音轨在 WSL 不可验收，本轮实现可移植内核，不伪造设备 ACK。

## What Changes
- nai-media-core 新增 timeline 模块：audio block 校验、单调媒体时钟、有界环形 PCM 缓冲、重采样样本映射、
  utterance 时轴与水位提交、录制 sink（单调 PTS + 句间静音）与播放 ACK 追踪。
- 音频时长按样本数、视频按有理数 time base；审计 UTC 不是媒体时钟；不得比较不同采样率分支的 sample_start。
- 获准首段音频与首批视频达到水位才 commit；持续迟到取消当前句并降级，不长时间有声无嘴动。
- 录制 PTS 单调，句间补静音保持媒体时钟连续；录制默认不上传。
- 独立窗口/音轨依赖 Windows 原生设备与宿主，如实 Blocked。

## Capabilities
### New Capabilities
- `media-basics-timeline`: 可移植样本时轴、时钟、缓冲、录制与播放 ACK 语义。