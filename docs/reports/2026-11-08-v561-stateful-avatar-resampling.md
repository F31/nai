# 2026-11-08 连续音频重采样基线

## 实现

新增scripts/avatar_audio_stream.py，使用已安装soxr 1.1.0/float32/HQ的ResampleStream，保留24kHz→16kHz滤波状态，将特征按已验证模型块帧数装配（当前40帧=25600特征样本）。输入每次最多38400源样本、总量144000，单pending块，输出同步背压，取消/格式/超限/消费者失败即终结且清空pending。只有调用方验证供应商EOS后才调用finish，flush和ceil长度修正与librosa保持一致；不足模型块补零与有效特征数分开。

本组件为Python模型侧内部输入算法，不涉及跨语言消息，因此本轮没有修改或生成协议。正式worker仍协议6、整段PCM；组件未装配到生产推理，未更改motion/画质、安装版或主UI。不新增依赖，不复活Qwen服务。

## 验证

5项CPU测试通过：8组长度（1/2/3/959/960/38400/73921/144000源样本）×3种传输分片，与现有librosa整段soxr_hq逐样本比较，绝对误差阈值2e-7、长度完全相同；输出偏移/有效样本/尾零、EOS前输出与无提前补尾、错误和限额终结、取消/消费者故障、单次EOS/空输入/几何边界。相邻frame_prefix测试3项通过、1项既有GPU测试未执行（skip），不算GPU通过。

另以已有授权真实语音qwen-av-diagnostic-1920ms.pcm做CPU对照（文件名是历史诊断，不调用Qwen）：46080源样本→30720特征样本，158/1920/76800字节三种分片均最大绝对误差0.0；两个固定计算块、有效样本和补尾分离。证据target-windows/v561-stateful-avatar-resampling/source-comparison.json。未调用模型API/GPU、未播放/录制/推流。

## 下一步

新增正式Begin/PCM元数据引用/EOS/取消契约并生成消费者，PCM保持独立原始数据通道；worker单任务内接本重采样器与现有缓存音频/motion，禁止逐块调用会重置状态的render。补充EOS前临时计算及失败撤销、未知总长下的帧索引与尾帧裁剪；再做真实GPU同源窗口播放。重采样只解决连续输入正确性，本轮没有首声加速证据。34ACC仍pending、M0仍No-Go。
