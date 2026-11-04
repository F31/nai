## MODIFIED Requirements
### Requirement: Product knowledge is editable without changing provider credentials
桌面 SHALL 提供商品事实与讲品段落工作台，允许在引擎运行时保存草稿和显式热发布；文档外发许可 SHALL 由操作者逐条选择，默认仅本地。读写 SHALL 有大小/数量界限和并发版本检查；运行中发布 SHALL 经prepare/activate受控撤权切换，禁止直接覆盖运行绑定。
#### Scenario: Save a product draft
- **WHEN** 操作者在停止或运行状态保存有效商品草稿
- **THEN** 草稿在数据库重启后可读，已发布知识与供应商密文保持不变，不发起付费请求
#### Scenario: Publish product facts
- **WHEN** 操作者显式发布有效事实
- **THEN** 可信引擎创建并发布新的知识快照；停止时切换下次会话，运行时经撤权及持久化确认后切换当前会话，不改旧事实版本
#### Scenario: Concurrent or running edits
- **WHEN** 场景已被另一写入者修改，或操作者在引擎运行中修改商品
- **THEN** 并发冲突拒绝覆盖新配置；运行中草稿可保存，发布必须经过受控两阶段切换，准备后的失败保持暂停，不自动恢复旧授权

## ADDED Requirements
### Requirement: Live knowledge publication preserves engine and revokes old output
宿主 SHALL 允许运行中保存商品草稿并显式热发布。热发布 SHALL 在引擎内执行prepare/activate两阶段：先静音并持久化撤权，再准备快照；宿主持久保存后激活。失败 SHALL 保持暂停并显示原因，不自动恢复旧授权或回滚旧事实。
#### Scenario: Publish while engine runs
- **WHEN** 操作者显式发布有效商品知识
- **THEN** 同一引擎进程加载新快照，旧候选/审批与迟到任务不再外发，后续人工请求使用新知识
#### Scenario: Persistence or activation fails
- **WHEN** prepare之后宿主保存失败或激活失败
- **THEN** 不恢复旧播报；界面提示暂停和显式恢复办法，持久化的新配置不得静默回滚
#### Scenario: Forged staging reference
- **WHEN** 请求包含数据根外路径、符号链接或非受控暂存标识
- **THEN** 拒绝读取与发布，不授予新输出权限
