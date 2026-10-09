# Proposal

## Why

D04 是宿主和会话的前置条件。V3.5 第 3/4/21.2 章要求显式准入、依赖闭包与冻结绑定，当前内核仍只有入口，不能判定缺失与降级。

## What Changes

- 先增补版本/准入、绑定需求/诊断及版本化 provider 引用契约与配置 Schema。
- SDK 以同源清单验证版本/OS/架构，转换为生成注册 DTO；不赋予信任或授权。
- kernel 仅依赖 contracts，执行有界注册、循环检测和显式主备解析。
- 独立可信 owner 准入证据、健康和会话权限，未知/到期状态不视为可用。
- 生成 CapabilityView/冻结 BindingSnapshot，运行中不热换版本；撤销可立即使旧快照无效。
- 实现 NotReady/ReadyDegraded/Ready 及硬守门、形象 profile、自动模式前置条件的逻辑判定。
- 增加 kernel suite、配置/兼容回归及文档，具体宿主/设备/SDK仍未开始。

## Capabilities

### New Capabilities

- `capability-binding`：可信注册、闭包、显式选择、会话冻结和失效重检。

### Modified Capabilities

无；新增消息及 Binding 的可选版本字段，不删除已有 v1 字段。

## Impact

影响 schemas、生成产物、plugin-sdk/kernel、xtask/CI 与文档。SDK 增加离线清单校验/semver 开发和运行库。
无新增常驻服务或 API 调用。D04 Ready 仅绑定预检，不宣称宿主启动、最终模式授权或播放完成。
