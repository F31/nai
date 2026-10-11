## ADDED Requirements

### Requirement: The CI pipeline runs the complete engineering profile

持续集成（CI）SHALL 执行完整工程 profile（`cargo xtask check --profile full`），其步骤覆盖 `portable` 的**全部**步骤，
并在其后执行前端 workspace 的类型检查、构建与测试。
CI SHALL 在 Linux 与 Windows 两个平台上执行同一命令，MUST NOT 按平台选用不同的 profile。

#### Scenario: CI covers the frontend workspace

- **WHEN** CI 在 Linux 或 Windows 上执行工程检查
- **THEN** 前端 workspace 的类型检查、构建与测试被实际执行；任一步失败 SHALL 使流水线非零退出

#### Scenario: CI reports only what it ran

- **WHEN** CI 的全部工程步骤通过
- **THEN** 结论只声称工程基线可执行，MUST NOT 声称渲染正确、界面可用、设备/GPU/官方 SDK/安装验收达标

### Requirement: CI does not define the frontend commands beside the profile

CI MUST NOT 用「只覆盖 Rust 侧的 profile + 另行定义的前端命令」来代替完整工程 profile——那会让前端检查在流水线中出现第二套定义。
CI 执行的前端命令 SHALL 与门控同源，即取自各包 `package.json` 的 `scripts`；MUST NOT 在流水线中另写一份可能与之漂移的命令清单。

#### Scenario: No second frontend command list

- **WHEN** 检查流水线的步骤与门控所执行的前端命令
- **THEN** 两者指向同一处定义（各包 `package.json` 的 `scripts`），MUST NOT 出现只覆盖部分前端检查（例如只做类型检查与构建、
  不做包内单测）的第二套命令
