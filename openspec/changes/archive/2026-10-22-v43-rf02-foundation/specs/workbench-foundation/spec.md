## Purpose
为V4.3提供独立单写存储、认证主体/权限、不可变版本与有撤销能力的私有文字交付基础，并对失败/重启/未知结果提供真实、可验证的边界。

## ADDED Requirements
### Requirement: Foundation state has one authenticated durable owner
新线 SHALL 只在显式独立数据根内创建SQLite及对象仓库，由唯一writer操作；主体来自认证上下文，不信客户端actor。工作区角色、对象CAS/不可变版本、幂等结果、审计/outbox SHALL 事务一致；旧库/Key不打开或迁移。
#### Scenario: Unauthorized or conflicting write
- **WHEN** 非成员/已撤权主体跨空间写入，或使用陈旧revision/重复幂等键不同正文
- **THEN** 具名拒绝，无对象版本、审计成功条目或outbox半提交。
### Requirement: Private text dispatch uses an exact revocable gate
所有私有文字交付 SHALL 检查可信守门、角色/对象版本/hash、明确目标、session/epoch/范围、批准与期限。缺门控或未知目标拒绝；停止先收紧原子许可再写日志，DB失败不得恢复。
#### Scenario: Stale permit or scope
- **WHEN** 内容或角色变化、epoch改变、超时、取消、停止或请求公共/媒体目标
- **THEN** 私有发送器拒绝，且不加载TTS/keeper/Provider。
### Requirement: Recovery and receipts never fabricate external completion
dispatch意图 SHALL 先持久化；重启不恢复旧批准/许可，会话只读待重建。失联dispatch标OutcomeUnknown而不自动重发；本地文本投影Submitted不得当作真实界面可见、用户已读或Completed。预算未知费用保留待对账。
#### Scenario: Crash after dispatch begins
- **WHEN** 进程在记录dispatching后异常退出，或外部结果不确定
- **THEN** 恢复保留审计与未知状态，不重复执行该副作用。

### Requirement: Revocation intent survives audit failure without deleting unknown assets
撤权/删除 SHALL 先收紧内存许可并持久化独立撤销意图，随后写业务事务/审计；失败和重启不得恢复旧角色/内容权限。对象清理 SHALL 仅处理当前writer有明确所有权记录且未被引用、hash匹配的内容，不删除未知素材或覆盖不匹配的所有权文件。
#### Scenario: Critical audit fails during revocation
- **WHEN** 角色撤销或删除的审计事务失败并重启
- **THEN** 旧角色/正文仍不可访问，恢复须有明确的新授权或完成删除。
#### Scenario: Unknown asset in the object directory
- **WHEN** 对象目录存在未登记素材、被人工替换的正文或不匹配的所有权文件
- **THEN** 不删除或覆盖这些文件；不匹配的已登记内容给出完整性错误。
