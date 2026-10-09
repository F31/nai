# spec-driven-delivery Specification

## Purpose
让 nai 的需求、实现和验收证据在仓库内可追溯，以 V3.5 为统一设计基线，通过 OpenSpec 的增量规格和任务记录驱动每轮开发，避免聊天计划与真实工程状态分离。

## Requirements

### Requirement: Specification before implementation
开发流程 SHALL 在实现每轮 Dxx 任务前创建 OpenSpec proposal、需求场景、design 和 tasks，引用 V3.5 及相关 ADR；基线变更必须有 ADR，OpenSpec 不得隐式覆盖基线。

#### Scenario: Start a development task
- **WHEN** 开始一个包含功能或工程行为变化的任务
- **THEN** 存在可校验的变更规格及未勾选的实现/验证清单，再开始编码

### Requirement: Evidence-based completion
开发流程 SHALL 仅在对应实现与验证完成后勾选任务，更新开发计划和报告，分别记录实现、实测、Blocked、NotApplicable 与未开始。

#### Scenario: External environment unavailable
- **WHEN** Linux 检查通过但尚无 Windows 设备或真实 SDK/API/GPU 测试结果
- **THEN** 报告保留环境边界，不将这些外部认证记录为 Passed

### Requirement: Validated spec lifecycle
项目 SHALL 提供锁定版本的 OpenSpec CLI；变更和主规格必须通过严格校验，已完成变更经校验后归档并合并增量规格，保留可追溯任务与报告。

#### Scenario: Complete and archive a change
- **WHEN** 变更任务全部完成且实际检查通过
- **THEN** 使用标准 OpenSpec 归档操作更新主规格，归档文件保留完整任务和证据链接
