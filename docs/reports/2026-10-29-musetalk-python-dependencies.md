# MuseTalk 隔离 Python 依赖安装

状态：NumPy 缺失已解决，Python 主要依赖已安装；完整原生扩展及模型权重仍未就绪，不能运行完整推理或声明生产 Ready。

在 `target-windows/realistic-avatar-research/venv` 安装上游 requirements，含 NumPy 1.23.5、diffusers 0.30.2、transformers 4.39.2、librosa 0.11.0 和其他包；保留 torch 2.7.1+cu128，安装 torchvision 0.22.1+cu128。使用 Python 3.10，临时文件和 uv 缓存位于 E 盘；安装时未修改系统 Python 或 Qwen 服务。将构建工具固定为 pip 24.3.1 和 setuptools 69.5.1 后，chumpy 0.70 成功构建。

实测 `numpy.array` → `torch.from_numpy` → CUDA → 求和为 6；NumPy 1.23.5、Torch 2.7.1+cu128、torchvision 0.22.1+cu128、CUDA 可用。`uv pip check` 对已安装 157 个包报告兼容；这仅检查包元数据，不代表模型或二进制算子可用。安装了 mmengine 0.10.7、mmdet 3.1.0、mmpose 1.1.0、mmcv-lite 2.0.1，`mmpose.apis` 导入成功，但 `mmcv.ops` 真实导入失败：`ModuleNotFoundError: No module named 'mmcv._ext'`。mmcv-lite 不含 CUDA 原生算子，不能作为上游所需完整 mmcv 的通过证明。

本机工具包 nvcc 为 CUDA 13.1，PyTorch wheel 对应 CUDA 12.8；官方 OpenMMLab cu128/torch2.7 索引直连出现证书吊销检查离线错误，不能据此认定官方 wheel 不存在。尚未完成与本机/PyTorch 兼容的 mmcv 原生扩展安装、真实 DWPose 模型构建及 GPU 操作验证；禁止同时安装 lite/full 后宣称成功。官方资料：https://mmcv.readthedocs.io/en/latest/get_started/installation.html 。

已安装包快照保存在 `target-windows/realistic-avatar-research/installed-packages-20261029.txt`，是现状记录而非完整可复现的哈希锁。模型权重与各自许可、哈希尚未闭合，也未运行真实 MuseTalk、生成视频或更新正式工作台。下步查找可信匹配 wheel 或在 E 盘设置匹配 CUDA 12.8 构建工具进行有界编译，移除 lite 后安装完整 mmcv，并通过真实原生算子及模型加载检查；不禁用 TLS 校验绕过错误。

## 2026-10-30 原生扩展编译进展

仅隔离研究环境安装 NVIDIA 官方 `nvidia-cuda-nvcc-cu12==12.8.93` 与 `nvidia-cuda-runtime-cu12==12.8.90`，该 nvcc 包含 ptxas 但不含 Windows `nvcc.exe`，不能视作完整工具包。固定官方 MMCV v2.0.1 源码提交 `fc038a386a42d1f36d9ec49169d5817a52c70b8e` 到 E 盘；首次 CPU 扩展构建因上游 Windows 强制 C++14 与 Torch 2.7 头文件不兼容失败。隔离源码 setup.py 改 `/std:c++17` 后 CPU wheel 构建成功，日志 `target-windows/realistic-avatar-research/mmcv-cpu-cxx17-build.log`，wheel SHA-256 `377a68b21b36170b36281e166d16661ca6651a578229f1bff845a1f6d165effe`。隔离环境移除 mmcv-lite 后安装本地 CPU wheel，未替换系统 CUDA 或正式程序。

CPU NMS 测试的索引为 `[0, 2]`，MMPose API 导入成功；CUDA NMS 试运行明确失败，错误为 `implementation for device cuda:0 not found`，该包不含 GPU 原生算子。进一步对上游 DWPose 配置禁用预训练下载，用随机权重和空白图在 GPU 执行一次 `inference_topdown`，得到 `(1, 133, 2)` 形状；这仅是依赖/执行路径演练，不是人脸检测、真实权重、质量或性能验证，不能推断完整 MuseTalk 不需要其他 CUDA 算子。`uv pip check` 对159包通过，仅元数据兼容。

下一步仍须固定并取得 MuseTalk、VAE、Whisper、DWPose、人脸解析及检测权重，核查许可并用真实权重、授权图片和语音验证；任何需要 MMCV GPU 算子的路径保持不可用，不能将 CPU 扩展标成完整 CUDA 能力。
