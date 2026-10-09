# Tasks

## 1. 规范与契约

- [x] 1.1 完成 D05 proposal/specs/design/tasks 并通过 strict 校验。
- [x] 1.2 SDK 增加 frame 编解码与 Plugin/PluginContext/CapabilityClient 语义，验证编译与边界测试。

## 2. 宿主与传输

- [x] 2.1 实现有界 framing 及 partial/coalesced/EOF/超长/恶意长度/期限/取消测试。
- [x] 2.2 实现握手协商、nonce 回显、身份校验与 ACL 先于分派；测试非法身份/拒绝/成功授权/未知消息类型。
- [x] 2.3 实现宿主生命周期（describe/validate/start/health/suspend/resume/stop）与取消记录；测试挂起/重启不继承授权及 CANCELLED 前置拒绝。
- [x] 2.4 实现加载守卫（穿越/绝对/符号链接逃逸）与 NativeBoundary（无认证沙箱返回 Unavailable）及 env 密钥清理；测试各拒绝路径。
- [x] 2.5 使用 UnixStream 对/TcpStream 回环做真实传输端到端握手与调用；记录 Windows 原生 peer/命名管道为 Blocked。

## 3. 验证与交付

- [x] 3.1 实现 hosts suite/CI；通过 SDK/hosts/contract 测试及 Windows 可用编译检查。
- [x] 3.2 更新 ADR/操作指南、开发计划、报告/README/AGENTS；运行 portable、前端与 OpenSpec 严格校验。

## Workflow follow-up

- 验证后归档合并主规格，并验证全部规格/归档任务。
