# FlashHead Lite 联合基线准备

首轮采用现有 Qwen 1.7B，模型服务已运行；不修改已有数据或中断服务。尚无可运行的 FlashHead worker，未开展 M0。

[官方仓库](https://github.com/Soul-AILab/SoulX-FlashHead)列出 Python 3.10、PyTorch 2.7.1/cu128、FlashAttention、ffmpeg，以及 FlashHead 和 wav2vec2 权重。官方性能数据不能代替 5090 Laptop 本机测试。

[官方模型卡](https://huggingface.co/Soul-AILab/SoulX-FlashHead-1_3B)注明 transformer 权重 Apache-2.0，Lite 所需 VAE_LTX 为 OpenRAIL-M；依赖与再分发仍须完整核查。只读元数据查询固定版本 `5fbd2e44ba08ab2deb28bd55cff2c15523e51ba5`：

- `Model_Lite/diffusion_pytorch_model.safetensors`：6107542048 字节，SHA-256 `aaf1cde6e80ca23f740aae236c47954249f65b151db133cc0f77d3a138ccdf6e`。
- `VAE_LTX/diffusion_pytorch_model.safetensors`：1676798532 字节，SHA-256 `265ca87cb5dff5e37f924286e957324e282fe7710a952a7dafc0df43883e2010`。

两文件合计约 7.8 GB（十进制），另需配置和 wav2vec2。当前未下载、未校验或加载；按 OpenSpec 1.3b，权重来源及许可闭合前不自动下载。此前 Qwen 0.6B 的 E 盘断点缓存保留，但不阻塞首轮。

默认值调整的定向验证：存储 V5.6.1 测试 13 项通过（跳过 Windows 符号链接权限测试），Qwen 单元 4 项及本地适配集成 1 项通过。追加已有 0.6B 配置保持不变的断言后，首次编译内存不足，随后增量链接失败；关闭增量、单任务重编后该断言通过。OpenSpec 严格校验通过。
