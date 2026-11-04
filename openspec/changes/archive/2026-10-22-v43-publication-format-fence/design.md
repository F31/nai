## Context
发布语义存在于对象快照和版本拒绝意图中，仅变更消息不能阻止旧binary用旧规则打开数据库。必须同时改变数据格式准入。
## Decisions
采用nai-foundation-v2/schema_version=2根清单、foundation-schema-2初始化标志及SQLite user_version=2三重格式栅栏。表结构复用，但数据库消费者能力版本发生变化。旧root加载器精确校验清单与user_version，因此旧引擎拒绝新库；新引擎拒绝旧根且不重写旧文件。没有自动迁移/兼容分支。
## Validation
源Schema先生成；正反例验证清单格式；真实SQLite确认v2创建/重启及v1原字节保留。运行相关storage/engine及Windows定向检查，不重跑无关模块。
