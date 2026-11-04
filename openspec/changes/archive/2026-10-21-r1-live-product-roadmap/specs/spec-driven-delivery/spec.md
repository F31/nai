## ADDED Requirements

### Requirement: Product milestones separate implementation and usable operation
产品交付计划 SHALL 依据当前设计基线和验收注册表，以操作者可完成的完整路径定义里程碑；核心算法、生产装配、界面操作和真实环境证据 SHALL 分别陈述。缺仓内实现不得仅登记为外部阻塞。

#### Scenario: A core evaluator exists without an operational workflow
- **WHEN** 核心已有节目校验或知识检索，但没有引擎持续执行或界面管理流程
- **THEN** 计划明确保留可复用实现，并把缺少的生产装配和界面列为待开发，不宣称该用户能力已经交付

#### Scenario: Livestream integration is planned
- **WHEN** 计划把数字人接入直播
- **THEN** 分别定义音画进入直播软件、官方互动事件进入引擎和账号用途准入的证据；手工演练与本地录制不能替代真实自动互动验收
