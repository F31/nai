# MuseTalk 候选权重与真实 DWPose 盘点

状态：DWPose 权重与真实照片的单次本机 GPU 执行通过；不代表完整 MuseTalk、真人画面、质量或实时认证通过。

官方 Hugging Face 直连超时。使用上游脚本所列镜像 `hf-mirror.com`，Python requests 启用证书验证、禁用环境代理；新增 `scripts/download-musetalk-reference.py`，仅允许两个固定版本和官方文件页 SHA-256，支持分块、断点、有限重试及完成后摘要校验，文件留在 E 盘。DWPose 固定镜像仓库 revision `1a7144101628d69ee7a3768d1ee3a094070dc388`，实际 SHA-256 `0d9408b13cd863c4e95a149dd31232f88f2a12aa6cf8964ed74d7d97748c7a07` 与官方文件页一致；来源许可证声明 Apache-2.0。MuseTalk 1.5 固定 revision `2bcb936e2fddb4d86db4c62fd45b387d0c061571`，预期 SHA-256 `7ebf6c98c181e20838e4c0054e96e944ac60d5d692cc01db42839fe11b787007`，截至本报告仍在续传，未完成或验哈希。

使用官方摘要核验的 DWPose 权重、上游配置和用户授权 `docs/jjf.jpg`，在隔离 RTX 5090 进程完成 `inference_topdown`；结果一组 `(1,133,2)`，坐标有限，退出码0。未保存关键点或上传照片；使用默认整图检测区域，未验证识别准确率、嘴部定位质量或多姿态。Torch 2.7 默认安全加载首次拒绝旧检查点包含的 NumPy 及 `HistoryBuffer` 元数据；依据权重列举的类型和有限 NumPy dtype 使用临时 safe_globals 白名单后成功，未关闭 `weights_only` 安全限制。白名单仅用于这次隔离实验，尚未接入正式产品。

仍缺 MuseTalk 主权重、VAE、Whisper、人脸检测/解析权重的取得与核验和完整依赖许可闭包；MMCV 为 CPU 原生扩展，CUDA NMS 不可用。未运行完整 MuseTalk、未生成真人视频、未接公共输出或 OBS。不得以此次姿态模型执行等同真人数字人验收。

## 后续下载进展

MuseTalk 1.5 主权重续传完成，SHA-256 与上文官方值一致；VAE safetensors、Whisper tiny 和上游脚本指定的人脸解析权重已固定镜像提交与官方文件页摘要，并启动 E 盘隔离下载。上述三项完成和校验情况以下载进程最终结果为准；不以启动下载表示权重可用。配置文件、ResNet、人脸检测权重和完整许可/加载链路仍需核验。研究环境中的上游 Python 推理不代表正式产品 NativeOnnx 就绪。
