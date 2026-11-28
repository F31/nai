# FlashHead 资产核验与环境准备

用户已手动下载 FlashHead Lite、VAE_LTX 和 wav2vec2 到 `target-windows/models/`。四项 SHA-256 与官方模型库元数据完全一致：

- Lite `diffusion_pytorch_model.safetensors`：`aaf1cde6e80ca23f740aae236c47954249f65b151db133cc0f77d3a138ccdf6e`；
- VAE_LTX `diffusion_pytorch_model.safetensors`：`265ca87cb5dff5e37f924286e957324e282fe7710a952a7dafc0df43883e2010`；
- wav2vec2 `model.safetensors`：`8aa76ab2243c81747a1f832954586bc566090c83a0ac167df6f31f0fa917d74a`；
- wav2vec2 `pytorch_model.bin`：`c34f9827b034a1b9141dbf6f652f8a60eda61cdf5771c9e05bfa99033c92cd96`。

FlashHead 模型固定版本 `5fbd2e44ba08ab2deb28bd55cff2c15523e51ba5`，wav2vec2 官方查询版本 `22aad52d435eb6dbaf354bdad9b0da84ce7d6156`。模型文件验证不等于可加载或具备发布许可。

官方代码已检出到 `target-windows/flashhead-upstream`，commit `9bc03de06bb0de82cd6bc477804512ae06144bf2`。官方安装说明使用 Python 3.10 和 PyTorch 2.7.1/cu128，模型代码依赖 FlashAttention、xfuser、diffusers 等。现有 Qwen 使用 WSL Python 3.12 / PyTorch 2.14.1+cu130，相关依赖不齐，不应在该环境中直接升级或降级。Qwen 1.7B 服务仍健康。

E 盘独立 uv 0.9.24 和 Python 3.10.19 已安装，独立虚拟环境位于 `target-windows/flashhead-venv`；官方 PyTorch 2.7.1/cu128 及依赖下载和安装正在进行；FlashHead 依赖、模型加载、推理及 Qwen/OBS 联合基线均未通过，任务 0.19 保持未完成。

Python 3.10.19 已在 E 盘安装，独立虚拟环境已创建。首次官方 PyTorch/CUDA 安装在 NumPy 下载处超时，未报告成功；已使用 E 盘缓存、降低并发并延长超时重试，安装仍在进行。完成后还须执行实际 GPU 运算和 FlashHead 依赖检查。

WSL 现有 Qwen 环境独立执行一次 512×512 矩阵乘，RTX 5090 Laptop、PyTorch 2.14.1+cu130 返回 512，CUDA 通路可用；这不是隔离环境 PyTorch 2.7.1 的验证，也不是 FlashHead 联合性能证据。E 盘剩余约 98 GB。PyTorch 官方依赖重试仍在进行，未授予环境完成状态。

本轮复核：E 盘约剩 98 GiB。隔离环境 uv 下载多次重试，NumPy 已下载，其余 CUDA 组件仍未完成；进程仍在运行，不能视为 PyTorch 已安装。现有 Qwen 的 CUDA 小矩阵运算通过，仅证明当前 WSL/GPU 通路。
