# Proposal

## Why

R1 交付顺序 3 的前端与演示包已经存在，但桌面壳只有配置声明，没有可执行程序。先交付能嵌入现有控制台的真实原生壳，消除「配置存在」与「能够构建启动」之间的断点。

## What Changes

- 添加独立 Cargo workspace 的 Tauri 2 壳，锁定依赖，复用现有前端与最小权限配置。
- 明确原生壳尚未接引擎受保护 IPC，在原生视图禁用 HTTP 测试连接，避免给出可操作假象。
- 提供真实前端内嵌构建及有界 WebView 加载冒烟；分开记录 Linux 构建与 Windows 安装认证。
- 记录 ADR 0129，限定 ADR 0046 D3 的适用面为便携 workspace；不把原生工具链加入 full。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `desktop-shell`: 增加独立原生壳构建、前端内嵌与未接线状态的行为要求。

## Impact

A12/O01（不新增 D 编号）；V3.11 §10.1、§29、§1259。影响 src-tauri、桌面入口、构建说明。
新增 Tauri 2 原生依赖及其独立锁文件，成本为 GTK/WebKit 或 Windows WebView2 构建链、平台维护与额外编译缓存；无常驻新服务，内核和根 workspace 不依赖 Tauri。
