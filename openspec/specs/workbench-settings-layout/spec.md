# workbench-settings-layout Specification

## Purpose
为操作员提供聚焦产品、场次与预演的工作界面，把模型预算、主持偏好、工作区管理和能力检查集中到独立设置视图，减少重复选择和向导步骤提示，同时保留未保存保护、任务取消与私有内容的使用边界。

## Requirements

### Requirement: Separate work and settings surfaces
界面 SHALL 在右上角提供设置按钮，主导航只展示高频工作入口，设置视图按类别呈现低频功能，并能返回之前工作页。
#### Scenario: Return from settings
- **WHEN** 操作员从已有产品页打开设置并返回
- **THEN** 恢复工作页和已加载的内容，不自动创建场次、调用模型或发布。

### Requirement: Simplify entry without bypassing safety
界面 SHALL 去除步骤编号和强制下一步措辞，空工作区直接提供创建入口；保存/放弃和任务取消规则保持实际执行。
#### Scenario: Unsaved edits or running test
- **WHEN** 存在未保存工作修改或在途模型测试时尝试切换
- **THEN** 未保存修改阻止误切换，在途任务走既有取消流程，不丢弃草稿或自动重发。
