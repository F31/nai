## Why
生产常驻音频已用IAudioClock确认尾部，但画面仍随copied缓冲位置推进，可能提前最多一段缓冲。按V3.11 §5.7/§8.4与A01/R07补齐实际播放位置到画面的接线，不用提交次数冒充音画完成。
## What Changes
- 画面索引由实际设备played位置驱动，拒绝回退/越界，尾帧须在音频到尾后成功提交。
- Protobuf先追加可选视频索引/设备位置与尾帧提交标志，再生成消费者。
- 宿主以独立预期帧数/样本数验证提交，旧缺证报告拒绝，人工确认沿用相同复核。
## Capabilities
### Modified Capabilities
- `resident-local-output`: 实际设备时钟驱动画面及尾帧提交诊断。
## Impact
media-core纯时间轴、keeper窗口循环、engine复检、相关契约与测试；零新依赖/服务/付费请求。不升级完整PlaybackAck或自动恢复。
