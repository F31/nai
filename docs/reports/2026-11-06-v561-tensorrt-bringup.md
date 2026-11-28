# V5.6.1 TensorRT 隔离环境验证

用户手动续传 NVIDIA TensorRT 10.9.0.34 Windows 运行库；文件 1,627,286,840 字节，SHA-256 为 `e43d38cc380d615bf8afab637c265f01a9452dc23deb9b9dd47b7662edf24531`，与 NVIDIA 索引值一致。TensorRT 运行库、Python bindings、Python 封装和 PyTorch 官方 Torch-TensorRT 2.7.0+cu128 安装在 `target-windows/flashhead-win-diagnostic/tensorrt270-isolated`，未替换现有 PyTorch 或 Qwen。Python 3.10.19、PyTorch 2.7.1+cu128、TensorRT 10.9.0.34、Torch-TensorRT 2.7.0+cu128 导入成功，CUDA 可见 RTX 5090 Laptop。

独立 GPU 小模型（Linear／ReLU／Linear）首次默认设置实际生成 `TorchTensorRTModule`，与 PyTorch 比较最大绝对误差 0.000367，高于试验阈值 0.0001；保留失败结果。关闭 TF32 后的首次对照发现默认最小子图阈值使模型退回 PyTorch，即便配置要求完整编译也不能单靠参数判断。将最小子图阈值设为 1 后明确检查到 `TorchTensorRTModule`，最大绝对误差约 `2.09e-7`，`atol=rtol=1e-4` 测试通过。该测试仅证明基础 GPU 编译可运行，不表示 FlashHead 支持、画质、速度或完整实时已通过。

下载的源码封装构建过程中，pip 将两个小 wheel 写入了本次 Codex 沙箱的 C 盘临时缓存；主依赖、安装目录和后续 pip 缓存均在 E 盘。已在 `scripts/use-project-build-cache.ps1` 设置 `PIP_CACHE_DIR`，防止后续项目构建默认写入 C 盘。沙箱缓存位于工作区外，未尝试删除。

下一步对真实 FlashHead 去噪输入进行有界导出和分区检查，明确 SageAttention／自定义算子与 TensorRT 的兼容、回退和实际编译覆盖，再比较同一输入的数值与吞吐。不得将小模型通过当作数字人模型加速；当前正式配置、M0 及 OBS 联合验收状态不变。
