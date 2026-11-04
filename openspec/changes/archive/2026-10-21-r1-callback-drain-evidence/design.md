## Context
CPAL 0.16.0 WASAPI output_timestamp 明确说明playback是estimate（本地依赖stream.rs）；不能据此宣称硬件完成。现有submitted在复制后sleep220ms，没有后续回调产生者。
## Goals / Non-Goals
补齐真实回调尾部观测及跨进程身份，不将其升级硬件ACK或自动节目检查点。本轮不接自动归位、麦克风或OBS。
## Decisions
DeviceChunk提供只否决的可克隆fence，持有原始deadline与撤权/epoch原子信号。keeper的每packet持有同cue的drained原子计数；回调仅保留一个待观测尾部。填完本次缓冲后记录预计尾部时刻、累计frame数、fence和失败信号，下一次有效回调到达该时刻后才写drained。无回调不能由主线程定时器自证。新cue、撤权、到期、异常时不得将旧pending归给新cue；全部计数按声道frame计算。

keeper成功时发送描述符摘要、完整drained计数和observation标志；宿主以独立重采样预期数核对，两个退出分支都需同样验证。缺字段旧keeper不能对新宿主产生成功结果。保留submitted状态和actual_local_playback_completed=false，兼容原人工确认语义，不触发自动推进。P0仍不等待业务锁，主线程继续每5ms租约复检。
## Risks / Trade-offs
- 后端预计时间并非设备/扬声器实测 → 字段与文档明确是callback horizon，不写PlaybackAck。
- 回调中断 → 在既有deadline内失败，不能用sleep补证。
- 观测后发生撤权 → root再次复检当前授权，旧证据不能授予后续输出。
## Migration Plan
先扩展protobuf并生成，再修改生产者/消费者。无DB迁移或新增常驻依赖。不替换安装版。
