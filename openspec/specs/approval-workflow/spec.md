# approval-workflow Specification

## Purpose
为操作员提供真实、有界、可信的审批闭环：可查看待审批候选、批准或拒绝候选，并让审批/拒绝写入决策日志；保证候选哈希、session/epoch 作用域、权限与幂等语义，明确保留真实候选生成、GUI 与手机审核缺口。

## Requirements

### Requirement: Bounded scoped approval listing
审批列表 SHALL 限制 workspace/session/epoch 作用域并按 created 顺序有界返回待审批候选；已撤销、已过期、已批准候选 MUST 被过滤，越界请求 MUST 用明确原因拒绝而不是隐式全量返回。列表项 MUST 携带候选哈希、审批引用（如有）、期限和证据计数，不返回候选载荷明文。

#### Scenario: Stale or approved candidates hidden
- **WHEN** 候选已批准、已过期或已撤销
- **THEN** 不再出现在待审批列表，且不能作为 approve/reject 的成功输入

#### Scenario: Limit exceeded
- **WHEN** 请求的 limit 超过服务上限
- **THEN** 返回可读拒绝，不得伪造空列表成功

#### Scenario: Candidate admission reaches capacity
- **WHEN** 当前 workspace/session/epoch 已有 200 个有效待审批候选而又入库新候选
- **THEN** 返回 ResourceExhausted 且不插入候选；HTTP 满页提示列表已达上限

### Requirement: Verified approve/deny with log correlation
审批命令 SHALL 在鉴权、approval.write 权限、session/epoch/候选哈希/幂等校验通过后写入或推进候选，并 MUST 通过既有决策日志机制登记；拒绝候选 SHALL 将 utterances 状态置为 Blocked 且已批准候选不可再拒绝。已过期/已撤销候选拒绝 MUST 返回明确过期错误，不标记成功。

#### Scenario: Approve after approval written
- **WHEN** 审批已写入后对该候选重试相同命令
- **THEN** 幂等返回原 ACK，不重复写日志或推进

#### Scenario: Reject already-approved candidate
- **WHEN** 对已批准候选调用拒绝
- **THEN** 拒绝返回失败，候选不进入 Blocked

#### Scenario: Critical log write fails
- **WHEN** 批准或拒绝事务的 critical 决策日志无法提交
- **THEN** 审批、候选状态与审计一起回滚且不返回成功 ACK

#### Scenario: Store recovery invalidates pending candidates
- **WHEN** 存储 owner 重新打开已有会话数据库
- **THEN** 旧会话递增 epoch、旧候选撤销；审批 backend 必须接收 root 当前 owner 的共享 Store，不绕过恢复撤权

### Requirement: Honest availability and incomplete-work boundary
无 workspace、存储不可用或候选来源缺失时 SHALL 返回明确 Unavailable/Blocked，不把空结果当作已审批；候选本身 SHALL 有作用域和证据有效性校验。未实现真实候选生成、桌面审批 GUI、手机审核或平台审批代理。

#### Scenario: Missing workspace
- **WHEN** 目标 workspace 不存在或权限被撤销
- **THEN** 返回明确 Unavailable/PermissionDenied，操作不改变任何状态

#### Scenario: No fabricated candidates
- **WHEN** 审查本轮实现
- **THEN** 不存在伪造候选载荷或伪造审批成功的路径；只有 list/approve/reject 三个操作标 implemented
