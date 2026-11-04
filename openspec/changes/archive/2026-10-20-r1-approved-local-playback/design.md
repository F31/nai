# Design

本机人工辅助输出最多5秒，仅Assisted。文件预览默认不播放；local_playback显式true时独立幂等记录，每个候选/模式最多一次TTS，不自动重试付费请求。PCM/RGBA只经root临时目录的原始文件，控制只发Protobuf元数据、完整审批和短租约，不夹带媒体字节。

media描述符绑定候选/session/epoch、采样率、帧率、尺寸、样本/帧数量和两个文件SHA256；固定文件名，拒绝符号链接、超限和长度/哈希不符。PCM最多5秒，RGBA最多64MiB。keeper先打开设备并发ready，root随后重新读取真实审批proto、cue及approval_valid，发送最长600ms的租约，每100ms完整复检并刷新；继承管道仅本次spawn使用。根写入与读回都在独立有界线程，根监督循环不等管道/数据库锁，失败先终止子进程。进程回收与所有调用有期限。

keeper控制读取在独立线程、队列容量4；主线程非阻塞处理授权、每轮MediaGate轮询与窗口更新。音频按200ms授权块、待消费队列容量1（另有活动块，共2块）；每块完整Preplay复检，回调逐片复检P0/代次/期限，包含预测设备排队时间。默认设备48kHz f32单/双声道；复用有界重采样，试听固定15%增益。EOF或无续租独立静音，未知设备能力不视为已授权。画面依据音频提交样本位置选择真实2D帧，撤权清黑；原生API卡死仍依赖父进程监督，根与窗口同时卡住的独立画面待机认证不声称完成。

输出状态是submitted/cancelled/failed与实际复制样本/窗口提交数，actual_local_playback_completed恒false。它不写PlaybackAck或Ready，不代表平台送达。测试使用明确本地协议Provider，真实Windows设备另外显式运行，不能算付费API认证。

Provider返回格式必须描述实际线上PCM（OpenAI固定24kHz s16 mono），不得回显请求采样率冒充响应；引擎在应用层有界重采样到渲染采样率，保持时长，原始文件与哈希对应转换后的真实字节。
