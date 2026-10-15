# local-control-surface Specification

## Purpose
为本机操作员提供显式启动且可真实连接的受控状态读取与撤权入口，使用当前 Session 状态和独立媒体撤销信号形成桌面控制闭环；保持身份、设备、会话作用域与重试语义，明确区分控制状态执行和真实设备播放/静音确认。

## Requirements

### Requirement: Protected bounded local access
本轮服务 SHALL 仅监听 loopback，身份令牌须有作用域、权限、期限和撤销检查；token MUST 不出现在 URL/日志/持久化。输入、并发和连接期限 SHALL 有界，未知路由不得伪造成功。

#### Scenario: Invalid identity or scope
- **WHEN** token 缺失/错误/过期/撤销，权限不足或目标会话不匹配
- **THEN** 读取/命令拒绝，不泄露会话状态或变更输出

### Requirement: Honest live status and command result
读取 SHALL 返回当前 Session 的精确 uint64 字符串及真实生命周期/模式/媒体状态；没有插件/设备时不报告 Ready。命令 ACK 的 executed SHALL 仅在控制状态已执行时为 true，不当作硬件静音/播放 ACK。

#### Scenario: Empty runtime
- **WHEN** 本机控制进程只有新建 Session、没有实际媒体设备或插件
- **THEN** 状态保持 created/standby，并明确真实媒体未连接

### Requirement: Safe idempotent emergency command
mute/takeover SHALL 在鉴权、权限、作用域及结构检查后先撤销独立媒体信号，再更新 Session。新命令须验证 epoch/revision/TTL 和绑定设备的单调 seq；同 ID 同内容重试返回原 ACK，改内容或过量记录拒绝。状态锁忙/CAS 失败 MUST 不恢复媒体输出。

#### Scenario: Busy or stale command
- **WHEN** 合法撤权命令到达而状态锁忙或 revision 已变
- **THEN** 媒体信号仍先撤销，状态更新返回明确失败，不返回已执行 ACK

#### Scenario: Replay
- **WHEN** 重试已执行命令或使用旧 seq 的新 ID
- **THEN** 前者返回原 ACK，后者拒绝，不重复推进 epoch

### Requirement: Explicit desktop connection
桌面 SHALL 明确区分未连接、连接中、已鉴权状态与错误，token 仅内存保存。命令失败/断连后不得按成功更新界面或自动重发；关闭 UI 不隐式 Stop 会话。

#### Scenario: Response missing
- **WHEN** 读取或命令返回网络/鉴权/结构错误
- **THEN** 显示错误并关闭依赖旧状态的操作，需显式刷新/连接获得真实状态

### Requirement: Engine-hosted control service

loopback 控制服务 SHALL 可由 `apps/engine` 以真实装配（存储 owner、共享会话、审批后端、决策日志）
启动，替代独立 example 的临时会话形态。引擎形态 MUST 保持既有全部不变量：显式 loopback、Bearer
临时凭据、权限粒度、有界连接与载荷、未知操作 404。

#### Scenario: Desktop client against real engine
- **WHEN** 桌面 `ControlClient` 以有效 token 连接引擎控制地址
- **THEN** status/mute/takeover/approvals 行为与既有契约一致，且 mute 后共享会话 epoch 递增、
  决策日志写入 critical 记录

#### Scenario: Revoked token after shutdown
- **WHEN** 引擎进入停机流程
- **THEN** 控制 token 被撤销（进程内验证），服务随后停止监听并以 0 退出

#### Scenario: Control revoke is visible to the shared session
- **WHEN** 桌面客户端通过引擎执行 mute/takeover
- **THEN** 同一会话实例的 epoch 递增对装配侧立即可见：引擎随后签发的租约与记录使用新 epoch，
  且旧租约不再被接受

### Requirement: Approval actions act only on candidates that are still valid

桌面审批动作 MUST 在发出请求前复核候选仍然有效：候选哈希仍在**最新列出**的队列中、其 `expires_at_utc_ms`
未过期、且当前会话 `generation_epoch` 与列出该候选时一致。任一条件不满足时 MUST NOT 发出请求，
MUST 报出具名理由，并 MUST 要求重新列出而非沿用旧快照。界面 MUST NOT 依据列出时的快照直接批准。

#### Scenario: Expired candidate

- **WHEN** 候选的 `expires_at_utc_ms` 已过（或载荷未给出有效期）
- **THEN** 不发出请求，报出「已过期／有效期未知」，该候选的批准动作关闭

#### Scenario: Session generation changed since listing

- **WHEN** 动作前重新读取的会话 `generation_epoch` 与列出候选时不同
- **THEN** 不发出请求，报出「会话已换代」，并需重新列出

#### Scenario: Candidate no longer listed

- **WHEN** 候选哈希已不在最新列出的队列中（已被处理或不再有效）
- **THEN** 不发出请求，报出「候选已不在队列」

### Requirement: High-risk approval needs a second confirmation and declares what it cannot show

批准（高危方向）MUST 需要第二次显式确认：单次点击 MUST NOT 发出请求。确认面 MUST 声明本载荷
**不提供候选的最终文本**、身份为候选哈希，并 MUST 指向该缺口对应的验收项；界面 MUST NOT 使操作员
误以为已经看到将要播报的文本。拒绝（安全方向）不要求二次确认。

#### Scenario: A single click does not approve

- **WHEN** 操作员点击一次批准
- **THEN** 不发出请求，界面进入待确认态；再次点击同一候选才提交

#### Scenario: The unavailable final text is declared

- **WHEN** 确认面呈现一个候选
- **THEN** 明确声明其最终文本不在本载荷内、身份是候选哈希，并给出该缺口对应的验收项

### Requirement: The desktop UI layer declares each assertion's implementation status

§30.1 指定的 UI 层 SHALL 位于 `tests/ui`，且 MUST 逐条声明其四条断言
（审批失效、模式切换条件、静音 ACK、升级与回滚提示）的实现状态。对界面路径尚未实现的断言，
MUST 以**未实现**记录并点名缺失前置，MUST NOT 断言其通过；声明的断言集合 MUST 恰为这四条，
既不得漏登记，也不得臆造未登记项。

#### Scenario: An unimplemented assertion is declared, not faked

- **WHEN** 某条断言所需的界面路径不存在（模式切换需 `POST /sessions/{id}/mode`，升级提示需升级状态来源）
- **THEN** 该断言以未实现记录并点名缺失前置，测试对其断言「未实现」而非通过

#### Scenario: The declared set is exactly the four

- **WHEN** 枚举 UI 层声明的断言
- **THEN** 恰为 §30.1 列出的四条，无遗漏、无未登记项

### Requirement: Upgrade cost is listed per component, and an unknown component is not rendered as zero

桌面 SHALL 把升级成本**逐项列示**（云资源、下载、存储、预估运行增量）。无法估算的分项 MUST 呈现为
**未知及其原因**，MUST NOT 呈现为 0；总额若含未知分项，MUST 呈现为**下界加未知计数**，MUST NOT 呈现为总额。
载荷 MUST 被严格解析：缺字段、未知字段、非法金额或空原因 MUST 导致**具名拒绝**，不得静默猜补。

本需求只保证**呈现口径**；真实升级界面的渲染、控制面暴露 plan/journal 与生产接线
**不在**其内，故它不构成 O05 的完整 UI 验收。

#### Scenario: An unknown component

- **WHEN** 某分项被声明为未知并给出原因
- **THEN** 该行呈现为「未知（原因）」，且不出现数字 0

#### Scenario: A partially known total

- **WHEN** 部分分项已知、部分未知
- **THEN** 总额呈现为下界加未知计数，且明确**不是**总额

#### Scenario: A malformed payload

- **WHEN** 载荷缺少必需字段、含未知字段、金额不是非负安全整数或原因为空
- **THEN** 解析具名失败，呈现层不产出任何行

### Requirement: Memory endpoints forward to the runtime through a seam

控制面 SHALL 提供 `POST /api/v1/memory/read` 与 `POST /api/v1/memory/forget`，经接缝 `MemorySurface`
调到装配根的读取与遗忘落点。接缝 SHALL 由 `nai-control-api` 定义、由装配根实现、由引擎注入。
控制面 MUST NOT 自己重跑 §42.3／§42.4 的任何一条判定。
后端未注入 SHALL 报 `CAPABILITY_NOT_READY`，MUST NOT 假成功。

#### Scenario: A wiring that exists is a wiring that runs

- **WHEN** 以持 `memory.read` 的 token 请求 `POST /api/v1/memory/read`
- **THEN** 接缝**真的被调用**（调用可被观测），MUST NOT 是一个恒真桩

#### Scenario: No backend is not an empty backend

- **WHEN** 控制面未注入记忆后端而收到记忆请求
- **THEN** 报 `CAPABILITY_NOT_READY`，MUST NOT 以空结果报成功

### Requirement: A refused read is reported by name, not as an empty read

请求级拒绝 SHALL 以具名 HTTP 错误返回，MUST NOT 折成 `200` 加空 `records`——
「本次读取不成立」与「读出来零条」不得不可区分。路径自身的故障 SHALL 与判定拒绝分开，
MUST NOT 把存储故障或锁竞争记在判定名下。

#### Scenario: A read the judgement refuses is not an empty read

- **WHEN** 查询本身被 §42.3 拒绝（例如请求了近似顶替，或观众记忆未启用）
- **THEN** 响应是具名的请求级错误，而不是 `200` 加空 `records`

#### Scenario: Storage faults are not reported as refusals

- **WHEN** 记忆后端返回故障（例如装配根正忙）
- **THEN** 响应把它报成冲突，MUST NOT 报成权限不足或判定拒绝

### Requirement: Memory reads are scoped by the control surface and out-of-scope stays decidable

记忆读取的作用域维度（workspace 与 session）SHALL 由控制面自身的绑定给出，
MUST NOT 由请求指定。越作用域这一条判定 MUST 仍然可达：属于同一工作区但**其他会话**的记录
SHALL 逐条进 `refused` 并具名 `out_of_scope`，MUST NOT 被取数层预先筛掉。

#### Scenario: An out-of-scope record is named, not dropped

- **WHEN** 同一工作区里存在属于其他会话的记忆记录，且经控制面发起一次读取
- **THEN** 该记录逐条出现在 `refused` 中且拒因为 `out_of_scope`

#### Scenario: A request cannot choose its own scope

- **WHEN** 请求里带上 `workspace_id` 或 `session_id`
- **THEN** 这两个字段被忽略，取数仍按控制面自身的绑定

### Requirement: Forget blocks the read side first and fails by name

遗忘端点 SHALL **先**阻断读取、再清除载荷，并返回**生效中的**撤销时刻（已撤销则返回原有时刻，
MUST NOT 覆盖）。清除结论（副本状态、载荷清除时刻）SHALL 由**系统**给出，MUST NOT 由请求体提供；
请求体 MUST NOT 承载结论字段。记录不存在 SHALL 具名失败，MUST NOT 静默成功。

#### Scenario: Forget blocks the read side first

- **WHEN** 以持 `memory.forget` 的 token 对一条存在的记录请求遗忘
- **THEN** 该行被标为已撤销，随后读取该记录时它进 `refused(revoked)`；
  重复遗忘返回**同一个**撤销时刻

#### Scenario: A missing record is a named failure

- **WHEN** 遗忘请求给的 `record_id` 在本工作区不存在
- **THEN** 响应是具名的失败，MUST NOT 报成功

#### Scenario: The caller cannot declare the purge finished

- **WHEN** 请求体自带 `purge_state` 或 `claims_fully_erased`
- **THEN** 该请求按**未知字段**被拒（`400`）；MUST NOT 静默忽略一个自称已擦除的字段，
  MUST NOT 照抄成响应里的结论

### Requirement: The copy-site ledger is a closed world derived from the code

控制面 SHALL 持有一份**记忆载荷副本清单**，每个在册点 MUST 有逐字存在的清除体。
清单 MUST 与**代码派生**的落点集合一致：新增一个承载记忆载荷的落点而未登记，判据 MUST 失败。
清单为空 MUST 被**清点**并报出 `0`，MUST NOT 当成「无副本可清」而跳过。

#### Scenario: An empty ledger is counted, not skipped

- **WHEN** 清单当前没有任何在册副本点
- **THEN** 清除仍执行清点并报 `copy_sites_cleared = 0`，副本状态得出「已清」；
  把空清单短路成「跳过」的实现不满足本需求

#### Scenario: A new payload landing that is not registered fails the guard

- **WHEN** 存储层新增一处写入记忆载荷的落点而清单未登记
- **THEN** 闭世界判据失败并点名该落点；MUST NOT 因为清单是手写而放行

### Requirement: Erasure is claimed at the scope that was actually achieved

清除响应 SHALL 声明擦除**口径**：`scope` 为 `logical`、`physical_bytes_erased` 为 `false`。
本轮 MUST NOT 宣称介质级完全擦除——置空一列不是介质级擦除（页与 WAL 可能仍留有字节）。

#### Scenario: The response never claims media-level erasure

- **WHEN** 一次清除成功完成
- **THEN** 响应里的 `erased.physical_bytes_erased` 为 `false` 且 `scope` 为 `logical`；
  把它写成 `true` 的实现不满足本需求

### Requirement: Memory endpoints reject cross-session credentials

控制面 SHALL 保证 memory 读取 / 遗忘端点绑定当前场：鉴权走 `self.access`（绑定 session_id + device）。用别的 session 或别的 device 的 token 访问本场控制面 SHALL 以 `401` / `403` 拒绝，MUST NOT 被接受。

#### Scenario: A token from another session is rejected

- **WHEN** 用 session B 颁发的 token 访问 session A 控制面的 `/api/v1/memory/read`
- **THEN** 返回 `401` / `403`，MUST NOT 读到任何记录

#### Scenario: A token from another device is rejected

- **WHEN** 用别的 device 颁发的 token 访问本场控制面
- **THEN** 返回 `401` / `403`

### Requirement: Memory scope is a fact of the control surface, not a request parameter

控制面 SHALL 从自身绑定取 `scope.session_id` / `workspace`，请求体中出现同名字段 MUST 被忽略（沿用 W109/W111 纪律）。

#### Scenario: A forged session_id in the body is ignored

- **WHEN** 请求体携带与当前场不符的 `session_id`
- **THEN** 控制面仍用自身绑定的 session_id 构造查询，伪造字段不影响作用域
