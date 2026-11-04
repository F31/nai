## ADDED Requirements
### Requirement: Consumers must understand the publication data format
发布组合 SHALL 创建独立格式2数据根和数据库，根清单、初始化标志和SQLite user_version同时声明消费者格式。新旧消费者 SHALL 拒绝不能理解的数据格式，不自动迁移、覆盖或删除旧开发根。
#### Scenario: A legacy owner is pointed at a publication database
- **WHEN** 不理解版本撤销语义的旧引擎尝试打开新发布库，或新引擎尝试打开旧格式根
- **THEN** 格式准入拒绝，不建立业务writer，不将不理解的发布产物当普通文本交付，旧内容保持原字节。
