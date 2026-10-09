# D10 智能体/守门评估实测报告

- 日期：2026-10-08
- 环境：Linux/WSL，Rust 1.96.0，锁定 pnpm/OpenSpec/protoc/libprotobuf
- 变更：`openspec/changes/d10-agent-and-guard/`（本轮归档）
- 结论：决策路由、四阶段守门、技能/记忆/节目评估已实现并实测；实际守门插件与真实平台事件未接入。

## 实现

`nai-kernel::evaluate`（agent/guard/skill/memory/show）纯逻辑，仅依赖 contracts/prost/serde_json，无 I/O。
agent 对 speak 候选校验并重算受控风险，无效候选降级 ask_human/stay_silent，无插件时按触发事件回退且不编造内容。
guard 四阶段：INPUT 注入/私密属性/越权、EVIDENCE 权限版本/来源版本/有效期/冲突、OUTPUT 金额单位/承诺/引用范围、
PREPLAY 审批/事实/epoch/权限/期限；URL 单次消毒重写，承诺与金额 block/escalate，重写上限 1。
skill 高风险拒绝注册、观众可见需 risk_2 与 output.viewer 权限、通知需 notify.* 权限、工具计划步骤/时长/受控参数。
memory 工作记忆只保留已播完、场次摘要确定性去重、观众长期记忆默认关闭且需作用域/用途/授权/保留/删除。
show 时长与打断政策有界、敏感场景需证据绑定、checkpoint 恢复必须显式版本化。

## 已执行检查

| 命令 | 结果 |
|---|---|
| `cargo test -p nai-kernel` | Passed：lib 42 项 + evaluate 集成 2 项 + scheduler 8 项 |
| `cargo xtask check --profile portable` | Passed：全 workspace 178 项 Rust 测试；11 包边界、fmt、Clippy `-D warnings` |
| `cargo xtask schema --check` | Passed：生成无漂移（本轮无 schema 变更） |
| `pnpm typecheck` / `pnpm build` | Passed：desktop 与 contracts-ts |
| `OPENSPEC_TELEMETRY=0 pnpm exec openspec validate d10-agent-and-guard --strict --no-interactive` | Passed |

测试覆盖：决策降级/受控风险重算/无插件回退、四阶段各自命中与单次重写后 pass、注入集与私密属性拒绝、
证据版本/权限/有效期、承诺与金额、preplay 各检查、技能高风险/观众/通知权限、工具计划注入拒绝、
记忆未播排除与观众默认关闭、节目敏感绑定与 checkpoint 版本化。

## 状态与边界

- **已实现/实测**：内核评估逻辑及 20 项新测试；通过 178 项全 workspace 与前端/生成检查。
- **未实现**：真实 LLM 决策/守门插件、技能执行器、节目执行器与记忆持久化消费；这些属于插件层/D11/D13。
- **Blocked**：无真实平台事件/媒体执行认证，不把评估逻辑当作端到端播放授权。
- **NotApplicable/未执行**：Windows 原生设备/SDK/GPU、真实直播输出；不将本轮评估当作 T/E/U 认证。

守门插件仍须调用本评估或等价更强保证；本轮未伪造真实插件已接入。
D10 活动变更已归档为 `openspec/changes/archive/2026-10-08-d10-agent-and-guard/`，
主规格 `agent-guard-evaluation` 生效。