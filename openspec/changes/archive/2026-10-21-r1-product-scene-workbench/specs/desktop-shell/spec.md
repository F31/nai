## ADDED Requirements
### Requirement: Product knowledge is editable without changing provider credentials
桌面 SHALL 提供商品事实与讲品段落工作台，允许保存草稿和显式发布；文档外发许可 SHALL 由操作者逐条选择，默认仅本地。读写 SHALL 有大小/数量界限和并发版本检查，运行中 SHALL 拒绝修改。
#### Scenario: Save a product draft
- **WHEN** 操作者保存有效商品草稿
- **THEN** 草稿在数据库重启后可读，已发布知识与供应商密文保持不变，不发起付费请求
#### Scenario: Publish product facts
- **WHEN** 停止引擎后显式发布有效事实
- **THEN** 可信引擎创建并发布新的知识快照，成功后切换场景绑定；失败不切换绑定，不改旧事实版本
#### Scenario: Concurrent or running edits
- **WHEN** 场景已被另一写入者修改或引擎正在运行
- **THEN** 保存和发布拒绝，不能覆盖新配置或改变运行知识授权
