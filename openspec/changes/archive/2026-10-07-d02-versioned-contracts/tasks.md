# Tasks

## 1. 契约规范与源

- [x] 1.1 完成 D02 proposal/specs/design/tasks 并通过 OpenSpec strict 校验。
- [x] 1.2 建立核心/领域 Protobuf 与能力目录，验证 protoc 编译及所有目录类型引用。
- [x] 1.3 建立配置/插件/模型/目录 JSON Schema 及控制 OpenAPI，以有效/非法样本验证。

## 2. 生成及结构校验

- [x] 2.1 锁定生成器并实现 Rust/TS/C++ 与 HTTP TS 生成，通过三语言编译/类型检查。
- [x] 2.2 实现非覆盖的 schema --check 与陈旧/缺失/修改产物测试，通过漂移负向测试。
- [x] 2.3 实现信封/协议结构检查及测试，验证大整数、必需字段、大小/期限/版本与未知值。

## 3. 互通及工程集成

- [x] 3.1 实现 contracts suite，实际执行 Rust→TS→C++→Rust 大整数/未知字段/枚举互通。
- [x] 3.2 更新 CI、ADR、契约指南与命令失败测试；检查原有工程和前端仍通过。

## 4. 验证与交付

- [x] 4.1 执行完整契约/portable/前端/OpenSpec 检查并记录真实结果与未测环境。
- [x] 4.2 同步开发计划、README/AGENTS 和 D02 报告；下一任务保持 D03。

## Workflow follow-up

- 完成后以标准 OpenSpec archive 合并主规格并复验归档状态。
