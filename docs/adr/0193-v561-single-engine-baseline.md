# ADR 0193：V5.6.1 单引擎精简基线

日期：2026-11-04。状态：Accepted。实施契约：`openspec/changes/v561-flashhead-single-engine/`。

用户要求按 V5.6 审阅建议继续收敛：保留 SoulX-FlashHead 单引擎，删除 LiveAct 主线、双引擎冷切换、热备、过渡切换和 8 小时硬基线；不向前兼容 V4.8/V5.2/V5.5/V5.6 的 ACC、INV、任务编号、数据库、配置或协议。

## 决策

1. 视觉只加载一个引擎：FlashHead Lite 默认；Pro 仅在目标机 M0 实测通过后作为可选画质档。
2. LiveAct 18B 移出主线。未取得书面许可、未通过目标机资源基线前，不打包、不分发、不进入验收；仅允许隔离研究。
3. 不实现运行时引擎切换。换引擎在开始直播前通过重启/重新加载完成；预录动作片段满足常见手势需求。
4. TTS 默认 Qwen3-TTS 0.6B；先探测真实流式/取消/采样率，流式不可用时退化为句级合成 + 预取。
5. 动作使用授权预录片段 + 按钮/关键词；LLM 动作标签默认关闭；不做任意动作/全身/舞蹈。
6. 输入使用通用 HTTP 事件入口；不内置平台官方连接器，不自动开播平台。
7. 输出使用本机窗口 + OBS 窗口捕获/录制；RTMP、虚拟摄像头、平台 connector 不进主线。
8. 数据使用单数据根、单 `config.json`、单 SQLite 最小 schema；不建工作区、角色、审批、预算、账本、outbox、审计哈希链。
9. 复用现有 Rust engine、Tauri/React UI、SQLite 适配、输出门/安全帧/共享内存；替换 avatar worker；归档旧 2D/MuseTalk/LivePortrait 双模型与双引擎切换路径。
10. 验收改为约 30–34 项 V5.6.1 ACC，按 M0 目标机基线、M1 30 分钟、M2 2 小时、M3 4/8 小时推进。

## 不兼容与归档

- `tests/registry.yaml` 是 V4.8 历史注册表，只读，不再作为 V5.6.1 验收来源；V5.6.1 使用 `tests/v561/registry.yaml`。
- V4.8/V5.2/V5.4/V5.5/V5.6 设计与旧 OpenSpec 变更保持历史参考，不授予本基线通过。
- 旧数据库不自动迁移；用户资产只通过手动导入（肖像、参考音频、知识文档）。
- 旧配置/协议/ABI/形象包/动作包不保证兼容。

## 目标机基线门

- 目标机：Windows 11 + RTX 5090 Laptop 24GB + 32GB RAM。
- 先执行 `scripts/v561-baseline.ps1`：FlashHead Lite 10 分钟 + Qwen3-TTS + OBS，采集 VRAM/RAM/温度/功耗/FPS/首帧/首音。
- M0 未通过：停止功能开发，先定位模型/驱动/Qwen/散热。
- Pro 未通过：只保留 Lite，Pro 标为不可用/实验。
- 8 小时不是初始基线；先 30 分钟、2 小时，再 4/8 小时。

## 后果

- FlashHead Lite 成为唯一可发布的视觉默认；V5.6 的双引擎叙事作废。
- V5.6.1 ACC 与检查器成为当前验收入口；旧 164 ACC 只读。
- 任何恢复 LiveAct、双引擎、Pro 默认、RTMP、平台 connector 的改动必须新增 ADR 与 OpenSpec 变更，不能静默加入。
- 本 ADR 不授予任何模型许可、真实性能或平台能力通过；这些仍需目标机与书面许可证据。
