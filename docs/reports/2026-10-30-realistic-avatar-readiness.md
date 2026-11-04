# 真人视频样片准备检查

为 v49 写实样片新增 `scripts/check-realistic-avatar-readiness.py`，仅在 E 盘研究目录读取指定文件并核对既有固定 SHA-256，检查本机 127.0.0.1:8000 TCP 可达性；不加载模型、不调用 TTS、不扫描用户业务素材、不请求远端。端口开放不等于服务身份或合成可用，缺少素材与许可闭包保持未验证。

本机实际结果：MuseTalk 1.5 UNet、DWPose、VAE、Whisper tiny 和 face-parse 五项权重与现有固定摘要一致；四个小型配置已从固定版本镜像下载到 `downloads/metadata-candidates` 并记录 SHA-256，但官方源直连超时，独立来源一致性未完成，仍标为候选，未用于模型加载。上游路径仍需 S3FD、ResNet18、人脸检测与依赖/许可闭包核验；Qwen 8000 端口未检测到监听，合法真人视频待用户提供。没有运行完整 MuseTalk，也没有生成写实视频或接入 OBS。

验证：脚本实际运行成功、Python 语法编译通过，写实 OpenSpec 严格验证通过。输入文件和缓存位于 `E:\projects\nai\target-windows`，未修改用户业务数据或安装版；本轮不增加 ACC 通过状态。下一步取得视频及可用 Qwen，核验配置来源和其余依赖，再进行隔离的样片推理和真实可播放验证。
