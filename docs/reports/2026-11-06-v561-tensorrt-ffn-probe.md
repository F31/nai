# V5.6.1 TensorRT 真实 FFN 子模块试验

使用授权参考图、既有 Qwen 语音和 768／49 帧设置，运行真实 FlashHead 至首个去噪层，并捕获第一层 FFN 的实际输入 `[1,4032,1536]`、bf16。仅编译该层的 Linear／GELU／Linear，TensorRT workspace 限制 512 MiB，优化等级 0，禁用 TF32；验证实际存在 `TorchTensorRTModule`。未修改正式模型、播放或推流。

TensorRT 构建 26.125 秒；PyTorch 与 TensorRT 输出最大绝对差 0.5、平均绝对差约 0.00396，未预先建立此层的画质误差门槛，不能认定等价。PyTorch 5 次约 11.3–11.8 ms，TensorRT 5 次约 12.2–34.0 ms，存在明显波动；未证明加速。该对照的 PyTorch FFN 为 eager，未与已编译的整网路径严格同条件对照，且未控制 GPU 频率，不能据此推断完整模型性能。进程 GPU 峰值预留约 8.45 GiB，包含模型和其他中间张量，不等于引擎独占显存。

原始数据 `target-windows/flashhead-acceptance/trt-flashhead-ffn-20261106.jsonl`。本次只覆盖一个 FFN，输出差异原因尚未定位；不接入生产、不把构建成功当作质量或实时通过。后续需分析数值差异，比较原生 PyTorch 编译路径及更合理的 TensorRT 精度／分区配置；若无法证明有效收益，停止该路线并评估更高效模型或硬件。完整去噪、衣物质量、Qwen／OBS 并发及 M0 验收均未通过。
