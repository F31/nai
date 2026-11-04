# ADR 0139：独立中性输出预热，不借用语音授权

状态：Accepted（2026-10-21）。承接ADR0137/0138，依据V3.11 §5.7、§8.4、§10.1。

节目启动需在任何LLM/TTS请求前确认合法中性画面和本机输出。新增显式 `POST /sessions/{id}/output-prewarm`，沿既有控制鉴权、会话/epoch/CAS/seq/TTL与共享幂等账本，独立权限 `media.prepare`，不等价于审批、播放或自动模式授权。操作异步且有界，ACK仅接收，events的output_ready才是本次准备确认；不是平台Ready或硬件播放ACK。

root从已准入的Avatar与许可生成闭嘴帧，写独立RGBA文件和LocalStandbyMedia描述符。私有继承管道上的LocalOutputWarmup绑定描述符SHA256、session/epoch、至多4秒处理预算，不包含PerformanceCue/Approval/PlaybackLease；像素不走控制总线。keeper读取时检查类型、尺寸、16MiB帧上限及两个摘要，然后呈现和报告零语音样本、单stream、待机摘要。root再复检epoch/取消和至多8秒任务期限，失败清理未确认子进程及临时文件，不重试Provider。

预热与既有语音渲染共享单槽准入与resident输出锁；忙时拒绝，无排队增长。P0沿独立EmergencyHandle撤权，不等待预热锁。成功后后续真实审批播放复用同一窗口、进程、stream，仍逐句完整守门。已关闭的子进程可由下一次显式预热重新建立；失败不偷偷重播语音。

订正ADR0138“首个完整preplay后装入中性图”：现在有独立合法静默预热准入路径；语音路径的preplay仍不改变。形象与许可在该引擎会话固定，变更需停机重准入。该路径不是动态许可撤销、正式共享内存、命名管道认证、长播、连续讲品、真人麦克风或OBS实录。
