# FlashHead 单引擎精简基线（V5.6.1）

## Why

V5.6 的模型选型方向正确：用 SoulX-FlashHead 1.3B 替换 MuseTalk+LivePortrait，消除了双嘴、接缝和嘴部 owner 仲裁。但 V5.6 仍保留 LiveAct 18B、双引擎冷切换、热备、过渡画面、EchoMimicV3 片段工坊、LLM 动作标签和 8 小时基线，对当前「单人、单机、单形象、先跑通」的阶段过重。

当前 nai 仍是 V4.8 执行基线，164 项 ACC 全部 pending，公共输出为 `builtin-2d`，没有生产写实渲染器，Qwen 当前不可达。继续扩展双引擎不会让第一条真实可播链路更快出现，反而会推迟它。

本变更建立 V5.6.1 新基线：**FlashHead Lite 单引擎 + Qwen3-TTS 1.7B CustomVoice + 预录动作片段 + 本机预览/OBS + 单操作员控制**。不向前兼容 V4.8/V5.2/V5.5/V5.6 的 ACC、INV、任务编号、数据库或配置。

## What Changes

- 建立 V5.6.1 自包含基线：本变更的 proposal、design、specs、tasks 与验收独立，不再依赖「沿用 V5.5/V5.4」。
- 视觉只保留单引擎：FlashHead Lite 默认；Pro 仅在目标机实测通过后作为画质档；一个会话只加载一个视觉 worker。
- 删除双引擎相关机制：LiveAct 主线、冷切换、热备、过渡切换画面、切换回滚、双 venv 切换。
- LiveAct 18B 移出主线；仅在独立实验分支中保留资源/许可验证，不进本变更验收。
- 当前联合验证采用已部署的 Qwen3-TTS 1.7B CustomVoice；0.6B 为按需优化选项，其能力与性能需单独实测。
- 动作采用预录片段库 + 按钮/关键词触发；LLM 动作标签默认关闭；不做任意生成动作。
- 输入保留通用 HTTP 事件入口（弹幕/礼物），不内置平台官方连接器。
- 输出保留本机预览窗口 + OBS 窗口捕获 + OBS 录制；不做平台自动开播、RTMP 主线、虚拟摄像头主线。
- 数据使用单数据根、单配置、单 SQLite 库的最小 schema；不建工作区、角色、审批、预算、账本、outbox、审计哈希链。
- 验收从 164 项改为约 30 项分层 ACC，按 M0 目标机基线、M1 30 分钟、M2 2 小时、M3 4/8 小时推进。
- 复用现有 Rust engine、Tauri/React UI、SQLite 适配、输出门/安全帧/共享内存；替换 avatar worker；归档旧 2D/MuseTalk/LivePortrait 双模型路径。

## Impact

### 受影响能力

- 安装与目标机资源基线；
- Qwen3-TTS 合成与取消；
- FlashHead 形象渲染与口型；
- 媒体时钟、共享内存、安全帧；
- 直播会话、讲解、问答、打断；
- 弹幕/礼物通用入口、预录动作片段；
- 本机预览、OBS、录制、本场报告；
- 可靠性、看门狗、降级；
- 易用性、热键、许可确认。

### 删除/延期

- 删除：LiveAct 主线、双引擎切换、热备、过渡切换、切换回滚、双 venv 切换、LLM 动作标签、RTMP 主线、虚拟摄像头主线、8 小时硬基线。
- 延期：LiveAct 实验、EchoMimicV3 片段工坊、Pro 画质档、ASR/VAD、平台官方连接器、多工作区/多用户/审批/账本。

### 依赖与成本

- 依赖：Qwen3-TTS 本地服务、SoulX-FlashHead 权重/代码、OBS、ffmpeg、Python/PyTorch worker、Rust engine、Tauri/React。
- 新增成本：FlashHead 权重与 venv 打包；目标机资源基线脚本；共享内存帧协议；许可核查。
- 不新增常驻服务、云依赖、平台 connector、第二视觉引擎。

### 不兼容声明

- 旧 V4.8/V5.2/V5.5/V5.6 的 ACC/INV/任务编号不继承；旧 `tests/registry.yaml` 归档只读。
- 旧数据库不自动迁移；旧用户资产只支持手动导入（肖像、参考音频、知识文档）。
- 旧配置/协议/ABI 不兼容。
- 旧 2D/MuseTalk/LivePortrait 代码路径不保证保留；只保留必要的降级与输出基础。

## 基线映射

- 本变更不映射 V48-01–V48-15，不继承 ACC/INV；V4.8 注册表保持归档，不用于本变更通过。
- 对应设计输入：`docs/数字人直播工作台-V5.6设计方案.md` 与 `docs/reports/2026-11-04-v56-review-and-recommendations.md`。
- 本变更是执行契约；V5.6/V5.5/V5.4 设计作为历史参考，不参与发布门。
- 旧任务/报告状态不授予本变更任何通过。

## 非目标

- 多工作区、多用户、多角色、审批、预算账本、审计哈希链；
- 官方抖音/快手/视频号连接器、自动开播、commerce；
- LiveAct 主线、双引擎并发、热备、任意手势生成、3D、全身、舞蹈；
- 云渲染、云 TTS、MCP/A2A、Marketplace、移动端、远程控制；
- 24 小时发布门、商业 PMF 声明。

## 风险

- FlashHead 代码 Apache-2.0，但权重/VAE 许可需单独核查，不能声明整栈 Apache-2.0。
- FlashHead Pro 在 5090 Laptop 上未必达到 15–17 FPS；Lite 才是安全默认。
- 现有 1.7B 服务实测非流式、首句可能 4–15s；0.6B 服务能力需重新实测，不可达时只能文字模式。
- 24GB VRAM 与 32GB RAM 对 FlashHead Lite 足够，但对任何第二视觉引擎都不安全。
- 8 小时长稳受笔记本 TGP/散热限制；必须 30 分钟→2 小时→4/8 小时递进。
- 当前代码与 V5.6 设计存在差距，必须先做复用/删除映射，避免新旧路径并存。
