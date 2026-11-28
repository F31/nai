# nai

当前按 **V5.6.1 单引擎基线** 推进：FlashHead Lite + Qwen3-TTS 1.7B CustomVoice + 预录动作片段 + 本机窗口/OBS。唯一在用桌面 UI 为 apps/workbench-desktop。V4.8/V5.2/V5.4/V5.5/V5.6 设计与 164 项 ACC 归档为历史，不再作为当前验收来源。见 [V5.6.1 基线索引](docs/v561-baseline.md)、[ADR 0193](docs/adr/0193-v561-single-engine-baseline.md) 与 [OpenSpec 变更](openspec/changes/v561-flashhead-single-engine/)。

智能体结合的直播数字人系统。首发面向 Windows 桌面与抖音，采用统一 Rust 内核、版本化能力契约与可替换适配器。

项目仓库：https://github.com/F31/nai

## 设计基线

- [V5.6.1 单引擎基线索引（当前目标）](docs/v561-baseline.md)
- [V5.6.1 OpenSpec 执行契约](openspec/changes/v561-flashhead-single-engine/)
- [V5.6 设计方案（历史输入）](docs/数字人直播工作台-V5.6设计方案.md)
- [V4.8 完整设计（历史归档）](docs/digital-host-agent-design-V4.8.md)
- [V4.3 完整设计（历史重构起点）](docs/digital-host-agent-design-V4.3.md)
- [V4.3 仓库重构设计](docs/architecture/v43-product-reconstruction.md)与[实施任务](docs/v43-reconstruction-plan.md)（ADR0147；RF01最小装配已实现，业务领域由RF02继续）
- [设计入口](docs/design.md)
- [开发任务与验收跟踪](docs/development-plan.md)
- [开发准备记录](docs/reports/2026-10-07-development-readiness.md)
- [项目命名与基线 ADR](docs/adr/0001-project-name-and-design-baseline.md)
- [工程与 OpenSpec SDD 流程](docs/engineering.md)

## 核心约束

- Lite / Standard / Pro 是同一产品的能力预设；Local / Hybrid / Distributed 是独立部署维度。
- LLM / TTS 使用 API；知识与 RAG 本地优先，并支持第三方知识 API。
- 视觉运行端为 FlashHead 单引擎；Rust engine + 独立 Python/PyTorch avatar worker，后续再评估 ONNX/TensorRT。
- 桌面控制台采用 Tauri 2 + React + TypeScript；媒体输出独立于 UI 生命周期。
- 以会话、epoch、停止/静音/接管和安全帧控制输出；所有队列有界。
- 单数据根、单操作员、单直播间；不做多工作区、审批、预算账本与审计哈希链。
- Local 不要求自建云主机；未启用的能力不启动或占用对应资源。

## 历史状态（V3.11/D14，已归档）

> 以下为历史快照，不代表 V5.6.1 当前状态；V5.6.1 见 [基线索引](docs/v561-baseline.md)。

2026-10-22：后续重构按V4.3备播—协作—复盘—下场复用实施，不保留旧代码/ABI兼容，保护现有用户资产。已安装Engineering版保留，旧验收不自动转为新ACC通过；详见[设计入口](docs/design.md)。下列D阶段文字仅为早期历史摘要。

已实现 D01–D11：workspace、契约生成、SQLite owner/迁移、事实权限、快照恢复、凭据 broker、
内核绑定、宿主/IPC、会话/epoch/媒体租约，以及有界调度、共享预算持久账本、API Provider 传输、
知识完整链路（导入/发布/冻结/远端门控）、智能体/守门评估内核和媒体样本时轴内核。
Linux/WSL 已实测存储、三语言互通、宿主端到端调用、Provider mock 传输、知识发布闭环、守门评估与媒体时序；
Windows 凭据/Named Pipe/沙箱/窗口音轨未实测。
另已实现 D12 模型包完整性前置检查与 D13 平台事件准入逻辑；真实模型推理、官方 SDK/网关尚待实现。
本轮 D14 本机控制工程服务与桌面客户端已接线：真实状态读取、静音/接管，完整预检/审批/配置仍待实现。
当前238项 workspace Rust测试与portable/生成无漂移通过。十类消息互通、5项客户端测试与真实控制进程冒烟为此前归档证据。
基线已升为 V3.11（ADR 0065；此前 ADR 0049 升 V3.10、ADR 0048 升 V3.9、ADR 0047 升 V3.8、ADR 0042 升 V3.7、ADR 0015 升 V3.6，其裁决继续有效）：验收唯一来源为 tests/registry.yaml（S/R/A/B/O/P/M/H/X + release_line（新增 capability_pack），D01–D14 映射回填），
`cargo xtask check --registry` 通过。决策日志哈希链/原子冲刷与值守门控已实现实测，真实ACK/root定时与媒体进程接线仍待W05/W06/W10。
真实 Provider 冒烟验证传输/鉴权/错误映射；账户无余额，付费生成 Blocked。
真实音轨/媒体进程静音、实际业务插件及 Tauri 原生宿主仍待后续任务。

> 状态快照提示：以上为早期（D14 轮）摘要，已归档；V5.6.1 当前进度与验收以
> [V5.6.1 基线索引](docs/v561-baseline.md) 与 `tests/v561/registry.yaml` 为准。

```bash
pnpm install --frozen-lockfile
cargo check --locked
cargo xtask check-deps
pnpm typecheck
pnpm build
pnpm spec:validate
pnpm check:frontend
node scripts/check-v561-registry.mjs
```

契约检查（需安装匹配工具，详见 [契约指南](docs/contracts/README.md)）：

```bash
cargo xtask schema --check
cargo xtask test --suite contracts
cargo xtask test --suite storage
cargo xtask test --suite kernel
cargo xtask test --suite hosts
cargo xtask test --suite session
cargo xtask test --suite scheduler
cargo xtask test --suite provider
cargo xtask test --suite knowledge
```

采用 OpenSpec SDD，按 proposal/specs/design/tasks → 实施 → 验证 → 归档推进。
详细记录见 [D01 报告](docs/reports/2026-10-07-d01-engineering-foundation.md) 和
[D02 报告](docs/reports/2026-10-07-d02-versioned-contracts.md) 与
[D03 报告](docs/reports/2026-10-07-d03-storage-and-credentials.md) 与
[D04 报告](docs/reports/2026-10-07-d04-kernel-registration-and-binding.md) 与
[D05 报告](docs/reports/2026-10-07-d05-plugin-hosts-and-ipc.md) 与
[D06 报告](docs/reports/2026-10-07-d06-session-leases-and-cancellation.md) 与
[D07 报告](docs/reports/2026-10-07-d07-scheduling-and-budgets.md) 与
[D08 报告](docs/reports/2026-10-07-d08-api-provider.md) 与
[D09 完整报告](docs/reports/2026-10-08-d09-knowledge.md) 与
[D10 智能体/守门报告](docs/reports/2026-10-08-d10-agent-and-guard.md) 与
[D11 媒体基础报告](docs/reports/2026-10-08-d11-media-basics.md)、
[D12 模型完整性报告](docs/reports/2026-10-08-d12-model-admission.md) 与
[D13 平台事件入口报告](docs/reports/2026-10-08-d13-platform-ingress.md) 与
[D14 本机控制报告](docs/reports/2026-10-08-d14-local-control.md) 与
[D15 验收注册表报告](docs/reports/2026-10-08-d15-admission-registry.md)。本机操作步骤见 [控制指南](docs/control.md)。

开发按 D01–D19 的依赖顺序推进。Windows 输出、真实 Provider、官方平台接入、写实模型和升级认证均需分别提供实际测试结果；规划与文档不代表认证通过。

