## Context
既有Avatar::standby只从准入素材生成闭嘴像素，不依赖合成；resident keeper可保留一条stream及静默画面。应复用此链路而非假造已审批文本。
## Decisions
新增LocalStandbyMedia元数据与Warmup命令，RGBA仍以独立文件传递，文件大小/摘要/几何尺寸校验。命令包含描述符摘要、session/epoch和短预算，只走可信父进程继承管道；该命令没有语音授权能力。
HTTP使用SessionCommand元数据，独立media.prepare权限，共享幂等账本；异步执行，既有events给出output_preparing/output_ready/failed，ready包含待机摘要。父进程复检epoch/取消/期限；设备确认必须一条stream、零样本和匹配待机摘要，否则失败回收。不碰LLM/TTS、不写PlaybackAck。
预热与语音共享有界输出互斥；忙时拒绝，不排队。静音路径不等预热业务锁。许可随当前引擎准入，换形象/许可需停止重准入。中性待机不声称任何平台Ready，动态许可撤销/长播另项。
## Validation
媒体描述符篡改/超限，控制权限/幂等/陈旧epoch，UI只在真实ready显示成功；Windows原生静默预热后真实cue复用同一stream。契约生成和相关三语言互通；模块测试与生产UI构建。不调用真实Provider、不改安装/用户库。
