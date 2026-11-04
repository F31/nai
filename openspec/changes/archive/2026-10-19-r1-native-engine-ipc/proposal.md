# Proposal

## Why

原生壳已能加载控制台，但没有到 engine 的控制通道，操作者不能在壳内使用已实现的候选与审批流程。接通私有继承管道与引擎监督，复用同一 ControlApi，完成 R1 桌面实际控制路径。

## What Changes

- 先扩展 Protobuf 控制契约并生成三语言消费者；消息有界、带版本、序号与期限。
- engine 新增显式 stdio 控制模式，不监听 HTTP；凭据只通过私有管道引导，业务权限与幂等处理复用现有入口。
- 原生壳从固定侧车路径启动 engine，托盘管理显示/退出，关窗隐藏，传输丢失失败关闭且不自动重启。
- 桌面新增原生 transport，保留浏览器显式 HTTP 测试适配。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `desktop-shell`: 私有控制通道、显式启动参数、托盘生命周期与断连处置。
- `local-control-surface`: 同一业务处理器支持继承管道与 HTTP 两种适配。

## Impact

对应 A12、S09、O02 与 V3.11 §10.1、§11、§29；沿用 R1 任务，不新增 D 编号。
新增 shared 层 nai-control-ipc 共享有界帧与子进程客户端，依赖 contracts/prost/getrandom（复用现有版本），无新服务。
原生壳增加 Tauri tray-icon 平台依赖；Linux 构建增加已安装的 AppIndicator 前置。Windows 认证保持未执行。
