# approval-workflow Specification

## Purpose
为操作员提供真实、有界、可信的审批闭环：可查看待审批候选、批准或拒绝候选，并让审批/拒绝写入决策日志；保证候选哈希、session/epoch 作用域、权限与幂等语义，明确保留真实候选生成、GUI 与手机审核缺口。

## Requirements

### Requirement: Bounded scoped approval listing

审批列表 SHALL 限制 workspace/session/epoch 作用域并按 created 顺序有界返回待审批候选；已撤销、已过期、已批准候选 MUST 被过滤，越界请求 MUST 用明确原因拒绝而不是隐式全量返回。列表项 MUST 携带候选哈希、审批引用（如有）、期限、证据计数与候选的**展示文本**；展示文本供操作员核对将要播报的内容，MUST NOT 被当作身份（身份仍是候选哈希）。列表 MUST NOT 返回候选的其余载荷（语音文本、证据明细、性能参数等）。

#### Scenario: Stale or approved candidates hidden
- **WHEN** 候选已批准、已过期或已撤销
- **THEN** 不再出现在待审批列表，且不能作为 approve/reject 的成功输入

#### Scenario: Limit exceeded
- **WHEN** 请求的 limit 超过服务上限
- **THEN** 返回可读拒绝，不得伪造空列表成功

#### Scenario: Candidate admission reaches capacity
- **WHEN** 当前 workspace/session/epoch 已有 200 个有效待审批候选而又入库新候选
- **THEN** 返回 ResourceExhausted 且不插入候选；HTTP 满页提示列表已达上限

#### Scenario: Candidate payload beyond display text
- **WHEN** 审查列表载荷
- **THEN** 每项含候选哈希、展示文本、期限与证据计数，且不含语音文本、证据明细或性能参数

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

### Requirement: Displayed candidate text is display-only and absence is named

候选的展示文本 SHALL 随候选入库保存（来自生成侧的最终可展示文本），并随审批列表返回。展示文本
MUST 只用于**显示**：批准与拒绝的身份 MUST 仍是候选哈希，界面 MUST NOT 用展示文本构造或替换命令身份。
当载荷**未**给出展示文本时（例如旧库补列前的行），界面 MUST 具名说明该缺失，MUST NOT 把哈希当作文本展示，
也 MUST NOT 以空串冒充「内容为空」。

#### Scenario: Payload carries the display text
- **WHEN** 审批项带有非空展示文本
- **THEN** 界面显示该文本，且批准命令仍以该候选的哈希为身份提交

#### Scenario: Payload omits the display text
- **WHEN** 审批项缺少展示文本
- **THEN** 界面报出具名的缺失说明，不显示哈希为文本，也不显示空内容

#### Scenario: Hash is never substituted for text
- **WHEN** 审查呈现逻辑
- **THEN** 不存在把 `id`（候选哈希）渲染成展示文本的路径

### Requirement: The utterance status domain is declared in exactly one place

`utterances.status` 的取值域 SHALL 有且仅有一处声明（`nai-storage` 的 `utterance_status` 模块），
该列 MUST NOT 有第二处定义 —— 生产源码（承载这些 SQL 语句的模块）里 MUST NOT 出现
`status` 的裸数字字面量，SQL 里的每个数字 SHALL 由该模块的常量派生。
该列的分类 SHALL 是**闭世界**的：任意 `i64` MUST 归到一格，不存在「不知道该归到哪」这种第四种结果。

#### Scenario: A new bare literal is refused

- **WHEN** 有人在生产源码里新写一句带 status 数字的 SQL（例如 `SET status=11`）
- **THEN** 判据 MUST 失败并指出是哪个文件，逼他去声明处给那个值一个名字与归类

#### Scenario: Every value has a class

- **WHEN** 对任意取值（含 `i64` 两端）做分类
- **THEN** 每一格分类都真的被某个取值命中，且不存在无归类的取值

### Requirement: Usability is an allow list, never a block list

判定一个 utterance 是否**可用** SHALL 使用白名单（只认已声明的取值），
MUST NOT 使用「不在黑名单里」这种写法 —— 后者会把「还没人声明过的取值」默认判成可用，
而可用是危险方向（可用 ⇒ 可播出 / 可外发）。未声明过的取值 MUST 判为不可用。

#### Scenario: An undeclared status is not approvable

- **WHEN** 一条 utterance 的 status 从未被声明为可用（例如只受级联保护的 `11`、未分配的 `7..=10`、越出已声明域的值）
- **THEN** 它 MUST NOT 作为 `approve` 的成功输入

#### Scenario: Declared usable statuses keep working

- **WHEN** 一条 utterance 处于待批准带或已批准
- **THEN** 它仍 MUST 可以走完批准流程（白名单化不得把已声明的取值挡在外面）

#### Scenario: The SQL guard and the Rust predicate agree

- **WHEN** 逐值比较 SQL 的可用性守卫与 Rust 的可用性谓词
- **THEN** 两者 MUST 完全一致；任何一侧绕开派生链另写一份都 MUST 被判据挡下

### Requirement: Cascade invalidation protects exactly the band it names

级联失效 SHALL 只保护它点名的那一带（级联保护带），带内的行 MUST NOT 被改写，带外的一律改写成已失效。

#### Scenario: A row inside the protected band survives

- **WHEN** 一次全局级联失效作用到一条处于保护带内的 utterance
- **THEN** 它的 status MUST 保持不变

#### Scenario: A row outside the protected band is invalidated

- **WHEN** 一次全局级联失效作用到一条待批准或已批准的 utterance
- **THEN** 它的 status MUST 变成已失效

### Requirement: Values nobody writes are named rather than guessed

取值域里**没有写入者**的值 SHALL 被具名归类为「只受保护」或「未分配」，
MUST NOT 因「它看起来像终态」而被并进终态，也 MUST NOT 因「它看起来像活跃」而被判为可用。

#### Scenario: The undeclared value is not quietly treated as terminal

- **WHEN** 审查 `11` 的归类
- **THEN** 它 MUST 是「只受级联保护」这一格，而**不是**终态 —— 它在两处历史口径里的归属本就矛盾，
  保留这个矛盾 MUST 优先于替它选一边

### Requirement: The database refuses utterance statuses the code cannot classify

存储层 SHALL 用 schema 级约束把 `utterances.status` 限定在已声明的取值域内，
该域 MUST 是 `utterance_status::classify` 不肯返回 `Unassigned` 的那些取值。
域外整数（`0`、`7..=10`、`16..`、负数）MUST 在**写入时**被拒绝，
MUST NOT 先写进去、由读侧把它归成「未分类」。
拒绝 MUST 由该约束给出且可归因，MUST NOT 归因于别的约束（`NOT NULL`、外键、`STRICT` 类型）。

#### Scenario: An undeclared value cannot be written

- **WHEN** 任何 SQL 客户端（含绕开 `Store` 的直连写入）把域外整数写进这一列
- **THEN** 写入 MUST 失败，且错误 MUST 指明是取值域约束

#### Scenario: Every declared value still round-trips

- **WHEN** 写入已声明域内的取值
- **THEN** 写入 MUST 成功，且 schema 接受的集合 MUST 与声明的域**逐值**相同

#### Scenario: The stored constraint and the classifier agree

- **WHEN** 从存储读出 `utterances` 的建表语句，与 `classify` 派生的域比较
- **THEN** 两者 MUST 逐字一致；改了域而没有新迁移（或反之）MUST 失败

### Requirement: Rebuilding the utterance table preserves rows, evidence links and the foreign key

整表重建 MUST 逐字保留既有行，MUST NOT 丢失 `evidence_links` 的子行，
且重建后 `PRAGMA foreign_key_check` MUST 为空。该升级路径 MUST 在**有既有行且带子行**的
数据库上验证 —— 空表上的重建不构成证据。

#### Scenario: A populated database upgrades

- **WHEN** 一个已有 utterance 行与 evidence 链的旧版本数据库升级到新版本
- **THEN** 行逐字保留、子行仍在、外键完好，且新约束升级后才开始生效

#### Scenario: The hole existed before the upgrade

- **WHEN** 在升级**前**尝试写入域外整数
- **THEN** 它 MUST 成功 —— 这是「新约束真的是新增的闸门」的判别性前提

### Requirement: A recorded candidate is stored as pending and a fresh approval advances it to approved

登记候选 SHALL 把该行的 `utterances.status` 落成**待批准**的声明值；批准 SHALL 把**同一条**
候选推进成**已批准**的声明值。两个取值 MUST 是**落盘契约**：它们是既有数据库里已经存在的值，
改变必须配迁移，MUST NOT 只改一处常量了事。判定 MUST 从存储读回，MUST NOT 采信调用方给出的值。

#### Scenario: Recording a candidate stores the pending value

- **WHEN** 经生产的 `record_candidate` 登记一条候选
- **THEN** 从存储读回该行的 `status` MUST 是待批准的声明值

#### Scenario: Approving the same candidate advances it

- **WHEN** 对同一条候选调用 `approve` 并成功
- **THEN** 从存储读回它的 `status` MUST 是已批准的声明值，且 MUST NOT 仍是待批准

#### Scenario: The stored numbers are pinned literally

- **WHEN** 有人改动待批准或已批准的**数字常量**
- **THEN** 判据 MUST 变红 —— 断言钉的是**存储里的值**，不是「常量等于常量」
