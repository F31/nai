## Why
V3.11 W05/W10 的候选实际音频进度尚未接线；ADR0143已实测IAudioClock，现以真实位置取代生产CPAL预计尾部作为提交条件。
## What Changes
常驻输出改用安全WASAPI专用线程；成功写入后绑定本次cue绝对尾帧和撤权栅栏，实际设备位置越过尾帧才报告完整音频进度。Protobuf追加独立设备进度字段，宿主两个出口及持久化复检。
## Impact
media-core/keeper/engine及生成契约；wasapi从Windows开发依赖升为运行依赖，无新增服务，线程替换CPAL已有音频线程，预分配有界缓冲。R07/A01只补实测不升级；不生成完整A/V PlaybackAck或自动推进。
