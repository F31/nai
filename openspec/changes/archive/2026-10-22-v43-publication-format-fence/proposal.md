## Why
RF03发布语义引入了必须由消费者执行的版本拒绝scope。旧RF02/草稿引擎不理解该语义，若允许打开同一库会绕过新规则。补V4.3 INV-05/23/30与ACC-CONT-02的格式栅栏，保持整体ACC pending。
## What Changes
- 新发布组合采用独立foundation格式2根清单、初始化标志和SQLite user_version。
- 新旧格式互相拒绝，不自动迁移，不覆盖或删除旧开发根。
- 添加旧格式保持原字节与新格式重启用例。
## Capabilities
### New Capabilities
### Modified Capabilities
- `publication-snapshots`: 新增拒绝不理解发布语义的数据库消费者的要求。
## Impact
源Schema/生成配置类型、SQLite fresh schema与root加载器、相关测试及ADR0150。无新服务或依赖，不更新安装；旧数据导入保持另作显式映射/备份的任务。
