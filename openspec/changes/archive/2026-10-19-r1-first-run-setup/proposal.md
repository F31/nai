# Proposal

## Why

原生壳已能通过命令行参数启动 engine，但「安装后无需命令行配置」仍未满足：数据根、工作区、场景与 Provider 凭据都只能从终端传入。操作者双击应用后无法完成首次配置并连接。

## What Changes

- 原生壳持久化**非机密**主机设置（工作区/身份/profile/场景/runtime profile），文件 0600、原子写。
- 新增首次设置命令：读取默认值、保存并初始化工作区、启动 engine；初始化复用 engine 的 `--provision`。
- Provider 凭据只在本次会话中由界面输入，作为环境变量注入派生的 engine，**不落盘、不进日志**；关闭应用后需重新输入。
- 设置面板接入现有原生控制流程；浏览器控制台保持显式 loopback 测试适配。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `desktop-shell`: 首次设置、非机密配置持久化、会话凭据注入与显式连接。
- `local-control-surface`: 设置命令与既有控制命令共用同一宿主边界，不新增业务判定。

## Impact

对应顺序 3「无需命令行配置」与 A12；V3.11 §11、§29、§900。新增 `serde`/`serde_json` 到壳的独立
workspace（复用根锁定版本），无新增常驻服务；数据根固定为应用数据目录，界面不再接受任意数据根。
