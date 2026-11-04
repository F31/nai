# V5.2 主组合与验收映射

2026-10-29；决策见 [ADR 0190](adr/0190-v52-primary-composition.md)。这是人工可审阅的主组合决策与映射，不是运行时已消费的 profile；不启动程序、不授予权限，不更改 `tests/registry.yaml`。

| 维度 | 决策 | 当前事实或状态 |
|---|---|---|
| 主机 | 单台 Windows、单人物、RTX 5090 Laptop | 本机 GPU 实测 24463 MiB，驱动 610.62；适配器 LUID/功耗策略尚未盘点 |
| 工作台 | Tauri 2、React、TypeScript | 现有代码使用 React；不迁移 Vue |
| LLM | 已配置的 API Provider | 历史真实测试由用户确认成功；当前健康与时延未复测 |
| TTS | 外部本地 Qwen 服务，通过现有适配器 | 历史真实合成可用；本轮已知 `127.0.0.1:8000` 未发现监听且只读请求超时，当前不可达。真实模型 ID、版本、采样、流式与取消未知 |
| 形象 | 路线 A：合法真人动作素材＋语音驱动口型；MuseTalk 1.5 为候选 | 实际权重/完整许可、真实推理与真人动作素材未验证；照片不证明动作能力 |
| 写实运行时 | 目标 NativeOnnx；实验 Python 单独隔离 | PyTorch CUDA 算子实验成功；不等于 MuseTalk 或 NativeOnnx 就绪 |
| 输出 | 独立 keeper、一个明确选择的 OBS 或伴侣 | 既有内置形象/麦克风/OBS 局部证据，不授予写实或平台认证 |
| 输入与平台 | 人工接管优先；公屏和 ASR 分别按实际权限启用 | 公屏、ASR 与平台真实授权尚未证实，不自动推流 |
| 部署 | Windows 本机；构建、模型研究与缓存放 E 盘 | 外部 Qwen 不自动安装、重启或复制 |

## 验收追踪映射

V5.2 左列是设计目标，未进入现行注册表；右列是相关既有 ACC，**仅为追踪，不表示等价或通过**。关联并不穷尽；新增与拆分需后续 schema、唯一 ID、适用条件、证据迁移及检查器共同落地。

| V5.2 目标 | 相关现有 ACC | 需新增或补足的证据 |
|---|---|---|
| ACC-PRIMARY-01 本机/Qwen 盘点 | ACC-ASM-02、ACC-PERF-01 | 设备身份、真实模型及服务能力、资源来源与未知值 |
| ACC-PRIMARY-02 Qwen 适配 | ACC-PERF-12、ACC-LIVE-04 | 首可播 PCM、声道/采样、取消与旧 epoch、服务失联 |
| ACC-PRIMARY-03 单路写实效果 | ACC-PERF-02/03/05/08/11、ACC-HAND-01 | 合法素材、真实口型、动作和衔接、OBS 录制与人工评价 |
| ACC-PRIMARY-04 整机认证 | ACC-PERF-11、ACC-PREVIEW-01 | Qwen＋Avatar＋实际直播软件整机峰值、热状态、2/8/24h |
| ACC-PRIMARY-05 文档单源 | ACC-ASM-01/03 | schema、默认源、生成器及一致性检查 |
| ACC-PRIMARY-06 客户验证 | ACC-UX-01/04、ACC-UE-01/02 | 可复现任务、实际用户、真实互动与复用 |
| ACC-PRIMARY-07 发布复核 | ACC-ASM-03、ACC-PERF-11 | 原始证据、版本摘要及独立具名复核 |
| ACC-PRIMARY-08 范围与分期 | ACC-ASM-03、ACC-ACT-08、ACC-CHANNEL-06 | 发布闭包、明确 Deferred 子项、失败分支与能力声明 |

## 本次盘点及测试

- 已校验 wheel SHA-256：`5174f02de8ca14df87c8e333c4c39cf3ce93a323c9d470d690301d110a053b3c`；PyTorch 2.7.1+cu128 安装在 E 盘研究 venv。
- NVIDIA GPU 计算能力 `(12, 0)`，PyTorch 显示 CUDA 12.8 可用，实际 1024×1024 全 1 矩阵乘法并同步后，结果为 1024；这只是设备算子验证。
- 测试时总可见内存 32947252 KiB、可用 6445368 KiB；GPU 已用 4420 MiB、59°C。这些只是时间点快照，不是 Qwen 归因或并发峰值。PyTorch 提示 NumPy 缺失，完整研究依赖尚未装齐。
- 过去曾对本地 Qwen 进行真实合成和 OBS 验证；本次没有调用 TTS，未重启服务，实际服务型号和运行框架未知。
