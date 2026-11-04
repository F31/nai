# ADR 0144：常驻 WASAPI 与候选实际音频进度

状态：Accepted（2026-10-21）。依据 V3.11 W05/W10 与 ADR0142/0143，不改变完整 A/V PlaybackAck 定义。

常驻本机播放改为安全 wasapi 0.23.0（MIT）的共享事件流，48kHz双声道浮点；COM对象仅在专用线程创建/使用，每keeper只打开一条stream跨cue/中性待机复用。CPAL仍供既有显式device-smoke使用。wasapi从Windows开发依赖升运行依赖；锁文件已锁定，无新增服务/端口/模型/API调用。线程替换原CPAL音频线程，预分配两块各38400字节缓冲，包队列容量1、在途尾部容量32；队列满/设备错误/未知时序拒绝完成。设备线程没有DB/模型锁；Drop只设停止信号，100ms有界事件等待后停止；异常退出置健康失败，父进程仍有回收边界。

原生成功write/release后才提交绝对流帧范围，静音也计入流坐标但不增加cue计数。实际IAudioClock position/frequency经DeviceClockCursor复检同流、单调、稳定频率和提交上界，不使用QPC为播放位置。每个在途buffer保留语音尾部、cue专属累计帧计数、最早期限和DrainFence；设备位置覆盖该尾部且未撤权/换代/过期/error，才更新该cue的played计数。保留逐buffer期限，不能以续租后的最后一块覆盖较早期限。音频编码后、原生提交前再复检否决栅栏；失败提交零样本，不登记语音尾部。

LocalPlaybackStatus追加optional device_played_samples(12)、device_position_observed(13)、audio_backend(14)。实际音频成功提交须本次描述符摘要、copied/played等于宿主独立预期帧数、已观测、wasapi-iaudioclock、单stream和窗口提交，宿主两个成功出口末次复检原授权；operator保存完整playback.pb。旧预计callback诊断不再作为成功依据，旧keeper缺实际字段不能降级通过。

音频实际尾部不代表完整A/V已呈现或声音被扬声器/OBS收录，actual_local_playback_completed保持false，无PlaybackAck、已完成句或自动前进。节目继续人工听完确认；画面仍按复制位置更新，下一步需定义并实现完整音画完成与检查点，之后接有限节目自动讲品/插问归位、真人音轨和OBS实录。桌面显示引擎实际结论，并明确音画仍需人工确认。

未更新安装文件/用户数据库、无付费API请求；设备测试使用显式本地夹具，不是正式直播认证，registry状态不升级。
