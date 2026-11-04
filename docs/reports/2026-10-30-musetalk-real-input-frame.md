# MuseTalk 真实视频与本地语音的单帧验证

## 依赖和核验

上游 face-alignment 代码引用 `s3fd-619a316812.pth`；通过固定版本镜像完成下载并校验 SHA-256 `619a31681264d3f7f7fc7a16a42cbbe8b23f31a256f75a366e5a1bcd59b33543`，与 [模型文件公开元数据](https://huggingface.co/chunyu-li/face-alignment/blob/30a934a95e2f903250ecf929846a1ebc475c2b0f/s3fd-619a316812.pth) 一致，且与 [原始地址](https://github.com/1adrianb/face-alignment/blob/master/face_alignment/detection/sfd/sfd_detector.py) 的文件名哈希前缀一致。镜像归属及该文件页许可证不等于全部传递许可闭包已完成。

四个配置的本地 Git blob 标识，分别与固定官方来源返回的 ETag 一致：MuseTalk `b822db87e503a283fbbee73617f89dcd294cb91c`、VAE `0db26717579be63eb0ddbf15b43faa43700dfe5a`、Whisper config `417aa9de49a132dd3eb6a56d3be2718b15f08917` 和 preprocessor `c2048dfa9fd94a052e62e908d2c4dfb18534b4d2`；其 SHA-256 固定于检查与装配脚本。来源见 [MuseTalk 配置](https://huggingface.co/TMElyralab/MuseTalk/resolve/main/musetalkV15/musetalk.json?download=true)、[VAE 配置](https://huggingface.co/stabilityai/sd-vae-ft-mse/resolve/31f26fdeee1355a5c34592e401dd41e45d25a493/config.json)、[Whisper 配置](https://huggingface.co/openai/whisper-tiny/resolve/169d4a4341b33bc18d8881c4b69c2e104e1cc0af/config.json?download=true)、[Whisper 特征配置](https://huggingface.co/openai/whisper-tiny/resolve/169d4a4341b33bc18d8881c4b69c2e104e1cc0af/preprocessor_config.json?download=true)。ResNet18 直接从 PyTorch 上游地址下载，哈希与地址前缀一致，完整摘要为实测值，尚未取得独立完整摘要。

`scripts/stage-musetalk-reference.py` 对 11 项已固定本地摘要校验后复制或硬链接至 E 盘 `offline-reference`，已有目标不一致时拒绝覆盖；研究文件不进入正式包。`scripts/check-realistic-avatar-readiness.py` 显示六权重和四个配置摘要通过，但视频权利和完整依赖许可不能由脚本验证。

## 实测

本机 PyTorch 2.7.1+cu128、RTX 5090 Laptop，VAE、Whisper 与音频特征提取器分别以离线文件成功加载；上游 UNet 默认直接恢复 GPU 权重时发生 CUDA OOM，错误同时显示约 20.33 GiB 可用，不足以单凭报错诊断实际原因。隔离实验改为 `weights_only=True`、CPU 内存映射、空权重初始化、严格参数匹配后转 GPU FP16，686 个参数项、缺失 0、额外 0，GPU 占用约 1633 MiB。该实验不是生产后端，也未修改上游源码。

使用已确认授权视频的研究片段首帧，OpenCV Haar 粗定位出一个人脸；将已合成的 4 秒 Qwen 音频通过 Whisper 处理得到 120 个 50×384 音频窗口，以其中一窗及该脸部裁剪通过 VAE→MuseTalk UNet→VAE 输出单张 256×256 面部诊断图。总过程 61.34 秒（含首次加载/JIT），末时 PyTorch GPU 分配约 1818 MiB；不是帧率或稳态基准。诊断图有明显模糊、未做正式人脸定位、分割和画面融合，不代表可用口型质量；未产生视频、未进行同步或听感验收。

## 检查及剩余任务

六项权重与四配置摘要的只读检查、11 文件离线装配、真实加载和单帧推理通过；v49 OpenSpec 严格校验通过。沙箱中 GPU 加载曾长时间无结果并由操作方中止，随后以明确权限进行隔离 GPU 试验成功；未上传视频、未推流、未修改用户数据库或启动应用。检查记录见 E 盘 `video-inspection/single-frame-diagnostic.png`。

仍需真实 DWPose/S3FD 定位、逐帧处理和区域融合、真实视频编码与音频复用、生成结果试听/目视、性能和 OBS 验收；完整依赖与模型许可仍待核验。不得将单帧实验视为真人视频、实时口型、正式 NativeOnnx 后端或直播通过。ACC 不变。
