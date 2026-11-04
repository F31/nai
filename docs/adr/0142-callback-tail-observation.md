# ADR 0142：尾部回调观测与本次媒体绑定

状态：Accepted（2026-10-21）。依据V3.11 §21.3与ADR 0137–0141，不改变Completed定义、验收状态或人工辅助节目边界。

本地锁定依赖cpal 0.16.0的`src/host/wasapi/stream.rs::output_timestamp`明确说明playback时间是estimate；当前接口不能给出硬件播放位置证明。旧keeper在全部样本复制后固定等待220ms，再返回submitted；该等待不是完成证据。

保留submitted/actual_local_playback_completed=false，改为要求后续真实音频回调越过最后一段音频的预计播放尾部。回调保存一个有界pending，包含累计frame数、预计尾部时刻、最早期限和只否决的DrainFence；后续回调需时序有效、未过期、原epoch/撤权信号不变且无cue错误，才写该cue的drained原子计数。没有回调、只有主线程等待、空缓冲、未知/超限延迟不能补证。回调无新增分配、锁或IO；队列与P0独立，沿用现有容量和deadline。

DrainFence只携带原授权失效信号和期限，无任何输出授权接口，不代替MediaGate/Preplay。每cue单独持有计数；新cue不能认领旧cue的尾部。计数为48kHz帧数，双声道不是两倍帧数。

LocalPlaybackStatus新增media_sha256（本次描述符摘要）、optional callback_drained_samples与callback_drain_observed。中性预热仍可缺失；缺失不等于0或已排空。宿主两个成功出口统一要求本次摘要一致、复制数/排空数等于其独立推算的重采样预期数、单stream和窗口提交，仍复检现有授权。旧keeper缺新字段不能在新宿主形成成功结果；不自动降级或重放。

完整诊断随既有playback.pb保存，仍不产生PlaybackAck、已完成句、工作记忆或自动节目推进。它证明的是“真实回调在预计尾部后仍运行且未撤权”，不是设备实际消费进度，更不是扬声器声音可听或OBS已录制。下一阶段需要独立可核验的后端播放进度来源，之后再接完成ACK、检查点和自动推进；不能把本诊断简单改名为Completed。

无DB迁移、新增依赖或常驻服务。不替换现有安装，不改真实用户库或调用付费API。
