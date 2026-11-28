# V5.6.1 单引擎基线索引

日期：2026-11-07。状态：进行中。ADR0193为单视觉引擎基线；当前TTS部署按[ADR0195](adr/0195-cosyvoice3-current-tts-deployment.md)替代ADR0194的Qwen选择。执行契约：`openspec/changes/v561-flashhead-single-engine/`及当前CosyVoice增量。

## 基线

- 视觉：SoulX-FlashHead Lite 单引擎（Pro 仅目标机实测后可选）。
- 语音：当前部署Fun-CosyVoice3-0.5B，24k PCM流式；Rust独立设备边收边播及独立语音控制页已实测。Qwen已由用户移除，其适配/探测记录只作历史，不再作为当前联测前提。
- 动作：授权预录片段 + 按钮/关键词；LLM 动作标签默认关闭。
- 输出：本机窗口 + OBS 捕获/录制。
- 数据：单数据根、单 `config.json`、单 SQLite 最小 schema。
- 操作：单人、单机、单直播间、单当前形象。
- 明确不做：LiveAct 主线、双引擎切换、热备、多工作区、审批、预算账本、审计哈希链、平台官方 connector、RTMP/虚拟摄像头主线、3D/全身。

## 关键文件

| 内容 | 路径 |
|---|---|
| ADR | `docs/adr/0193-v561-single-engine-baseline.md` |
| OpenSpec 变更 | `openspec/changes/v561-flashhead-single-engine/` |
| 验收注册表 | `tests/v561/registry.yaml` |
| 验收 Schema | `schemas/tests/v561-registry.schema.json` |
| 验收检查器 | `scripts/check-v561-registry.mjs` |
| 代码复用/删除映射 | `docs/architecture/v561-code-reuse-and-removal-map.md` |
| 许可证门 | `docs/v561-license-gate.md` |
| 许可证矩阵 | `docs/v561-license-matrix.yaml` |
| Qwen3-TTS 适配/探测 | `adapters/llm-openai/src/qwen.rs`、`apps/workbench-engine/src/bin/v561_tts_probe.rs` |
| Qwen 实测报告 | `docs/reports/2026-11-04-qwen3-tts-capability.md` |
| 目标机基线脚本 | `scripts/v561-baseline.ps1` |
| 目标机基线状态 | `docs/reports/2026-11-04-v561-baseline-status.md` |
| V5.6 审阅与建议 | `docs/reports/2026-11-04-v56-review-and-recommendations.md` |

## 任务状态

- 0.1 新基线与 V5.6.1 ACC：已完成（旧 `tests/registry.yaml` 标记只读归档）。
- 0.2 ADR：已完成（ADR 0193）。
- 0.3 复用/删除映射：已完成。
- 0.4 许可证门：已完成（当前阻塞项明确）。
- 0.5 目标机基线脚本：已完成（脚本已写，未执行）。
- 0.6/0.7 目标机 M0 Go/No-Go：**Blocked**；本机目标硬件已确认，模型及联合运行尚未就绪。
- 0.8 文档更新：本页 + README/design/development-plan/OpenSpec 配置。
- 0.9 旧 OpenSpec/ACC 归档索引：旧 `tests/registry.yaml` 只读；旧 OpenSpec 变更保持历史，不参与 V5.6.1 发布门。
- 0.10 OpenSpec 配置：已更新为 V5.6.1。
- 1.1 单数据根：已完成（`adapters/storage-sqlite/src/v561.rs`，7 项测试）。
- 1.2 最小 SQLite schema：已完成（10 表，无 workspace/审批/预算/账本）。
- 1.3a 启动器：未完成；`scripts/v561-start.ps1` 在新引擎及桌面装配完成前拒绝启动。
- 1.3b 安装包/venv/模型下载：待许可与权重来源确认。
- 1.4 依赖探测：部分实现；模型完整性及音频设备尚未验证。
- 1.5 配置脱敏/凭据：已完成（redact/reset/禁止明文；keyring 模块）。
- 1.6 旧资产导入：已完成（肖像/音频/知识，哈希，无旧许可导入）。
- 1.7 卸载/重装：已完成（`scripts/v561-uninstall.ps1`、`scripts/v561-reinstall.ps1`）；数据根默认保留。
- 第二批旧 Rust 树清理：已完成（workspace 26 → 18 包；见 `docs/reports/2026-11-04-v561-legacy-prune.md`）。

## 归档策略

- V4.8/V5.2/V5.4/V5.5/V5.6 设计与 ACC 不沿用、不授予本基线通过。
- 旧 `tests/registry.yaml` 保留历史；V5.6.1 唯一验收来源是 `tests/v561/registry.yaml`。
- 旧数据不自动迁移；用户资产通过手动导入（肖像、参考音频、知识文档）。
- 任何恢复 LiveAct/双引擎/平台 connector 的改动必须新增 ADR 与 OpenSpec 变更。

## 下一步

当前进展以[语音控制报告](reports/2026-11-07-v561-voice-desktop-control.md)及[CosyVoice任务清单](../openspec/changes/v561-cosyvoice-streaming/tasks.md)为准。下列早期Qwen步骤保留追溯，不继续执行。下一主线为应用EOS/身份缓存→FlashHead同源音画与OBS；完整直播桌面/单数据根装配及M0仍未闭环，独立语音入口不放行正式v561-start。

1. 在目标机运行 `scripts/v561-baseline.ps1`，完成 0.6/0.7。
2. 已完成数据根和 Qwen 部分适配；启动装配及完整依赖探测未完成；1.3b 待许可/权重来源；下一步 3.x FlashHead worker。
3. Qwen 1.7B CustomVoice 的 24k WAV 和非流式能力已实测；句级预取、播放接线、试听和完整指标仍待闭环，见 `docs/reports/2026-11-04-qwen3-tts-capability.md`。
4. 进入 3.x：FlashHead worker 协议与 Lite 集成。
