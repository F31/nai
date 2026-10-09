# Proposal

## Why

nai 已有 V3.5 设计但没有可编译工程，后续契约与内核任务需要可复现构建及依赖边界。
本变更落实 D01（第 19、31–33 章），并将后续开发纳入 OpenSpec SDD。

## What Changes

- 建立按 ADR 0001 命名的 Rust/pnpm workspace，锁定工具链及真实依赖锁文件。
- 提供实际执行的 `cargo xtask check-deps` 和 portable/Windows foundation 检查。
- 用元数据和目录职责验证 Rust 依赖边界，违规时返回非零退出码。
- 建立 React/TypeScript/Vite 桌面前端的工程页与类型/构建检查。
- 建立 Linux/Windows 基础 CI、OpenSpec 严格验证与开发操作文档。
- 持续记录实现、实测及未认证环境，后续每轮遵循规范→实现→验证→归档。

## Capabilities

### New Capabilities

- `engineering-foundation`：可复现 workspace、依赖边界及构建任务的失败语义。
- `spec-driven-delivery`：基于 V3.5 的 OpenSpec 变更、验证证据与归档流程。

### Modified Capabilities

无现有规格。

## Impact

影响根配置、`crates/`、`xtask/`、`apps/desktop/`、CI、OpenSpec 和工程文档。
只建立可移植库入口，不在 D02 前手写领域消息；不声明媒体、平台或 Tauri 宿主已实现。
新增 Rust 元数据解析和前端/OpenSpec 开发依赖，无新增产品常驻服务、云资源或付费 API 调用。
