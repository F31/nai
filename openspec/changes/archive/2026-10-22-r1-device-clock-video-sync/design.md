## Context
WASAPI分别提供copied与played原子计数；目前帧索引用前者，导致嘴型超前。minifb窗口调用成功只能证明提交，不证明显示器/OBS呈现。
## Goals / Non-Goals
Goals: 真实音频位置驱动画面，绑定尾帧和当前cue的提交证据，取消不出成功证据。
Non-Goals: 全部帧实际显示认证、GPU/compositor回执、完整AV ACK、自动节目/插问归位。
## Decisions
- media-core新增每cue VideoCursor，整数算术按48k实际device played位置选帧，校验played<=copied<=总帧数和单调，重复观测允许。选择与提交分离；只有对应窗口update成功后才登记。
- keeper在读取像素后、窗口提交前再次检查取消/租约/音频健康；成功尾部要求played全量且最终帧已提交，随后才转中性待机。诊断不包含像素，不走控制媒体。
- LocalPlaybackStatus追加optional video_frame_index(15)、optional video_device_samples(16)、video_tail_submitted(17)。宿主必须比对独立expected_frames-1和expected_samples，音频/画面/描述符身份缺任一均拒绝submitted。
- 原人工确认报告SHA256继续绑定所有新字段，实际完成仍false；不新增用户选择或填写。
## Risks / Trade-offs
CPU繁忙可能跳帧，此轮只保证选帧时基与最后帧提交，不宣称每帧呈现。窗口调用成功仍非完整音画ACK；后续应补独立呈现回执/录制证据与正式检查点。Windows设备集成明确使用本地协议夹具，非付费服务认证。
