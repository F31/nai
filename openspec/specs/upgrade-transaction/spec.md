# upgrade-transaction Specification

## Purpose
把「升级原子、可预检、可回滚、不伪报」从声明变成可执行判据：升级包在暂存前 MUST 通过哈希、签名与撤销检查；预检在任一资源**不确定**时 MUST 拒绝而不是乐观放行；升级 MUST 是带五状态 journal 的原子事务且崩溃可幂等恢复；激活 MUST 只在无活动会话时发生；回滚 MUST 保留升级后新增的写入。本 capability 不授予真实签名链、Windows 安装或真实下载器的认证。

## Requirements

### Requirement: Package verification before any staging

升级包 MUST 在**产生暂存之前**完成：JSON Schema 校验、文件声明路径规范且唯一、内容集合与声明**精确匹配**（缺失、额外、重复一律拒绝）、逐文件 SHA256、签名验证、撤销列表检查。任一步失败 MUST NOT 产生暂存物或改动当前版本清单。

#### Scenario: Tampered or inexact content
- **WHEN** 某声明文件的 SHA256 不符，或内容集合缺少/多出/重复文件
- **THEN** 校验失败，不产生暂存，不触碰当前版本清单

#### Scenario: Signature or revocation rejection
- **WHEN** 签名验证失败，或包身份命中撤销列表
- **THEN** 校验失败，不产生暂存，且失败原因可区分（签名无效 / 已被撤销）

### Requirement: Revocation is checked at install and on every start

撤销检查 MUST 在**安装时与每次启动时**分别发生。撤销列表 MUST 是**带期限的可信缓存**：命中 MUST 拒绝；未命中且缓存**未过期** MUST 放行；缓存**已过期** MUST 拒绝（不得当作「无撤销」）。被撤销对象 MUST 立即禁用、不进入 Ready。

#### Scenario: Expired revocation cache denies
- **WHEN** 撤销缓存已超过其 `expires_at`，且查询对象不在缓存内
- **THEN** 结果不是放行，而是显式的不可用拒绝

#### Scenario: Revoked package is disabled at startup
- **WHEN** 某已安装对象在下次启动前的撤销列表中
- **THEN** 启动检查将其禁用，不进入 Ready

### Requirement: Preflight refuses on insufficiency or uncertainty

预检 SHALL 校验依赖闭包、平台/架构、运行时矩阵、凭据与**双份磁盘空间**（下载暂存与新数据目录同时存在的余量）。任一必需资源**不足**或**不可判定**时 MUST 拒绝，MUST NOT 伪报可用。预检失败 MUST NOT 触碰当前版本清单，原版本 MUST 继续运行。

#### Scenario: Insufficient disk refuses activation
- **WHEN** 空闲空间不足以同时容纳暂存与新数据目录
- **THEN** 预检拒绝并给出该原因，当前版本清单不变

#### Scenario: Unknown resource state is not optimism
- **WHEN** 某个资源探针返回「未知」而不是「足够」
- **THEN** 预检拒绝，且错误与「不足」可区分

#### Scenario: Incompatible dependency or platform refuses
- **WHEN** 包的依赖闭包与目标内核无交集，或其 `hosts`/`os`/`architectures`/运行时矩阵不匹配
- **THEN** 预检拒绝，不进入暂存

### Requirement: Upgrade is a journalled atomic transaction

升级 SHALL 按固定顺序推进：解析闭包 → 预检 → 暂存并验签验哈希查撤销 → 等待无活动会话 → 一致性备份 → 迁移到新数据目录 → 隔离自检 → 写 journal → 原子替换当前版本清单。journal MUST 记录 `prepared`、`migrated`、`checked`、`committed` 与失败终态 `rolled_back`（含备份引用与错误码）。当前版本清单的替换 MUST 是原子的（新文件落盘后改名覆盖）。

#### Scenario: Failure at any step leaves no half-state
- **WHEN** 备份之后、提交之前的任一步失败
- **THEN** 事务收敛到 `rolled_back`，当前版本清单仍指向旧版本

#### Scenario: Manifest swap is atomic
- **WHEN** 提交阶段替换当前版本清单
- **THEN** 读者要么看到旧清单要么看到新清单，不会看到中间态

### Requirement: Crash recovery is idempotent

恢复 MUST 依据 journal 的**当前状态**决定继续或回滚，重复执行 MUST NOT 叠加副作用。处于 `committed` 的事务重入 MUST 直接返回成功。

#### Scenario: Restart in the middle of an upgrade
- **WHEN** 进程在 `migrated` 或 `checked` 状态断电后重启
- **THEN** 恢复动作与当前状态一致，且再次运行得到相同终态

#### Scenario: Re-entering a committed transaction
- **WHEN** 对一个 `committed` 的事务再次执行
- **THEN** 直接返回成功，不重复备份或迁移

### Requirement: Activation requires no active session; downgrade drains first

升级 MUST 在开播中**只暂存不激活**；激活 MUST 在无活动会话时才发生。降档 MUST 先排空多余会话再停用依赖能力，且 MUST 保留数据、MUST NOT 静默删除。目标版本更旧或超出 `rollback_range` 时 MUST 阻止静默回滚并显式报错。

#### Scenario: Stage during a live session, activate after
- **WHEN** 存在活动会话时发起升级
- **THEN** 暂存成功而激活被门控拒绝

#### Scenario: Downgrade drains but keeps data
- **WHEN** 从高档位降到低档位
- **THEN** 多余会话被排空、依赖能力停用，已有数据仍在

#### Scenario: Incompatible silent rollback is blocked
- **WHEN** 请求回滚到包 `rollback_range` 之外的版本
- **THEN** 回滚被显式拒绝，不静默执行

### Requirement: Rollback preserves post-upgrade writes

回滚 MUST 恢复旧程序与备份，并 MUST 导出升级后新增的事实与审计记录；MUST NOT 静默丢弃这些写入。

#### Scenario: New facts survive a rollback
- **WHEN** 升级后写入了新事实与审计记录，随后回滚
- **THEN** 这些记录被导出并保留，而不是随回滚消失

### Requirement: Cost estimate lists components or marks them unknown

升级成本 SHALL 逐项列示云资源、下载、存储增量与预估运行增量。无法估算的分项 MUST 标注为未知并说明原因，MUST NOT 以 0 冒充。

#### Scenario: An unknown component is not zero
- **WHEN** 某一分项无法估算
- **THEN** 它被报告为未知（带原因），而不是数值 0

### Requirement: Verification and preflight are bounded and cancellable

校验与预检 SHALL 限制清单长度、文件数量与累计字节，并在入口、逐文件与出口复检 deadline 与 cancellation。取消或超期 MUST NOT 产生成功结果或部分暂存。

#### Scenario: Cancelled verification
- **WHEN** 校验过程中被取消或超过期限
- **THEN** 返回对应失败，且不留下暂存物

### Requirement: An interrupted download resumes from durable state, read from storage

下载中断后 MUST 从**已持久化到本地介质**的字节数续传，且该偏移 MUST 由**读取存储**得出，
MUST NOT 取自调用方传入的进度或编排器自己的累计。每次追加后 MUST **回读**确认持久化确实前进。
写入的窗口 MUST NOT 越过其声明终点；一个**超过声明大小**的续传点 MUST 被具名拒绝，
MUST NOT 被夹取（clamp）成「已完成」或「从头开始」。

#### Scenario: A short read is resumed, not restarted
- **WHEN** 某个工件的第一次响应只送到一部分字节，随后连接中断
- **THEN** 下一次请求的偏移等于该工件**已持久化**的字节数
- **AND** 已持久化的那部分 MUST NOT 被重新请求

#### Scenario: A ledger that does not persist is a failure, not progress
- **WHEN** 追加写入报告成功但回读的持久化字节数没有前进
- **THEN** 下载以具名内部错误失败，MUST NOT 把该窗口记作已到达

#### Scenario: A resume point past the declared end is refused
- **WHEN** 台账报告的某工件持久化字节数超过该工件的声明大小
- **THEN** 下载以 `invalid_argument` 拒绝，且不发出任何窗口请求

#### Scenario: Content already durable is verified without moving a byte
- **WHEN** 某工件的全部声明字节已持久化
- **THEN** 不请求任何窗口
- **AND** 该工件仍 MUST 通过其声明哈希（哈希的输入是持久化内容，不是本次运行的缓冲）

### Requirement: Re-asking is bounded and exhaustion is a named refusal

一次下载 MUST NOT 无限重问。**短读**（到达字节少于所请求窗口）是**可重问**的取值并被清点；
传输返回的错误是**终局**，MUST NOT 被重问。每个窗口的重问次数受一个声明上界约束，
**恰在上界内**完成 MUST 成功，**越过上界** MUST 以 `resource_exhausted` 具名拒绝并停止。

#### Scenario: The last allowed attempt may still succeed
- **WHEN** 重问上界为 2，且第二次尝试收到完整窗口
- **THEN** 下载成功，且实际请求次数恰为 2

#### Scenario: A source that never completes stops by name
- **WHEN** 每次尝试都只送到一部分字节，且重问次数已达上界
- **THEN** 下载以 `resource_exhausted` 拒绝，且请求次数恰为上界的值

#### Scenario: A transport error is not re-asked
- **WHEN** 传输返回一个错误而不是短读
- **THEN** 该错误被原样传播，MUST NOT 消耗重问预算

#### Scenario: More than the window is refused
- **WHEN** 一次响应到达的字节多于所请求的窗口
- **THEN** 下载以 `invalid_argument` 拒绝，MUST NOT 把多余字节接在工件上

### Requirement: A complete artifact that fails its declared hash is refused and discarded

一个字节数已达声明大小、但哈希与签名声明不符的工件 MUST 被拒绝，
且该工件在本地介质上的持久化字节 MUST 先被**丢弃**并回读确认已清空。
丢弃的理由是续传点本身：若坏前缀留下，下一次运行会在坏文件的末尾续传，而那个位置永远不会被重取。

#### Scenario: Corrupt bytes are refused and do not survive as a resume point
- **WHEN** 某工件的全部声明字节已到达，但其哈希与声明不符
- **THEN** 下载以 `invalid_argument` 拒绝（与校验链同一判定词）
- **AND** 该工件的持久化字节数回读为 0

#### Scenario: The next run refetches from zero instead of inside the discarded bytes
- **WHEN** 在被拒绝之后用同一份台账重跑该下载
- **THEN** 第一个窗口的偏移是 0，而不是坏前缀的长度
