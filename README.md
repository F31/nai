# nai

智能体结合的直播数字人系统。首发面向 Windows 桌面与抖音，采用统一 Rust 内核、版本化能力契约与按需插件。

项目仓库：https://github.com/F31/nai

## 设计基线

- [V3.9 完整设计（唯一实现基线）](docs/直播数字人%20%2B%20智能体系统技术方案%20V3.9.md)
- [设计入口](docs/design.md)
- [开发任务与验收跟踪](docs/development-plan.md)
- [开发准备记录](docs/reports/2026-10-07-development-readiness.md)
- [项目命名与基线 ADR](docs/adr/0001-project-name-and-design-baseline.md)
- [工程与 OpenSpec SDD 流程](docs/engineering.md)

## 核心约束

- Lite / Standard / Pro 是同一产品的能力预设；Local / Hybrid / Distributed 是独立部署维度。
- LLM / TTS 使用 API；知识与 RAG 本地优先，并支持第三方知识 API。
- 写实运行端使用 Rust + ONNX Runtime；Python 仅用于明确的离线模型制作与导出。
- 桌面控制台采用 Tauri 2 + React + TypeScript；媒体输出独立于 UI 生命周期。
- 以会话、epoch、事实版本、审批和播放租约控制输出；所有队列有界。
- Local 不要求自建云主机；未启用的能力不启动或占用对应资源。

## 当前状态

已实现 D01–D11：workspace、契约生成、SQLite owner/迁移、事实权限、快照恢复、凭据 broker、
内核绑定、宿主/IPC、会话/epoch/媒体租约，以及有界调度、共享预算持久账本、API Provider 传输、
知识完整链路（导入/发布/冻结/远端门控）、智能体/守门评估内核和媒体样本时轴内核。
Linux/WSL 已实测存储、三语言互通、宿主端到端调用、Provider mock 传输、知识发布闭环、守门评估与媒体时序；
Windows 凭据/Named Pipe/沙箱/窗口音轨未实测。
另已实现 D12 模型包完整性前置检查与 D13 平台事件准入逻辑；真实模型推理、官方 SDK/网关尚待实现。
本轮 D14 本机控制工程服务与桌面客户端已接线：真实状态读取、静音/接管，完整预检/审批/配置仍待实现。
当前238项 workspace Rust测试与portable/生成无漂移通过。十类消息互通、5项客户端测试与真实控制进程冒烟为此前归档证据。
基线已升为 V3.9（ADR 0048；此前 ADR 0047 升 V3.8、ADR 0042 升 V3.7、ADR 0015 升 V3.6，其裁决继续有效）：验收唯一来源为 tests/registry.yaml（S/R/A/B/O/P/M/H/X + release_line（新增 capability_pack），D01–D14 映射回填），
`cargo xtask check --registry` 通过。决策日志哈希链/原子冲刷与值守门控已实现实测，真实ACK/root定时与媒体进程接线仍待W05/W06/W10。
真实 Provider 冒烟验证传输/鉴权/错误映射；账户无余额，付费生成 Blocked。
真实音轨/媒体进程静音、实际业务插件及 Tauri 原生宿主仍待后续任务。

> 状态快照提示：以上「当前状态」为早期（D14 轮）摘要，已明显滞后；最新进度与数字以
> [开发任务与验收跟踪](docs/development-plan.md) 与 `tests/registry.yaml` 为准。

```bash
pnpm install --frozen-lockfile
cargo xtask check --profile portable
pnpm typecheck
pnpm build
pnpm spec:validate
pnpm --filter @nai/desktop test
pnpm test:control-smoke
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
