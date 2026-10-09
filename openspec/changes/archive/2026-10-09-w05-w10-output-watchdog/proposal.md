## Why
W05/W10 缺少真实独立进程的失联租约与强杀证据。既有 MediaGate 只是进程内机制，不能将它的单测当作 keeper/engine 故障隔离。

## What Changes
- 新增显式 `nai-output-keeper --watchdog-drill <session>`，通过私有继承stdio接收Protobuf控制帧，独立10ms轮询MediaGate。
- 新增演练命令/状态契约，仅 lease arm/renew/inspect，不接收PCM、帧或生成硬件ACK。真实设备一律CapabilityNotReady。
- 有界输入队列、元数据最新值输出；读取/写入阻塞不阻塞租约检查，部分帧超时与断连拒绝后续控制。
- 用tests中的控制源进程做停续租、强杀和阻塞读取演练，另验证keeper强杀与显式重启不继承权限。

## Capabilities
### New Capabilities
- `output-watchdog-process`: 显式演练进程的有界控制与独立租约检查。
### Modified Capabilities
无。本轮不宣称完整output.deliver、R01/R03/S12或Slice-0通过。

## Impact
新增apps/output-keeper；复用contracts/media-core/plugin-sdk及锁定prost，测试使用tempfile和原生rustc编译tests专用源进程。无新第三方运行依赖、网络监听或自动启动服务。Local正式默认不变；窗口/音轨/P0独立通道/共享内存/W06尚待实施。
