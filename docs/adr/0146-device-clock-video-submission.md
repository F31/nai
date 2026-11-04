# ADR 0146：设备时钟驱动画面提交

状态：Accepted（2026-10-22）。沿用V3.11 §5.7/§8.4与ADR0144/0145，完整AV PlaybackAck定义不变。

生产常驻视频不再跟随音频copied计数，而用当前cue经IAudioClock覆盖后的played设备帧数。每cue创建独立VideoCursor，用整数算术选择对应视频帧；重复位置允许，回退/超出copied或独立总帧数使游标失效，旧提交标记不可恢复。先读played再读copied，防止并发正常前进造成假越界。

选帧与提交分离：读回并校验像素后，复检已观测取消、租约、音频健康与cue错误，窗口update成功才登记对应位置与索引。只有实际音频全量位置对应的最终帧已成功提交，才输出submitted尾帧证据；结束前再次检查租约/取消，撤权不携带成功尾帧。中性待机不继承cue证据。

LocalPlaybackStatus追加optional video_frame_index(15)、optional video_device_samples(16)及video_tail_submitted(17)。宿主两个成功出口统一对照独立expected_frames-1与expected_samples；人工确认报告SHA256继续绑定新字段并按相同要求复检。缺新字段的旧keeper报告不能降级通过，维护时须同时更新引擎和keeper。

这证明实际音频位置驱动的窗口尾帧提交，不证明每帧均显示、屏幕或OBS采集完成，也不证明人耳听到。繁忙时可能跳帧；actual_local_playback_completed仍false，不产生PlaybackAck、机器已完成句、正式恢复检查点或自动讲品。后续完整音画完成需要独立呈现/采集回执，并以实际录制验收，不从这些诊断字段推导。

零新依赖/后台服务/付费调用；媒体像素仍独立文件通路，不进入JSON或Protobuf控制载荷。
