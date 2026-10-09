## Context
V3.6 的 O02/S02 需要真实的审批闭环。既有 storage `approve`/`approval_valid` 只支持写入与在校验，无法列出待审批或拒绝推进；control-api 的审批路由是占位。

## Decisions
- 复用现有 approvals/utterances/evidence_links 表与权限模型，不新增迁移或 schema 版本。OpenAPI 把 GET /approvals、POST approve、POST reject 三个操作改标 implemented-loopback，并为 ResourceItem 增加 U64 字符串的期限与证据计数；先生成再修改消费者。
- nai-storage 新增：
  - `list_pending_approvals(op, workspace, actor, session, epoch, limit)`：需要 approval.read；过滤失效事实/文档、旧 epoch、已撤销/过期和已批准候选，按 rowid 创建顺序有界返回；返回候选哈希、approval_id（如有）、expiry、总证据计数，不返回 payload 明文。limit 范围 1–200；HTTP 固定 200，满页标 approvals_limit_reached，不声称完整队列，不提供分页接口。
  - `read_approval(op, workspace, session, epoch, candidate_hash)`：精确读取一条审批的元信息。
  - `reject_candidate(op, workspace, actor, session, epoch, candidate_hash)`：需要 approval.write 权限；同事务将候选置为 Blocked、写 approval.rejected 审计、撤销旧审批并登记 critical cancellation 日志。approve 同事务置为 Approved、写审批/审计/critical approval 日志；日志失败全部回滚。
- control-api 复用有界 CommandLedger：approve/reject 使用生成的 ApprovalCommand 校验器（command_id/epoch/revision/device/seq/ttl/candidate_hash/hash_algorithm），先鉴权+权限+作用域校验；相同 ID 与完整内容返回原 ACK，改变 action/内容拒绝。command_id 或 candidate_hash 可关联日志；ACK 仅表示本机数据提交，不代表真实播放。
- 审批上限：record_candidate 按 workspace/session/epoch 限制有效待审批候选最多 200 个，满额拒绝入库（ResourceExhausted）；list 每次最多 200 条，control 命令账本共享 256 条、重试保留 300 秒。不新增可调配置；生产配置映射待 root 接线。
- backend 接收 root 已打开的 Arc<Mutex<Store>>，保持唯一存储 owner；不得为审批重开 Store，因为 open 必须执行恢复/撤权。每个请求新建 Operation，最长 5 秒且写入不超过剩余 TTL；try_lock 失败立即返回 Conflict，不阻塞 P0。mount 校验 workspace 一致。
- 候选拒绝：已批准（Approved 状态）的候选不可再 reject；已撤销/过期候选 reject 返回明确已过期，不伪造成功。
- storage 为空/无 workspace 时返回明确 Unavailable，避免把空结果当作已审批。

## Risks / Trade-offs
- 审批决策日志依赖既有决策录入机制；候选/审批没有真实生成的音视频，仅做数据结构闭环，不作为真实平台审核。
- 拒绝推进不写 SQL 触发器之类持久化触发；候选状态由 storage 单写 owner 更新。
- OpenAPI 只标记可实测子集，避免把未来审批 UI/手机路径当作已实现。

## Verification
storage 测试覆盖有界列表、过期/撤销过滤、批准后不可拒绝、拒绝推进状态、权限缺失；control-api 测试覆盖鉴权/权限/epoch/seq/决策日志关联、上限拒绝、幂等重试、拒绝已批准
