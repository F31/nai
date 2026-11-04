# 真人效果数字人方向

用户要求直接面向真人效果，原三层开闭嘴组包不再作为前置路线。V4.9 已将 MuseTalk 列为写实首选验证候选；此次核对 MuseTalk 官方项目的图像/视频与语音输入、实时示例及许可声明。官方声明主代码 MIT、模型可商用，但其他依赖模型及预处理组件需逐项核实，官方测试素材不作为生产素材。项目的实时数值不能当作本机实测结果。

只读 `nvidia-smi` 检测到 NVIDIA GeForce RTX 5090 Laptop GPU、24463 MiB 显存、驱动 610.62；未运行模型、未下载权重、未上传照片或修改业务数据。官方研究实现包含 Python，不能直接作为项目 Windows 正式运行包；模型运行方式须在隔离评估后选择并记录。真实效果、延迟、显存、许可证闭包和公共输出尚未验证。

活动变更 `v49-realistic-avatar` 明确依赖与许可审查、真实照片和音频验证、生产后端、独立媒体宿主、持久许可及撤权、真实 Windows/OBS 验收顺序。下一步是固定模型及依赖版本、查验许可与建立 E 盘隔离研究环境，再用已授权照片和本地语音实测，不使用伪造结果。164 ACC 不变。

## 官方源码与依赖核查

已在 `target-windows/realistic-avatar-research/MuseTalk` 获取官方源码并固定提交 `0a89dec45a0192b824e3cf4daf96c239440c5ed8`。源码目录为隔离研究用途，不纳入正式运行包。官方权重脚本包含 MuseTalk 1.0/1.5、SD VAE、Whisper、DWPose、SyncNet、人脸解析和 ResNet18；实际实时推理所需闭包仍须按加载路径逐项核实，不能把训练专用依赖一律视为运行需要。官方许可清单将主项目、VAE、Whisper 和 face-parsing 列为 MIT，DWPose 列为 Apache-2.0；仍须逐一核实具体权重、传递依赖及发布义务。

官方推荐 Python 3.10、PyTorch 2.0.1/CUDA 11.8；本机是 RTX 5090，PyTorch 官方 2.7 文档声明 CUDA 12.8 支持 Blackwell。较新 PyTorch 与上游旧版 OpenMMLab 依赖的兼容性尚未验证，不能直接判定可用。E 盘本次只读检查剩余约 20.4 GiB；完整权重和 CUDA 环境可能占用可观空间，尚未下载权重或安装运行环境。官方批量下载脚本会配置镜像并安装依赖，未执行；研究环境须固定来源与版本并限制缓存目录。

参考官方：[MuseTalk](https://github.com/TMElyralab/MuseTalk/tree/0a89dec45a0192b824e3cf4daf96c239440c5ed8)、[权重许可清单](https://huggingface.co/TMElyralab/MuseTalk/blob/main/LICENSE.txt)、[PyTorch 2.7](https://pytorch.org/blog/pytorch-2-7/)。

## 隔离环境准备进度

已通过 uv 在 `target-windows/realistic-avatar-research` 安装 Python 3.10.19 并创建独立虚拟环境，缓存与临时目录均指定到 E 盘。尝试安装 PyTorch 2.7.1 CUDA 12.8 官方 wheel，3 GiB torch 包下载/解包流中断并发生重试，已主动中止以避免持续占用网络和磁盘；PyTorch 未安装成功，尚未完成 GPU 运算验证。直接连接官方 Hugging Face 权重仓库超时，权重版本与摘要尚未固定、尚未下载。不得把该失败归因于某一代理设置。

源码追踪显示推理加载 VAE、UNet、Whisper、人脸解析与 DWPose，且人脸检测依赖还可能从外部地址自动下载 S3FD；正式执行前必须显式锁定并核验所有模型来源、权利和本地缓存位置，避免后台隐式下载。此次未运行推理、未读取照片、未调用语音模型。

## 禁用代理与续传（2026-10-29）

本次仅对下载进程使用 `--noproxy '*'`，未修改系统设置。PyTorch 官方索引直连 HTTP 200，Hugging Face 官方页面直连 10 秒连接超时；原因未确定。PyTorch 官方索引提供目标 wheel 的 SHA-256，已创建 `scripts/download-realistic-avatar-torch.ps1`，E 盘保留部分文件并支持续传、有限重试和下载后摘要校验。实际中断后续传从 103149568 字节继续，30 秒测试超时，文件仍未完整；尚未验证完整文件摘要或安装。实际速度波动较大，剩余时间不可可靠预估。真实模型及 GPU 推理仍未执行。
