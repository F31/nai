# 设计

## 决策摘要

后续同基线性能整改细化为[流式与缓存增量提案](../v561-streaming-cache-pipeline/proposal.md)，总体方案见[设计文档](../../../docs/v561-streaming-cache-optimization-plan.md)。该提案不授予本变更M0、生产帧/输出门或任何ACC通过，不自动更改模型默认和既有配置。

V5.6.1 采用**单视觉引擎**：FlashHead Lite 默认，FlashHead Pro 仅在目标机实测通过后开放。删除 LiveAct 主线、双引擎冷切换、热备、过渡画面、切换回滚。TTS 当前采用已部署的 Qwen3-TTS 1.7B CustomVoice（联合资源与性能待实测），0.6B 为可配置的后续优化选项；动作使用预录片段库；输出使用本机窗口 + OBS。数据为单数据根、单配置、单 SQLite。

本变更**不向前兼容** V4.8/V5.2/V5.5/V5.6 的 ACC/INV/任务/数据库/配置/协议。旧 `tests/registry.yaml` 归档只读，本变更建立自己的验收清单。

## 为什么选择 FlashHead Lite 作为唯一引擎

- FlashHead 是单模型端到端流式头肩生成，直接消除 MuseTalk+LivePortrait 的双嘴、接缝、ROI 对齐和嘴部 owner 仲裁。
- 官方 README：Lite 在单 RTX 4090 上 96 FPS，或 3 路 25+ FPS；Pro 在单 4090 上 10.8 FPS，双 5090 才 25+ FPS。
- 目标机是 5090 Laptop 24GB、TGP 95–175W，不能按桌面 5090 或双卡结果外推。Lite 是唯一适合作为 8 小时主力的档位；Pro 只能实测后开放。
- 24GB VRAM 预算：先前实测的 Qwen3-TTS 1.7B ~3.9GB（0.6B 待实测） + FlashHead Lite ~6.4–8GB + 输出/系统 ~3GB ≈ 13–15GB，留有安全余量。
- 不做第二视觉引擎：LiveAct 18B 需要 12–16GB VRAM + 可能的 ~30GB 系统内存 + 6 FPS + 插帧，且仓库无 LICENSE；它不属于“先跑通”的主线。

## 为什么不保留双引擎切换

- 冷切换耗时 A 档 ≤10s、B 档 ≤90s；单人直播中任何 10 秒以上的画面变化都会破坏节奏。
- 切换需要 drain、显存回落校验、过渡画面、失败回滚、双 venv、切换锁和切换 UI；这些复杂度只服务“短时手势表演”一个低频功能。
- 预录动作片段可以在 ≤300ms 内插播，成本远低于 LiveAct，且可审核、可复用、无许可风险。
- 因此本变更**不实现运行时切换**。切换引擎应在“开始直播前”通过重启应用/重新加载完成，不属于直播会话能力。

## 进程与依赖

```text
nai-desktop（Tauri 2 + React）
        │ 本地 HTTP
nai-engine（Rust：会话/LLM/互动/动作/输出门/日志/资源监控）
        │
        ├── qwen3-tts（外部本地服务，当前 1.7B 实测 24k WAV、非流式；0.6B 按需配置）
        ├── nai-avatar-worker（Python/PyTorch，FlashHead Lite）
        ├── assets/clips（预录动作片段）
        └── OBS（外部窗口捕获）
```

- 自研常驻进程：desktop、engine、avatar-worker。
- 外部依赖：Qwen3-TTS、OBS、ffmpeg。
- 不设第二 worker、切换控制器、热备、control-server、platform-bridge、supervisor。
- engine 内部保留轻量输出门：停止/静音/安全帧/epoch。

## 媒体管线

```text
文本 → 分句 → Qwen3-TTS WAV（非流式）→ 句级预取/缓存 → PCM
 → 48k 播放队列 → WASAPI（音频主时钟）
 → 16k 特征 → FlashHead worker → BGRA 帧 → 共享内存 → 输出门 → 预览/OBS
```

- 音频是主时钟；帧选择按已播放样本位置，不按墙钟。
- 帧/PCM 走共享内存环形缓冲，不走 JSON/Base64。
- 帧带 `epoch/seq/pts/source_sample`；旧 epoch 拒绝。
- 帧迟到 >200ms 显示安全帧；连续迟到/停止/失联不得继续旧帧。
- 停止/静音/接管最高优先级；停止不等待句子或动作结束。
- 输出画布默认 1280×720 @ 25fps；MVP 可先 960×540。
- 不在 engine 内重复编码；OBS 做窗口捕获。

## 数据与配置

- 单数据根，例如 `%LOCALAPPDATA%\nai-v561` 或用户指定。
- `config.json` + `nai.db` + `assets/` + `logs/` + `recordings/`。
- 最小表：`settings`、`characters`、`voices`、`sessions`、`messages`、`actions`、`events`、`clips`、`metrics`。
- 不建 workspace/principal/role/approval/budget/ledger/outbox/audit 表。
- 旧数据库不自动迁移；旧资产通过手动导入（肖像、参考音频、知识文档）。
- 配置示例见 `specs/setup-and-resources/spec.md` 与后续实现；不在本变更中继承旧配置。

## 复用与删除映射

| 现有资产 | 处置 |
|---|---|
| `apps/workbench-desktop`（Tauri/React） | 保留 UI 壳；删除多工作区/审批/预算入口 |
| `apps/workbench-engine`（Rust） | 保留会话/HTTP/组合根；新增 Qwen/FlashHead 适配 |
| `adapters/storage-sqlite` | 保留 SQLite；新建 V5.6.1 最小 schema；旧表归档 |
| `crates/nai-media-core` | 保留输出门/时钟/安全帧/共享内存；删除 MuseTalk+LivePortrait 双模型路径 |
| `apps/public-output-keeper` | 保留为输出门，或合并进 engine 以减少进程 |
| `crates/nai-kernel` | 只保留会话/停止/必要的守卫；不继承 62 INV |
| `crates/nai-contracts` | 只保留 V5.6.1 需要的消息；不铺全量 Protobuf/OpenAPI |
| `plugins/avatar-2d` / 旧 2D 路径 | 作为降级保留或归档 |
| `tests/registry.yaml` | 归档只读；新建 V5.6.1 ACC |
| `openspec/changes` 旧变更 | 归档；本变更新建 |
| V5.4/V5.5/V5.6 设计 | 合并/引用为历史；本变更自包含 |

## 性能与资源目标（分层）

| 阶段 | 目标 |
|---|---|
| M0 目标机基线 | FlashHead Lite 10 分钟：≥20 FPS；首帧 ≤1.5s；温度 <87°C；无 OOM；记录 VRAM/RAM |
| M1 可用 | 30 分钟：≥20 FPS；首音 ≤2.5s；AV 偏差 ≤120ms；停止 ≤200ms |
| M2 长稳 | 2 小时：无持续掉帧/泄漏/降频；温度曲线可接受 |
| M3 发布候选 | 4–8 小时：允许合理静默；记录失败；不承诺 24h |
| 可选 Pro | 仅在 M0 实测 ≥15–20 FPS 且温度可控时开放 |

- 数值是初始目标，不是供应商 SLA；必须目标机实测。
- 禁止用桌面 4090/5090 或双 5090 结果替代 5090 Laptop。
- 禁止用单模型 FPS 替代完整链路 FPS。

## 降级链

1. FlashHead Pro → Lite；
2. FlashHead Lite + 预录片段；
3. 静态形象 + TTS 音频 + 字幕；
4. 纯文字；
5. 停止。

每一步在 UI 明示；不静默降级。

## 失败与恢复

- worker 心跳 1s；3s 无响应切安全帧；重启最多 2 次/场；失败则降级/停止。
- Qwen 30s 健康检查；不可达→当前句失败/文字模式，不伪造音频。
- 音频设备丢失→静音，不自动切默认设备。
- 帧迟到/旧 epoch→安全帧/丢弃。
- engine 崩溃→launcher 重启；不自动恢复直播。
- 温度 ≥87°C 或 RAM 接近 28GB→提示/降级；不静默继续。

## 验证与环境边界

- 当前实施优先补齐 Windows 原生 FlashHead worker，复用已安装的 Windows CUDA/PyTorch；Qwen 保持现有 WSL 部署，通过配置的本地 API 接入。构建、下载及编译缓存放在项目 E 盘 target-windows；试验依赖隔离，不覆盖现有模型或 Qwen 环境。
- 实现检查：单元/契约/状态机/IPC/共享内存/取消/旧 epoch/故障注入。
- 真实认证：Windows 11 + 5090 Laptop + 24GB + 32GB；真实 Qwen3-TTS；真实 FlashHead 权重；真实 OBS 录像。
- 未在目标机验证的项记 `pending`/`blocked`，不标 `passed`。
- 性能/长稳必须记录设备、驱动、模型、参数、冷热、样本、负载、阈值。
- 本变更不运行 LiveAct；LiveAct 任何结果不进入本变更验收。

## 偏离与 ADR

隔离AV汇合入口 `nai-v561-av-clock-bridge` 放在app装配层，只用于验证共享槽/帧调度与独立音频签名时钟的同任务接线；诊断窗口与该装配控制同进程，不替代正式控制/媒体故障隔离。它显式沿用未认证的诊断帧来源，必须保持非生产标识；后续须完成生产者/素材身份和独立常驻窗口执行门，再接桌面。Rust媒体核心仍不依赖minifb/Tauri/具体模型，音频独立进程及Python模型边界不变。

单房间任务首切片放在 app 层 `nai-workbench-engine::v561_tasks`，装配 kernel OutputOwner 与 runtime Operation，不把 kernel 依赖 Adapter。只保存一个活动句级任务；网络调用在槽外运行，通过任务 Operation 取消。完成只收回任务槽，不能代表音画播完。停止句柄仅持有弱引用，原子关门不依赖任务槽锁；不接回历史 foundation engine/工作区链。桌面命令、媒体租约和跨进程执行独立接线。

- 本变更取代 V5.6 的双引擎主线；需要一份 ADR 记录「V5.6.1 单引擎基线、LiveAct 移出主线、无向前兼容」。
- 如果后续要恢复 LiveAct/双引擎/Pro 默认/RTMP/平台 connector，必须新增 ADR 与本变更规格修改，不能在实现中静默加入。
- 任何性能阈值调整、默认档位变化、数据根/数据库结构变化，需记录在 design/report 并按 OpenSpec 变更流程更新。
