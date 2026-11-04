## Design
常驻 keeper 持有唯一 minifb 窗口，按句创建并释放 cpal stream，PID/窗口在句间保持。私有继承管道仅传描述符路径/租约，PCM/RGBA保持文件通道。命令队列4、状态队列2、最多一个播放；主控撤权先确认静音待机、750ms未确认则回收子进程；EOF/租约到期独立静音，不自动重播。空闲窗口黑色，最长会话30分钟后显式失败需重新连接。保留原 --local-playback 诊断协议。所有早退先撤回调授权再析构音频流；UI忙碌时静音与人工接管可用。见ADR0137。
先改 protobuf/OpenAPI并生成；Linux逻辑检查、Windows真实输出双句复用和EOF/失租定向测试。设备提交不是硬件PlaybackAck。后续连续讲品复用现有PitchRunner，麦克风需真实单路采集/回声证据，OBS录制是独立验收。

