# Proposal

## Why

R1 原生壳已能首次设置和运行预检，但 release 默认没有随包场景路径，构建只有 cargo build，尚无实际安装入口。操作者仍依赖源码目录，阻断 Windows 安装后使用。

## What Changes

- 配置当前用户 NSIS 安装，显式包含演示资源、侧车与 Windows 图标。
- 安装态默认场景从宿主 resource_dir 解析，保留配置覆盖且不复制密钥。
- 增加 Windows 构建脚本：锁定 CLI、先生成前端及同目标侧车，再构建安装器；区分工程未签名包与签名发布。
- 输出包摘要和操作文档；验证真实原生构建/安装路径与资源解析，无法执行的 Windows 真值具名记录。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `desktop-shell`: 随包资源与 Windows 安装构建。

## Impact

W09/W10 与 R1 交付顺序 3，D14/D19 仅追溯；V3.11 §15、§28.6、§29.5，ADR 0129/0130/0131。修改壳、Tauri 配置、打包脚本及文档；无新增运行依赖或常驻服务。NSIS/Tauri CLI 是构建依赖。签名、设备与 O01/A12 完整验收保持未完成。
