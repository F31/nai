## Why
V43-RF01要求从V4.3目标规划进入可构建工程：现有registry仍为V3.11、IPC仍为v1大帧和旧场景配置，无法作为新的呈现独立与能力状态真相源。需交付新源契约、完整ACC注册表、依赖门槛和真实独立引擎/桌面读取切片，而不是空目录或借旧测试宣布通过。
## What Changes
- schemas新增workbench配置、协议/状态/错误/证据及最小DeliveryUnit，生成Rust/TS/C++。
- 归档旧registry/检查器语料，新ACC注册表和检查器从V4.3精确对账，保留所有pending状态及RF01局部证据。
- 建立nai-runtime-core和nai-workbench-engine，私有继承stdio小端有界协议、短时进程凭据、版本/seq/TTL/配置验证，真实返回能力/Prepare缺口。
- 桌面新入口和窄宿主命令真实调用新引擎，不读取旧库或Key，不加载媒体/Provider。
## Capabilities
### New Capabilities
- `workbench-bootstrap`: V4.3契约、认证读取与依赖裁剪最小装配。
## Impact
V43-RF01；V4.3第6/18–20/28/35–39章，ACC-MODE/DEP/SEC部分前置、INV-11/23/26/27。不实施RF02存储/业务授权，不升级整项验收。无新常驻依赖、付费、用户数据迁移或安装替换。
