# ADR 0013：媒体基础与样本时轴（D11）

- 日期：2026-10-08
- 状态：Accepted
- 基线：V3.5 第 21/22/27 章与 `dh-media-core`（audio/clock/ring/timeline/resampler）；活动变更 `d11-media-basics`

## 决策

`nai-media-core::timeline` 提供纯逻辑、无设备/网络的媒体时序原语，仅依赖 contracts：
`audio`（块/seq/end_of_stream）、`clock`（单调媒体时钟与静音连续性）、`ring`（有界 PCM 缓冲）、
`resample`（确定性线性插值）、`record`（录制 sink 与句间静音）、`ack`（实际播放 ACK）与 `timeline`（utterance 时轴水位提交）。

媒体时钟位置只增不减，静音/句间补静音保持连续；审计 UTC 不是媒体时钟。音频时长按样本数、视频按帧序；
不同采样率分支的 sample_start 不得直接比较，必须用有理比例映射（16k 模型 → 48k 输出的对应样本）。
缓冲超限返回 Capacity，不静默丢弃或截断；首段获准音频与首批视频都达水位才 commit，
持续迟到返回 Late 由调用方取消/降级，不允许长时间有声无嘴动或其他缺同步。
录制以媒体时钟位置单调 PTS，句间写确定静音补齐；录制默认不上传，不以录制文件代替设备验收。
Completed 仅在实际播放完成 ACK（committed && actual_local_playback_completed && 样本数匹配）时上报，
生成结束、完整下载、接收都不等同观众已听完；状态不明任务按取消处理，避免重复播报。

## 边界与后果

本轮是可移植内核，不实现 Windows 独立窗口/音轨、虚拟摄像头/音频设备或录制文件装配（D11 设备里程碑，Blocked）。
设备层与宿主仍须调用本时序逻辑或等价更强保证；media-core 门控（MediaGate/租约）继续兜底 P0 与授权。
录制 PTS 连续语义需设备侧对齐，真实音量/声卡缓冲不在本轮保证。