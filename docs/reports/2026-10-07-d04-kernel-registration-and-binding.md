# D04 内核注册、依赖闭包与冻结绑定

## 状态与范围

- OpenSpec：`d04-kernel-registration-and-binding`；关联 ADR 0005。
- 已实现：plugin-sdk 清单转换、Registry 准入/注册/健康/权限/自动条件、依赖闭包、
  显式同域主备、CapabilityView 与冻结 BindingSnapshot、live recheck。
- Linux/WSL 实测：18 项内核测试 + 3 项清单 SDK 测试通过；全 workspace 42 项 Rust 测试通过。
- Windows GNU 的 kernel 及测试代码交叉检查通过；不是原生执行或 MSVC/设备认证。
- 真实宿主、IPC 身份、验签/信任库、资源实测、D06 会话/租约及 D13 平台材料仍未实现；
  空生产 registry 默认 NotReady。
- 本轮未提交/推送，未调用付费 API，未保存生产凭据/观众数据。
- 测试中使用明确 `test-only` 准入/健康/授权材料；这些引用不代表真实认证档案。

## 修改目录

`schemas/core/capability.proto`、`schemas/config/binding-plan.schema.json`、
生成 Rust/TS/C++/TS JSON 类型、`crates/nai-kernel`、`crates/nai-plugin-sdk`、
契约 fixture/校验/互通、xtask/CI、README/AGENTS、工程/内核指南、开发计划、
ADR 0005、OpenSpec 工件与契约文档。

## 关键行为

- SDK 同源 Schema/semver/目标检查，64KiB 上限、deadline/cancellation；生成注册 DTO，
  不创建运行实例、不赋信任。
- Registry 只依赖 contracts+std，owner 管理接口不对未认证插件开放。
- 独立准入必须匹配 manifest/artifact/host 身份；inproc 要求可信 root 确认编译期实现。
- 声明图（manifest requires + 硬依赖）循环保守拒绝，失败不覆盖原注册表。
- 健康期限/来源与 session/plugin 权限独立输入，逐次复检；声明不等于授权。
- 唯一 provider 可默认选，多 provider 必须显式主备；备用限制在同一 policy scope，
  且需完整依赖闭包；失败候选暂存闭包被丢弃。
- output/tts 硬依赖 guard，avatar.render 硬依赖 profile/guard，emit_output 要求 output/guard。
- Auto 附加平台/知识能力，要求 `auto.supervised` 明确授权、监督期限和预算准入材料。
- Ready/Degraded 才可冻结；冻结全部闭包 selected 身份，运行中拒绝换版/移除，
  允许健康/权限/准入撤销使 recheck 失败，不静默切备用。
- Ready 仅绑定预检，不冒充宿主启动、模式租约、播放完成或设备/平台认证。

## 锁定环境

Rust 1.96.0、Node 22.23.1、pnpm 8.15.0、OpenSpec 1.14.1、semver 1.0.28。
依赖由 Cargo 重新锁定；SDK 新增 jsonschema/sha2（已用于 storage/契约）。

## 实际运行与结果

| 命令 | 结果 |
|---|---|
| `cargo xtask schema` / `--check` | 103 消息、binding-plan 类型生成，无漂移 |
| `cargo xtask test --suite kernel` | 18 内核 + 3 SDK 测试通过 |
| `cargo xtask check --profile portable` | 6 包依赖边界、fmt/clippy、42 项 Rust 测试通过 |
| `cargo xtask test --suite contracts` | 9 项 Node 校验、Rust/TS/C++ 编译及两条 8 类互通通过 |
| `pnpm typecheck`、`pnpm build` | desktop/contracts 均通过 |
| `OPENSPEC_TELEMETRY=0 pnpm spec:validate` | D04 变更及已有规格严格校验通过 |
| `cargo check --locked -p nai-kernel --tests --target x86_64-pc-windows-gnu` | Windows 分支/测试代码检查通过；未执行 |
| `pnpm exec openspec archive d04-kernel-registration-and-binding --yes`（遥测关闭） | 9/9 任务完成，合并 6 条需求为 `capability-binding` 主规格并归档 |
| 归档后 `pnpm spec:validate`、`openspec validate --archived --no-interactive`（遥测关闭） | 6 个主规格与 D01–D04 四项归档全部通过 |

归档：[D04 proposal/specs/design/tasks](../../openspec/changes/archive/2026-10-07-d04-kernel-registration-and-binding/)。
新增主规格：`openspec/specs/capability-binding/spec.md`。

## 测试覆盖

18 项内核测试：

1. 空注册表 NotReady 且不编造插件。
2. 缺独立准入/inproc 审查、身份/版本/宿主不匹配拒绝。
3. 目标 kernel/协议/正式契约拒绝。
4. 循环拒绝原子（不污染原注册表）。
5. 硬依赖循环不能被 manifest 隐藏。
6. 单 provider Ready 并解析完整传递闭包。
7. 可选缺失 Degraded 且不要求 LLM/知识。
8. output/avatar 的 guard/profile 前置。
9. 声明权限不授予，其他 session 不能继承。
10. 权限来源/到期逐次复检。
11. 健康缺失/失败/到期不能变 Ready。
12. 多 provider 必须显式选择，即使只有健康。
13. 显式备用及其完整闭包；14. 备用不跨 policy scope。
15. 冻结版本/不可变/阻止热换；16. 撤销使 recheck 失败但不重写选择。
17. Auto 权限/监督/预算材料必须独立证明。
18. 有界调用/规模拒绝不改变状态，超出 session 上限拒绝。

3 项 SDK 测试：有效清单不赋信任、未知字段/范围/目标/正式能力拒绝、
畸形/超限/到期/取消拒绝。

## 失败与修复

- 首次使用 JS descriptor 时 oneof/field presence 存在精度风险（D02 已改用 Rust 编码）；
  本轮新增枚举/身份字段后无同类问题。
- SDK 进程取消参数原先遗漏；补充 `&AtomicBool` 取消与 tests，符合有界调用要求。
- clippy 对 kernel 大闭包无新告警；`cargo fmt` 后完整检查通过。
- contracts 互通样本从 7 类扩到 8 类，加入版本化绑定报告，未破坏既有三语言路径。

## 迁移、维护与成本边界

无持久化迁移；Registry 为内存结构，D03 存储接线及 D05 宿主后续实现。
启动使用唯一 boot_id；重启不恢复旧绑定快照。
新增 sdk 开发/运行依赖，无常驻服务、云资源、模型下载或 API 使用。
Registry 规模上限是结构边界，不替代资源/设备/GPU 认证。
Windows 宿主、IPC 隔离、真实平台权限与设备仍未验收。

下一项：**D05 宿主与 IPC**，独立 OpenSpec 变更实现 framing/握手/ACL/生命周期及真实进程隔离。
