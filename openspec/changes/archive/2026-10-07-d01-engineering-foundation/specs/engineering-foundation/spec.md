# Engineering Foundation

## Purpose

为 nai 后续契约、内核和插件开发提供可复现的工程入口及可自动执行的架构边界检查，使开发者能够区分可移植逻辑检查、Windows 基础构建与产品真实运行认证。

## ADDED Requirements

### Requirement: Reproducible workspace
工程 SHALL 使用统一 nai 命名的 Rust 和前端 workspace，提交工具链版本与真实锁文件，支持锁定依赖的编译、类型检查和前端构建。

#### Scenario: Clean dependency installation
- **WHEN** 开发者使用规定工具链执行锁定安装和 portable 检查
- **THEN** workspace 编译、测试和前端检查可执行，不要求 GPU、SDK 或自建云服务

### Requirement: Enforced dependency boundaries
依赖检查 SHALL 从实际 Cargo 元数据检查所有 workspace 包、依赖类别及目标条件，拒绝契约依赖业务、kernel 依赖插件/宿主/UI/平台 SDK、插件依赖另一插件，以及媒体层依赖公网 Provider；包层级必须与目录职责一致。

#### Scenario: Valid graph
- **WHEN** workspace 符合声明的层级和依赖方向
- **THEN** `cargo xtask check-deps` 输出已检查包数并成功退出

#### Scenario: Forbidden or disguised dependency
- **WHEN** normal/build/dev 或条件依赖违反边界，或包用错误层级隐藏其目录职责
- **THEN** 检查非零退出并标明包及违规原因

### Requirement: Explicit build and certification boundaries
任务工具 SHALL 区分 portable 与 Windows 基础检查，在非 Windows 环境拒绝 windows-local；未知或尚未实现的 schema/certify/package 命令 SHALL 非零退出，不输出认证通过。

#### Scenario: Unsupported profile or task
- **WHEN** Linux 请求 windows-local，或请求尚未实现的认证/打包/生成任务
- **THEN** 工具明确说明限制并非零退出

#### Scenario: CI foundation checks
- **WHEN** Linux 或 Windows CI 执行工程检查
- **THEN** 仅报告已运行的工程结果，不将其视为设备、GPU、官方 SDK 或安装验收
