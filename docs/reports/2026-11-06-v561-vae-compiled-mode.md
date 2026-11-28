# V5.6.1 VAE 编译模式对照

## 范围和方法

在 E 盘隔离环境中，使用现有 LTX VAE 编码重复的授权参考图，取得形状为 `[128,7,24,24]` 的实际潜变量；它不是来自数字人主模型的生成潜变量。固定 768×768／49 帧／bf16，比较默认 `torch.compile` 解码与 `reduce-overhead` 解码。两种模式依次运行在同一进程，每种各先运行一次，再同步计时三次。GPU 为 RTX 5090 Laptop，PyTorch 2.7.1+cu128。

| 配置 | 首次运行（秒） | 后三次（秒） |
| --- | ---: | --- |
| 默认编译 | 30.513 | 0.524、0.533、0.545 |
| reduce-overhead | 10.952 | 0.936、0.517、0.513 |

输出形状均为 `[1,3,49,768,768]`，此次最大与平均绝对误差均为 0；进程 GPU 峰值预留约 5.80 GiB。候选后两次略快，但第一次稳定计时更慢，且顺序固定、只有三次、功率未控制；编译缓存不同，首次运行不可直接比较冷启动。**没有证据表明该候选在完整模型中有可靠收益，未改正式默认。**

报告数据位于 `target-windows/flashhead-acceptance/vae-compiled-mode-49-20261106.jsonl`。该测试不含主模型、音频、显示、OBS，也不能代表口型或衣物连续性质量。

## 后续

真实主路径已有 GPU 端 8 位转换后再转 CPU，不能假设直接减少传输字节能解决主要瓶颈。去噪模型和 VAE 原本已启用 PyTorch 编译，本机当前 Python 环境未发现 `torch_tensorrt` 或 `tensorrt`。下一步在确认目标 Windows/PyTorch/CUDA 组合与许可后，评估固定尺寸去噪子图的编译覆盖和数值误差；不安装不明二进制、不把部分算子回退当作加速，也不声称实时性已闭环。仍须在 GPU 稳定负载、真实 Qwen／OBS 并发和用户认可画质下验证。

## Torch-TensorRT 依赖核对

本机 Python 3.10.19／PyTorch 2.7.1+cu128 与 [Torch-TensorRT 2.7 官方发布说明](https://github.com/pytorch/TensorRT/releases/tag/v2.7.0)列出的版本范围相符；[PyTorch 官方包目录](https://download.pytorch.org/whl/cu128/torch-tensorrt/)列有 Windows cp310 版本。主 wheel 已下载到 E 盘隔离目录，SHA-256 `811c5f6f764902c602302eae1d58752360749d4d712725feb12fe9aa0172137a`；其元数据要求 TensorRT 10.9.x。版本范围相符不等于模型或 ABI 已通过运行验证。

PyPI 的运行库占位包未能下载真实 wheel；随后直接从 NVIDIA 官方源下载 1.6 GB Windows 运行库，传输未正常完成并报 SHA-256 不匹配，因此未安装或使用该文件，现有 PyTorch 环境未改动。下一步需要恢复可靠下载并验证官方哈希后，才能在隔离路径检查依赖、导入和算子覆盖。
