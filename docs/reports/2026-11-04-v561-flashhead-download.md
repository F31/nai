# FlashHead 权重下载诊断

用户运行 huggingface_hub 下载时遇到 FileMetadataError；本机复测官方和镜像小文件 HTTP 200，但镜像权重入口 308 重定向官方站，huggingface_hub 的元数据检查重现失败。此前官方大文件传输也有连接中断；尚不能确定具体网络设备或代理是原因。

已改为官方固定版本 `5fbd2e44ba08ab2deb28bd55cff2c15523e51ba5` 的 curl 文件下载，`--continue-at -`、有限重试，写入 `target-windows/models/SoulX-FlashHead-1_3B/Model_Lite/diffusion_pytorch_model.safetensors.part`。下载仍在运行，VAE 与 wav2vec2 尚未下载；`.part` 不代表权重完整。完成后检查长度 6107542048 及 SHA-256 `aaf1cde6e80ca23f740aae236c47954249f65b151db133cc0f77d3a138ccdf6e`，通过后才可改名为正式权重。不得并发运行同一文件下载。
