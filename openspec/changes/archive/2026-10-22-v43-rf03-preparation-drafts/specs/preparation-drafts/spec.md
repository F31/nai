## Purpose
为V43-RF03提供可持久化的类型化备播草稿基础，明确草稿保存与正式发布、事实验证和交付许可的边界。
## ADDED Requirements
### Requirement: Preparation drafts are strict typed documents
系统 SHALL 按单一源JSON Schema和有界语义规则验证产品、主持、系列与场次草稿。事实记录类型、来源、权威声明、条件和有效期；FAQ/话术引用本地已存在事实ID。主持偏好不得授予事实权威，场次引用固定UUID/revision/hash。
#### Scenario: Invalid or unknown draft content
- **WHEN** 文档有未知字段、重复ID、非法类型值、反转有效期或悬空本地事实引用
- **THEN** 保存拒绝，不提交新对象版本或成功审计。
### Requirement: Draft writes retain durable identity without publication authority
草稿保存 SHALL 使用认证工作区ACL、CAS、不可变版本、审计和幂等，并由唯一writer执行。创建后的草稿类型不得被generic text入口擦除或改成另一类。保存 SHALL NOT 发布或授予交付许可。
#### Scenario: Restart and stale editor
- **WHEN** 重启读取已保存草稿，或陈旧编辑器按旧revision提交
- **THEN** 原始已保存正文仍可读，陈旧写入被拒绝。
#### Scenario: Unpublished draft delivery
- **WHEN** 客户端尝试将备播草稿进行私有交付
- **THEN** 守门拒绝，不创建delivery intent，不调用Provider或媒体路径。
