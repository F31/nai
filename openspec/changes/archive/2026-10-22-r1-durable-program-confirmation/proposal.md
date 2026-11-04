## Why
R1有限节目确认仅在内存前进，重启丢失人工复核线索，确认还只检查预览状态字符串。按V3.11 §5.7/§53与registry B07/R07/S02推进证据绑定及持久确认，为后续检查点恢复提供可追溯基础。
## What Changes
- 先声明OpenAPI人工复核记录与节目可选最近记录，再生成契约。
- 本机播放回执摘要绑定内存结果，确认复读有界报告并核验实际设备进度、有效审批/事实与epoch。
- 通过已有critical cue日志原子保存人工复核记录，持久成功才推进；重启仅展示最近历史，不恢复。
## Capabilities
### Modified Capabilities
- `basic-selling-loop`: 可追溯人工复核记录。
## Impact
control OpenAPI、engine节目/播放结果、storage只读有界查询、桌面进度。无新依赖、迁移、常驻服务或付费调用；不改变完整PlaybackAck定义，不实现自动归位/麦克风/OBS。
