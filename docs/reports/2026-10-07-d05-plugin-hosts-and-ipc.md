# D05 插件宿主与 IPC 边界

## 状态与范围

- OpenSpec：`d05-plugin-hosts-and-ipc`；关联 ADR 0006。
- 已实现：SDK 有界 frame 编解码、握手协商/nonce 回显、IdentityValidator、AccessPolicy、
  CapabilityHost（serve/取消前置/ACL 分派）、Plugin trait/PluginContext/CapabilityClient、
  加载守卫、env 密钥清理与 NativeBoundary。
- Linux/WSL 实测：hosts+SDK 23 项测试通过；全 workspace 42 项 Rust 测试通过。
- Windows GNU 的 hosts/SDK 及测试代码交叉检查通过；不是原生执行或 MSVC 安装认证。
- Windows 命名管道 peer PID、真实 Named Pipe、AppContainer/Job Object、受限身份与
  文件/网络/凭据限制：**Blocked，当前无 Windows 执行环境**。
- D06 异步取消/流式调度、真实本地总线（Named Pipe/Unix socket 集成）与 D14 桌面接线未实现。
- 本轮未提交/推送，未调用付费 API，未保存生产凭据/观众数据。

## 修改目录

`crates/nai-plugin-sdk`（frame/handshake/plugin）、`crates/nai-plugin-hosts`
（acl/boundary/host + e2e）、Cargo workspace、xtask/CI（hosts suite）、
ADR 0006、hosts 指南、开发计划、报告、README/AGENTS 与 OpenSpec 工件。

## 关键行为

- 帧 = 4 字节小端长度 + body，64KiB 上限分配前校验；partial/coalesced/EOF/恶意长度
  在测试中覆盖，携带期限与共享取消。
- 首帧必须是 Handshake；身份材料经 host 提供的 IdentityValidator 校验，可选 OS peer
  凭据（Unix peer_cred）；协商版本交集并回显 nonce。握手后所有帧为 Envelope。
- ACL 以 session/principal/capability/operation 精确授权，分派前检查；空策略默认拒绝。
- Plugin trait object-safe（describe/validate_config/start/health/suspend/resume/stop/
  invoke）；PluginContext 只暴露 host 注入的期限/取消/预算/获准配置/credential refs。
- 取消记录 request_id，后续匹配调用分派前拒绝 CANCELLED；同步 invoke 不抢占进行中计算。
- 加载守卫规范化后在根内，拒绝 `..`/绝对逃逸/符号链接逃逸；env 清理移除密钥类变量。
- NativeBoundary 在无认证 OS 沙箱证据时返回显式 Unavailable，不以同用户进程冒充沙箱。

## 环境与锁定

Rust 1.96.0、Node 22.23.1、pnpm 8.15.0、OpenSpec 1.14.1。
新增依赖：prost（SDK 直接使用）。依赖由 Cargo 重新锁定。

## 实际运行与结果

| 命令 | 结果 |
|---|---|
| `cargo xtask test --suite hosts` | hosts 9 项（6 lib + 3 e2e）+ SDK 14 项测试通过 |
| `cargo xtask check --profile portable` | 7 包依赖边界、fmt、clippy、42 项 Rust 测试通过 |
| `cargo check --locked -p nai-plugin-hosts --tests --target x86_64-pc-windows-gnu` | Windows 分支/测试代码交叉检查通过；未执行 |
| `pnpm typecheck`、`pnpm build` | desktop/contracts 均通过 |
| `OPENSPEC_TELEMETRY=0 pnpm spec:validate` | D05 变更及已有规格严格校验通过 |
| `pnpm exec openspec archive d05-plugin-hosts-and-ipc --yes`（遥测关闭） | 9/9 任务完成，合并 4 条需求为 `plugin-hosts-and-ipc` 主规格并归档 |
| 归档后 `pnpm spec:validate`、`openspec validate --archived --no-interactive`（遥测关闭） | 7 个主规格与 D01–D05 五项归档全部通过 |

归档：[D05 proposal/specs/design/tasks](../../openspec/changes/archive/2026-10-07-d05-plugin-hosts-and-ipc/)。
新增主规格：`openspec/specs/plugin-hosts-and-ipc/spec.md`。

## 测试覆盖

SDK（14 项）：frame roundtrip/超长/恶意长度/truncated/coalesced/partial/期限/取消、
握手成功/拒绝/版本不匹配、PluginContext 取消/期限/仅 host 注入值、manifest 3 项。

hosts（9 项）：ACL 精确/作用域/撤销/空策略拒绝、加载守卫穿越/绝对/符号链接逃逸、
env 清理、NativeBoundary Unavailable、TcpStream 端到端（握手+ACL+invoke+取消前置）、
ACL 拒绝不进入实现、未知消息类型关闭连接。

## 失败与修复

- 首次 SDK 测试发现 frame roundtrip 断言长度写错（应为 5），修正。
- handshake accept 返回 (major, minor)，host 需要 principal；改为返回 (principal, major, minor)。
- `Arc<dyn Plugin>` 不满足 Send/Sync，e2e 线程失败；Plugin trait 增加 Send+Sync 约束。
- e2e 测试因服务器阻塞在 TCP read 而挂起；测试改为显式 `shutdown(Write)` 再 drop，服务器
  干净读到 EOF。真实生产本地总线仍应由 D14/D15 提供并设置读超时。
- clippy 全量扫描清理：unused import、冗余 mut、single-match、too_many_arguments。

## 迁移、维护与成本边界

无持久化迁移。新增 crate 与 CLI suite；契约未变，无消费者破坏。
新增 SDK/hosts 为 std+contracts 依赖，无常驻服务、云资源、模型下载或 API 使用。
NativeBoundary 始终拒绝子进程是诚实默认，认证沙箱（AppContainer/受限身份）由 D13/D18 集成。
Windows 原生验收、设备/SDK/GPU 与受限身份文件/网络/凭据限制仍未实测。

下一项：**D06 会话/租约/取消**，独立 OpenSpec 变更实现 epoch、幂等、独立静音与故障测试。
