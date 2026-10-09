# ADR 0002：OpenSpec SDD 与工程构建边界

- 日期：2026-10-07
- 状态：Accepted
- 关联：D01、V3.5 第 19、31–33 章

## 背景

用户要求按任务顺序推进并采用 OpenSpec 实现 SDD。本地已有 OpenSpec config
及 OpenCode 指令，远端 main 初始仅 LICENSE；应用、媒体和原生 SDK 尚未实现。

## 决策

1. 使用项目本地 OpenSpec **1.14.1**、标准 `spec-driven` schema。
   流程为 proposal → specs/design → tasks → apply → 验证 → archive。
   保留现有 OpenCode 集成；其生成版本 1.3.1 不作为本轮 CLI 版本依据。
2. V3.5 仍是唯一完整产品/技术基线；OpenSpec 记录可验证的增量需求。
   主规格不能暗中改写基线，必要偏离或新增架构选择记录 ADR。
3. 固定 Rust **1.96.0**、Node **22.23.1**、pnpm **8.15.0**。
   Cargo.lock/pnpm-lock.yaml 为真实工具生成文件，不手写锁文件。
   契约生成器/protoc 组合由 D02 选择、锁定并认证，不提前声明已生成。
4. D01 仅建立 contracts/kernel/plugin-sdk 职责入口和实际工程工具，
   领域消息先在 D02 生成。前端仅验证 React/TS/Vite 工程，Tauri 原生边界在 D14 建立。
5. portable 默认构建不含 Windows SDK/GPU/设备代码。
   windows-local 目前是限 Windows 运行的同一基础检查，不是 Windows 产品认证。
6. 依赖边界依据 Cargo 实际声明和 workspace 可达关系检查。
   所有包声明 layer 并与目录匹配，未知目录须显式增加规则；本地路径依赖必须登记为成员。
   normal/build/dev/target 和别名均不能绕过边界。contracts 不依赖业务，kernel/sdk
   只允许 contracts 作为 workspace 依赖，runtime 负责后续装配具体插件。
   若后续确需改变允许关系，先评估职责并记录 ADR，再改规则和回归测试。
7. 未实现的 schema、suite、certify 和 package 任务非零退出。
   Windows/设备/SDK/GPU/API 认证与基础 CI 单独记录。

## 取舍

固定当前可测工具组合而非浮动 latest，便于复现；升级通过独立变更验证。
使用 cargo_metadata（锁定 0.23.1）而非自写 TOML 解析，获得真实别名和条件依赖声明。
边界工具检查 workspace 声明及可达关系，不声称审计所有第三方 crate 的内部实现或运行权限。
现有层级规则仅覆盖已规划职责，新增包不能借未知目录绕过。

## 影响与验证

无数据库/运行数据迁移，无新增产品常驻服务、云主机或付费 API 请求。
以 locked portable、真实 metadata 违规测试、前端 typecheck/build、OpenSpec strict
validation 验证；Windows 构建与真实运行证据另行补充。
