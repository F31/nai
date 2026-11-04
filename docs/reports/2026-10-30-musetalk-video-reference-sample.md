# 真人视频与 Qwen 语音的离线写实样片

## 实际结果

使用用户确认拥有肖像及素材授权的视频，取已备份的 5 秒研究片段和本地 Qwen 生成的 4 秒语音；在隔离 E 盘环境和 RTX 5090 Laptop 上，使用 S3FD 人脸检测、DWPose 关键点、Whisper 音频特征、VAE、MuseTalk 1.5 UNet 及面部分割融合，生成 `target-windows/realistic-avatar-research/video-inspection/musetalk-sample-1793359255491422100/sample.mp4`。固定 SHA-256 `fc7b85066be7139605556b57c8273f09508aca40c06e43a1499f1009cd6c8ae2`。仅本机文件，未推流、未上传、未修改原视频或用户数据库。

实际处理和封装耗时 90.49 秒，包含模型加载、120 帧逐帧定位、推理、融合及编码；输出 960×540、30 fps、120 帧约 4 秒。OpenCV 读回 10/50/100 三帧成功；FFmpeg 全文件解码通过，检测到视频与 24 kHz 单声道音频。抽查画面嘴部在变化，源视频中英文硬字幕仍存在，与新语音内容不一致。未进行精确唇音同步评分、听感、全程逐帧质量或用户主观评价；不能据静帧断定对口型质量。

## 失败及处理

首次 120 帧定位完成后，PyTorch 2.7 的 `weights_only=True` 拒绝读取 ResNet18 旧 tar 预训练文件，未产生视频。另行检查表明面部分割完整权重可安全加载，包含 120 项 ResNet 参数；运行时仅临时跳过构造骨干时的冗余预训练加载，再以默认 strict 方式加载完整分割 checkpoint，未使用 `weights_only=False`、未修改第三方源码。失败输出目录保留。DWPose 安全加载允许此前核验 checkpoint 所需的有限 NumPy dtype、数组与 HistoryBuffer 类型，并关闭骨干隐式预训练下载；模型/配置来自已固定摘要目录。UNet 采用 CPU 映射和 FP16 GPU 装配以避免上轮默认加载错误。

新增 `scripts/run-musetalk-reference-sample.py` 为固定输入、最多 120 帧的离线研究脚本；运行前单独执行 `scripts/stage-musetalk-reference.py` 复核 11 项文件摘要。该脚本明确依赖 Python/PyTorch，属于研究工具，不是 Windows 正式运行后端，且不代表能稳定接受其他视频、并发或流式音频。

## 验证与后续

Python 语法检查通过，v49 OpenSpec 严格校验通过；本轮生成一次成功样片，未新调用 TTS。用户尚未人工试听或评价，故 `v49-realistic-avatar` 样片验收任务仍未勾选。整体 90.49 秒处理 4 秒素材不能视为实时速度；尚未测连续流、逐帧时延、显存峰值、唇音偏差、长稳、异常接管、工作台或 OBS 输出。模型和依赖许可闭包及 NativeOnnx 后端也未完成，注册表状态不变。

后续用户反馈：声音正常，口型可以接受。这是该用户对首段样片的主观体验，不等于同步量化、实时性能或正式验收通过；分阶段性能复测见 [报告](2026-10-30-musetalk-reference-profile.md)。

下一步根据用户试听检查口型、清晰度及闪烁；同步量化各阶段耗时和显存峰值、优化预处理与音画时序，再接入受控实时宿主、公共输出和 OBS。带硬字幕源只用于技术研究，正式使用建议无字幕素材。
