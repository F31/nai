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
