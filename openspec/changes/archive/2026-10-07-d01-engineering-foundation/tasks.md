# Tasks

## 1. SDD 与工程配置

- [x] 1.1 关联既有远端，保留 LICENSE/用户文件；验证 origin、HEAD 及未提交状态。
- [x] 1.2 配置 OpenSpec 项目上下文并创建完整规格；通过 strict validation。
- [x] 1.3 建立锁定工具链的 Rust/pnpm workspace 与职责库入口；验证 locked 安装和编译。

## 2. 依赖边界与构建入口

- [x] 2.1 实现实际 metadata 依赖检查及有效/违规/伪装层级测试；通过 xtask 测试和 check-deps。
- [x] 2.2 实现 portable/windows-local 检查与未实现命令拒绝；验证集成检查和非零失败路径。
- [x] 2.3 记录依赖/构建决策 ADR 和工程操作说明；核对说明与实际命令一致。

## 3. 前端与 CI

- [x] 3.1 建立明确未连接运行时的 React/TS 工程页；通过 typecheck/build。
- [x] 3.2 建立 Linux/Windows 基础 CI 与 OpenSpec 校验；核对工作流命令，Windows 未实测单列。

## 4. 整体验证与状态

- [x] 4.1 运行 locked portable、前端和 OpenSpec 严格验证；记录真实命令和结果。
- [x] 4.2 更新 AGENTS/README、开发计划与 D01 报告，说明 D02 入口和未认证环境。

## Workflow follow-up

- 完成全部检查后通过 OpenSpec archive 合并主规格，重新验证主规格与归档任务。
