# V5.2 主组合、Qwen 盘点与 GPU 算子验证

状态：主组合决策和验收追踪映射已记录；仅 GPU 基础算子实测通过，Qwen 当前不可达，V5.2 注册表迁移与真实模型未完成。

- `Get-FileHash` 验证官方 wheel SHA-256 为 `5174f02de8ca14df87c8e333c4c39cf3ce93a323c9d470d690301d110a053b3c`。隔离研究环境在 `target-windows/realistic-avatar-research/venv`，安装 PyTorch 2.7.1+cu128 和所需基础 Python 依赖；未写入用户系统 Python。
- 实际 CUDA 检查：PyTorch CUDA 12.8、CUDA 可用、NVIDIA GeForce RTX 5090 Laptop、compute capability 12.0；1024×1024 GPU 全 1 矩阵相乘，CUDA 同步后结果为 1024，进程退出码 0。NumPy 尚未安装且导入提示警告，不影响该算子结果，不代表依赖闭包完成。
- `nvidia-smi` 快照：总显存 24463 MiB、已用 4420 MiB、驱动 610.62、温度 59°C；系统内存总 32947252 KiB、可用 6445368 KiB，不能推断 Qwen 资源或持续峰值。
- 本次只读端口检查未发现 8000 监听，`127.0.0.1:8000/v1/models` 请求连接超时；没有启动、停止或更改 Qwen，也没有请求 TTS。实际端点是否改变、模型身份、版本、流式、服务端取消和资源峰值未知；既往真实 TTS/OBS 证据保留，不代表当前服务在线。
- [主组合与追踪](../v52-primary-composition.md)、[ADR0190](../adr/0190-v52-primary-composition.md) 和 OpenSpec `2026-10-29-v52-primary-inventory` 记录 React 决策与新旧验收关系。新增追踪 ID 尚未纳入实际验收注册表；旧 164 ACC 状态不变。OpenSpec 严格校验通过并归档。

后续：在 Qwen 服务可用时核验真实接口与能力；锁定 MuseTalk 运行权重和传递许可、测试模型兼容与真实画面；以独立任务实现可执行 profile/schema、V5.2 注册表迁移和适用证据，不以 CUDA 算子结果宣称模型或直播就绪。
