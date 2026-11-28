# nai 设计入口

## 当前目标（V5.6.1，2026-11-08）

后续执行以 **V5.6.1 单引擎基线** 为准：[基线索引](v561-baseline.md)、[ADR 0193](adr/0193-v561-single-engine-baseline.md)、[OpenSpec 执行契约](../openspec/changes/v561-flashhead-single-engine/)。视觉为 FlashHead Lite 单引擎；当前语音部署按[ADR0195](adr/0195-cosyvoice3-current-tts-deployment.md)改为Fun-CosyVoice3-0.5B，Qwen已由用户移除；动作为预录片段，输出为本机窗口/OBS。不向前兼容 V4.8/V5.2/V5.5/V5.6 的 ACC/INV/数据库/配置。

当前验收唯一来源为 [tests/v561/registry.yaml](../tests/v561/registry.yaml)，共 34 项仍为pending；旧 [tests/registry.yaml](../tests/registry.yaml) 为 V4.8 只读归档，不再授予本基线通过。目标机M0尚未通过；早期Qwen组合见[历史基线状态](reports/2026-11-04-v561-baseline-status.md)，当前CosyVoice与FlashHead/OBS联合验收尚未完成。

V4.8/V5.2/V5.4/V5.5/V5.6 设计与旧 OpenSpec 变更保持历史参考，不拼接覆盖 V5.6.1。

2026-11-07性能整改：[文本/语音/视频流式与缓存整体方案](v561-streaming-cache-optimization-plan.md)、[CosyVoice当前任务](../openspec/changes/v561-cosyvoice-streaming/tasks.md)。Qwen对照不再执行；CosyVoice服务流式与每请求状态/单飞见[请求隔离报告](reports/2026-11-07-cosyvoice3-request-isolation.md)，最新Rust进展见下。方案/格式校验不代表功能或M0已通过。当前短片及M0失败尝试的边界见[真实音画报告](reports/2026-11-07-v561-real-window-independent-audio.md)。

最新Rust语音进展：C04独立设备边收边播、C05语音控制/统一取消及HTTP应用EOS已接通。单TTS短语义句/一句预取与插播归位见[报告](reports/2026-11-08-v561-one-ahead-narration.md)；新分句链路的认证设备时钟与帧调度出口见[时钟报告](reports/2026-11-08-v561-stream-device-clock.md)，独立协议2组合已实测，未拆开更新旧运行组合。当前仍为有限语音验证入口；完整直播桌面、公屏LLM问答、缓存身份、FlashHead真实帧/公共窗口联合输出与OBS未完成，不升级M0或34ACC。

## V4.3历史重构目标

此前重构产品/技术目标为 [digital-host-agent-design-V4.3.md](digital-host-agent-design-V4.3.md)，由 [ADR0147](adr/0147-v43-product-reconstruction-baseline.md) 记录采纳范围与仓库裁决。以真人主导的“备播—预演—协同直播—复盘—下场复用”为主线，不承担旧代码/ABI/场景配置兼容；工程标识仍为nai-*。

- [仓库重构设计](architecture/v43-product-reconstruction.md)：领域、进程、存储/契约、UI、回执与数据保护。
- [后续任务与第一张工程卡](v43-reconstruction-plan.md)：V43-RF01–12、依赖与里程碑。
- [V4.3全部ACC目标映射](acceptance/v43-target-matrix.md)：仅目标索引，不是通过记录。
- [本轮审视报告](reports/2026-10-22-v43-product-reconstruction-review.md)。

## 当前实现和证据边界

现有安装仍为V3.11演进的Engineering实现，未被新线替换；当前可操作能力与限制见[开发计划](development-plan.md)。RF01最小装配与RF02存储/私有交付基础已实现，用户备播/直播等完整业务目标尚未实现，不能把设计章节、历史测试数、单项设备或Provider证据当成新产品完成。

[tests/registry.yaml](../tests/registry.yaml)为V4.3唯一实际验收注册表，RF01已切换Schema/检查器和消费者，原表归档至tests/legacy/v311。90项ACC仍pending；[RF01报告](reports/2026-10-22-v43-rf01-contracts-and-bootstrap.md)中的前置证据不授予整项通过。

V3.11及此前方案、ADR和OpenSpec用于历史追溯，不自行拼接旧条款覆盖V4.3。现有库/Key/素材/许可保留，新数据根与凭据隔离；是否需要旧资产导入另按映射、预览、备份与授权执行。
