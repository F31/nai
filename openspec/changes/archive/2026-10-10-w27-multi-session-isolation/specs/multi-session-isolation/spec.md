## Purpose

把 A11 从声明变成可执行判据：Pro 多会话时**每会话状态 MUST 私有**（独立 epoch 与 lane，跨会话访问 MUST 被拒）；**共享 MUST 只限**不可变模型、只读知识快照与受控连接池三类，共享 MUST 只读，连接池 MUST 有界；容量 MUST 按 GPU 吞吐、显存、音频时序、API 配额**分别准入**，容量不足 MUST 拒绝新增会话且 MUST NOT 改动已开播会话的用量。本 capability 不授予真实 GPU/显存探针、真实多路音频时序或跨节点会话路由的认证。

## ADDED Requirements

### Requirement: Each session has a private namespace and its own epoch

每个会话 MUST 拥有以 `session_id` 为键的私有命名空间，含**独立**的 epoch 与 lane 集合。推进一个会话的 epoch MUST NOT 改变另一个会话的 epoch。会话数 MUST 有界，重复打开同一会话 MUST 被拒绝。

#### Scenario: Advancing one epoch leaves the other alone
- **WHEN** 两个会话已打开，推进其中一个的 epoch
- **THEN** 只有该会话的 epoch 改变，另一个保持原值

#### Scenario: Opening a session twice is a conflict
- **WHEN** 用一个已打开的 session id 再次打开
- **THEN** 打开被拒绝，原命名空间不变

#### Scenario: The registry is bounded
- **WHEN** 打开的会话数达到上限后再打开一个
- **THEN** 打开被拒绝为资源耗尽

### Requirement: A lane is reachable only through its owning session

到达一个 lane 的入口 MUST 以**属主**为键：请求方与属主不一致时 MUST 被拒绝，并以稳定标记说明原因。未绑定或会话不存在时 MUST 报告为能力缺席。因此「无串场」是访问路径的性质，MUST NOT 依赖调用方自觉。

#### Scenario: The owner reaches its lane
- **WHEN** 会话 A 访问自己已绑定的 lane
- **THEN** 访问成功，返回该 lane 所属的会话与当前 epoch

#### Scenario: A different session is refused
- **WHEN** 会话 B 试图访问会话 A 的 lane
- **THEN** 访问被拒绝为冲突，且理由指明该 lane 属于另一个会话

#### Scenario: An unbound lane is absent
- **WHEN** 会话访问自己尚未绑定的 lane
- **THEN** 访问被拒绝为能力缺席

### Requirement: Sharing is limited to three read-only kinds

可共享的资源 MUST 限于**不可变模型、只读知识快照、受控连接池**三类；活的会话状态（预算、审批、输出 ID）MUST NOT 有可共享的表示。共享 MUST 只读，任何写入 MUST 被拒绝。受控连接池 MUST 有持有者上限，超过即拒绝，且上限为零 MUST 在构造时被拒。

#### Scenario: The shareable set is exactly three kinds
- **WHEN** 枚举可共享的资源类别
- **THEN** 恰好是三类，不含任何会话状态

#### Scenario: Shared state is read-only
- **WHEN** 尝试写入一个共享资源
- **THEN** 写入被拒绝为权限不足，理由指明共享资源只读

#### Scenario: The controlled pool is bounded
- **WHEN** 同一共享资源的持有者数达到上限后再有会话申请
- **THEN** 申请被拒绝为资源耗尽

#### Scenario: A zero holder limit is refused up front
- **WHEN** 用持有者上限 0 构造共享池
- **THEN** 构造被拒绝为参数非法

### Requirement: Capacity is admitted per family, two-phase

容量 MUST 按 GPU 吞吐、显存、音频时序、API 配额**四族分别**判定；某族的余量 MUST NOT 被用于另一族。准入 MUST 为两阶段：先汇总并检查**全部**族，全部通过才写入用量。任一族不足时 MUST 拒绝且 MUST NOT 写入任何用量，已开播会话的用量 MUST 保持不变。未声明的族 MUST 按零准入。释放 MUST 只影响目标会话。

#### Scenario: Headroom in one family does not buy headroom in another
- **WHEN** 某族尚有余量而另一族不足，且一次请求同时触及两者
- **THEN** 请求被拒绝，不因前者有余量而放行

#### Scenario: A refusal leaves the on-air session untouched
- **WHEN** 一个会话已占用了部分容量，另一个会话的请求因超限被拒
- **THEN** 被拒会话的用量为零，且已占用会话的用量与该族总量都不变

#### Scenario: A partial breach commits nothing
- **WHEN** 一次请求中某族可满足而另一族不可满足
- **THEN** 整个请求被拒绝，连同可满足的那一族也不写入

#### Scenario: An unstated family admits nothing
- **WHEN** 上限表中没有某族，且有请求触及该族
- **THEN** 请求被拒绝为资源耗尽

#### Scenario: Release frees only that session
- **WHEN** 释放一个会话的用量
- **THEN** 只有该会话的用量归零，其他会话不受影响，腾出的余量可被新会话使用
