# runtime-composition Specification

## Purpose
建立 composition root（`crates/nai-runtime`）：把存储在 owner、配置、会话与值守租约、审批后端、决策日志、媒体门控与媒体子进程监督装配成一个受监督整体，并在子进程故障时执行「先静音、再撤权、再记录、转待机」。kernel MUST NOT 依赖本装配根。

## Requirements

### Requirement: Composition root assembly and dependency direction

装配根 SHALL 在 `runtime` 层注册并只依赖既有实现；kernel MUST NOT 依赖 runtime 或具体插件（由 `cargo xtask check-deps` 强制）。装配 SHALL 保持存储在单 owner 与短 `Operation` 约定下访问，不得重开 Store 绕过恢复撤权。

#### Scenario: Dependency violation
- **WHEN** kernel 或 sdk 依赖 runtime、或引入 plugin→plugin 直接依赖
- **THEN** 依赖检查失败，装配不得合入

#### Scenario: Second storage owner
- **WHEN** 已在运行的装配再次打开同一数据根
- **THEN** 拒绝（Conflict），不产生第二个写 owner

### Requirement: Composition lifecycle logging and control ordering

装配 SHALL 把会话控制按「先媒体侧静音/撤权，再持久化与记录」的顺序执行，并把关键事件写决策日志：critical 类别写入失败 MUST 上报错误，telemetry 类别 MAY 进入有界缓冲。值守租约的授予、到期与撤权 MUST 记为 critical。

#### Scenario: Mute or takeover
- **WHEN** 控制面请求静音或接管
- **THEN** 媒体侧先失权、会话 epoch 递增，随后写入 critical 决策日志；控制 ACK 不代表设备已停止

#### Scenario: Critical logging failure
- **WHEN** 关键事件写入决策日志失败
- **THEN** 调用方收到错误，不静默丢弃关键记录

### Requirement: Supervised child fault handling

装配 SHALL 监督媒体子进程（keeper、render-worker）的存活与状态；检测到退出、被强杀或状态读取失败 MUST 依次执行：媒体侧立即静音 → 撤销输出授权（epoch 递增）→ 写 critical 决策日志 → 置待机且拒绝继续投递。MUST NOT 因日志或数据库故障而回退静音。

#### Scenario: Keeper killed
- **WHEN** 被监督的 keeper 子进程被强杀
- **THEN** 装配撤销输出授权、记录 critical 日志并转待机；决策日志链校验通过

#### Scenario: Publish failure while halting
- **WHEN** 故障处置期间决策日志写入失败
- **THEN** 静音与撤权仍然生效，错误如实上报，不假装已记录

### Requirement: Honest supervision boundary
Composition库不自行监听HTTP；apps/engine显式装配loopback工程服务。文档 MUST声明无设备/窗口/音频输出/PlaybackAck/生产IPC认证，演练不得表示生产Ready。
#### Scenario: Status reporting
- **WHEN** 装配报告状态
- **THEN** output_available保持false并给出CapabilityNotReady或待机原因，帧消费不提升设备状态

### Requirement: Shared session between control surface and composition

控制面（HTTP 命令处理）与装配根（租约签发、值守、决策日志、故障处置）SHALL 共享同一会话实例
（`Arc<Mutex<Session>>`），使撤权产生的 epoch 递增对两侧立即可见。查询与命令路径的锁竞争 MUST 以显式
冲突错误处理，且 MUST NOT 阻塞或回退已执行的媒体静音。**撤权与故障处置**路径上的瞬态竞争 SHALL 在有界
预算内重试，耗尽 MUST 显式上报，MUST NOT 静默跳过撤权。**轮询读取**（epoch 活性检查）MUST NOT 把竞争
当故障：SHALL 在有界预算内重试，耗尽报「本 tick 未观测到」，MUST NOT 报错或退出。所有预算 MUST ≤ 30s。

#### Scenario: HTTP revoke visible to supervisor

- **WHEN** 控制面执行静音/接管使会话撤权
- **THEN** 装配根随后签发的租约与记录的决策日志使用递增后的 epoch；keeper 持有的旧 epoch 租约失效

#### Scenario: Session lock busy

- **WHEN** 会话锁被占用时到达控制或故障处置
- **THEN** 返回显式冲突错误；已执行的 P0 静音不回退

#### Scenario: Transient contention on the revoke path

- **WHEN** 会话锁被一次微秒级、自我消除的竞争短暂占用，而撤权或故障处置正在执行
- **THEN** 该动作在有界预算内重试并最终落地；MUST NOT 因这次竞争被跳过，也 MUST NOT 被报告为终局失败

#### Scenario: Contention during the epoch poll

- **WHEN** 活性检查读取会话锁时遇到同样的瞬态竞争，且竞争持续超过其预算
- **THEN** 该 tick 报告「未观测到」并继续；MUST NOT 被当作故障，MUST NOT 终止监督循环或触发撤权

### Requirement: Executable engine assembly

`apps/engine` SHALL 以显式参数装配并启动：数据根与工作区/actor/profile、token 文件、loopback 控制
端口（可为 0）、可选 halt 路径、成对给出的 keeper/render-worker 与媒体演练参数。无隐式服务启动；
缺失必需参数或 token 文件不合规 MUST 显式拒绝并退出非零。启动成功 SHALL 在 stdout 打印一行机器可
解析的控制地址。可信 owner 配置 SHALL 由 `--provision <profile.json>` 一次性完成（建工作区、授予
权限、写入并校验运行时 profile，随后退出 0 且不启动服务）。

#### Scenario: Missing or invalid arguments
- **WHEN** 缺少 data-root/workspace/actor/profile、缺少 token-file（且非 provision 模式），
  或 token 文件权限/格式不合规，或只给出 keeper 与 render-worker 之一
- **THEN** 引擎拒绝启动并退出非零，不创建任何服务监听

#### Scenario: Provisioning is a separate trusted-owner step
- **WHEN** 以 `--provision` 给出合法运行时 profile
- **THEN** 工作区、权限与 profile 被写入并校验，进程退出 0 且不监听任何端口；随后的正常启动无需
  bootstrap 分支

#### Scenario: Engine serves the real control surface
- **WHEN** 引擎以有效参数启动
- **THEN** loopback 控制服务由同一装配驱动：status 反映共享会话真实状态，mute/takeover 经
  「先静音→撤权→critical 日志」执行，审批列表来自同一审批后端

### Requirement: Engine supervision loop

引擎在配置 keeper 时 SHALL 运行监督循环：spawn keeper、以共享会话 epoch 签发 Rehearsal 租约并 arm、
按配置间隔续租、驱动值守 tick 与故障 observe。控制面撤权后引擎 MAY 显式重新签发租约（记决策日志）；
子进程故障后 MUST NOT 自动重启输出。监督循环 MUST NOT 因瞬态锁竞争而终止：活性检查失败 SHALL 视为
「本 tick 未观测到变化」并继续执行故障 observe；循环的终止只允许来自 HALT、REVOKED 或真实故障。

#### Scenario: Keeper killed under engine supervision

- **WHEN** 被引擎监督的 keeper 被强杀
- **THEN** 引擎静音、撤权、写 critical 决策日志并转待机；控制服务保持可达；输出不自动恢复

#### Scenario: Renewal stops after a control revoke

- **WHEN** 控制面撤权使 epoch 递增且 keeper 仍被监督
- **THEN** 引擎停止续租（记录 `renewal_halted_after_revoke`）并让既有租约到期失效；
  重新授予输出权限 MUST 由显式操作者决定，引擎 MUST NOT 自动重新签发

#### Scenario: Renewal failure does not silently exit

- **WHEN** 续租失败（例如子进程已消失）
- **THEN** 引擎记录失败并继续执行故障处置（静音→撤权→critical 日志→待机），
  MUST NOT 在未撤权的情况下退出监督循环

#### Scenario: Transient lock on the liveness read

- **WHEN** 活性检查因瞬态锁竞争读不到 epoch
- **THEN** 引擎跳过本 tick 的撤权判定，继续执行续租与故障 observe；监督循环 MUST NOT 结束，
  且后续 tick SHALL 恢复该判定

### Requirement: Graceful engine shutdown

引擎收到显式 halt 请求（`--halt-file` 出现）SHALL 依次：撤销控制 token、P0 静音与撤权、写 critical
`cancellation` 日志、有界回收受监督子进程、停止 HTTP 服务，然后以 0 退出。停机 MUST 有界。
被外部强杀的进程 MUST NOT 被表述为优雅停机。

#### Scenario: Halt during supervised playback
- **WHEN** 引擎在监督 keeper 期间收到 halt 请求
- **THEN** token 撤销、keeper 被有界回收、HTTP 停止、决策日志含 critical `cancellation` 记录，
  进程以 0 退出

### Requirement: Explicit media drill boundary
`--media-drill` SHALL仅使用真实资产、tmpfs与子进程，经broker root声明/proc核验、reserve/commit及keeper读取owned像素；记录telemetry，故障明确记录并撤权。UDS对端仍未认证、句柄仍spawn继承，MUST NOT称为正式protected IPC/设备/PlaybackAck通过。
#### Scenario: Missing tmpfs or asset closure
- **WHEN** 非Linux、资产缺失或worker未按时就绪
- **THEN** 记录失败、撤权与回收child，不成功提交；控制服务可查询失效状态
#### Scenario: Drill commit
- **WHEN** keeper诊断与已接受commit匹配
- **THEN** 记录media_drill_committed含buffer_ref/SHA256/bytes/keeper观察，不声称播放ACK

### Requirement: Broker-gated engine frame relay
media-drill SHALL由root声明真实spawn pid并按既有broker的/proc证据验证两端，再reserve/commit/pin/relay/release。consumer MUST只继承只读池FD；keeper MUST复检pool/epoch/live Rehearsal lease/允许utterance并保留owned像素，提供可关联seq/buffer_ref/SHA256的诊断，不产生PlaybackAck或Ready。
#### Scenario: Verified single frame reaches keeper
- **WHEN** 两端验证且worker在槽期限内提交
- **THEN** root接受commit后才转发；匹配keeper诊断前槽仍Reading不可覆盖
#### Scenario: Peer disappears or metadata changes
- **WHEN** peer退出/失去FD或票据与pool/live lease不符
- **THEN** 不成功转发/呈现，撤权或明确待机，不自动恢复

### Requirement: Bounded cancellable media supervision
engine SHALL在预热/授权/事件等待期间持续续租/observe并复检epoch/halt；阻塞worker管道 MUST不阻塞owner期限/停机。媒体失败 MUST先P0静音→撤权→critical记录，再有界回收child；日志失败不得回退静音。
#### Scenario: Worker stalls while preparing
- **WHEN** worker卡住或缺ready/commit，或halt/HTTP撤权到达
- **THEN** 有界退出等待与回收child；HTTP保持可达直到明确halt，不伪造成功帧

### Requirement: Manual event to decision-log pipeline
engine SHALL 接受显式标注为手工来源的事件，并按「归一化/去重 → 知识检索 → 智能体决策 → 守门四阶段 → 文本生成 → 决策日志」顺序编排。每一步 MUST 在进入下一次对外调用前复检 session、generation epoch、值守租约与审批对象哈希。守门缺失或未通过时 MUST NOT 外发；无知识命中 MUST 显式记录而非静默继续。

#### Scenario: Manual event produces a logged decision
- **WHEN** 手工事件通过守门且文本生成成功
- **THEN** 决策日志写入可校验条目，记录事件、路由、守门结果、模型标识与 usage，且哈希链校验通过

#### Scenario: Stale epoch or missing guard
- **WHEN** 事件携带陈旧 epoch、值守失效或守门不可用
- **THEN** 不发生对外模型调用，记录 critical 条目并进入撤权路径

### Requirement: Deterministic rehearsal replay for the decision log
engine SHALL 支持以确定性 rehearsal 生成器对同一手工事件重放，并与原次决策日志逐项比对。两次结果不一致 MUST 视为判据不达成并如实报告。

#### Scenario: Replay matches the original decision
- **WHEN** 同一事件集与同一确定性生成器重放
- **THEN** 路由、守门结论、生成文本与 usage 与原次一致，比对通过

### Requirement: Orchestration boundary and failure ordering
编排 SHALL 位于 app 层（`apps/engine`），MUST NOT 引入 kernel→runtime 或 plugin→plugin 依赖。链路任一阶段失败 MUST 依次执行「P0 静音 → 撤销输出授权（epoch 递增）→ critical 决策日志 → 有界退出」；决策日志写入失败 MUST NOT 回退已执行的静音。

#### Scenario: Injected stage failure
- **WHEN** 演练注入使知识、守门、生成或存储阶段失败
- **THEN** 引擎先静音再撤权并写 critical 日志，随后有界退出，不产生成功结果行

### Requirement: Rehearsal speech-to-playback completion
engine SHALL 在演练脚本显式给出 `render` 段时，把已审批候选对象的文本经语音接缝合成为有界 PCM，按样本窗口切成有界 `AudioBlock` 流，并由 2D 渲染核心以**同一候选对象**产出真实 RGBA 帧计入视频水位。音频水位与首批视频水位都达到后 SHALL 提交，再由演练播放槽按 `(output_id, utterance_id, 预期样本总数)` 发出 ACK。只有「committed=true、actual_local_playback_completed=true、played_sample_count=预期样本总数」的 ACK SHALL 记 `PlaybackAck` 决策条目；其余结局 MUST NOT 记该 kind，MUST NOT 声称观众已听到。

#### Scenario: Approved event reaches a verified playback completion
- **WHEN** 已审批手工事件通过守门与文本生成，且演练播放场景为 `complete`
- **THEN** 决策日志出现一条 `PlaybackAck` 条目，记录 utterance、输出身份、语音/渲染模型标识、采样数与帧数，哈希链校验通过

#### Scenario: Partial, invalid or cancelled playback never logs a PlaybackAck
- **WHEN** 演练播放场景为只部分确认、身份或样本数不符、或已取消
- **THEN** 不出现 `PlaybackAck` 条目，摘要如实标注未完成与 `playback_ack=false`，不产生重复播报

#### Scenario: Injected speech failure halts in order
- **WHEN** 注入使语音合成阶段失败
- **THEN** 引擎先 P0 静音、再撤权、再写 critical 决策日志，随后有界退出，不产生成功结果行

### Requirement: Rehearsal playback boundary is not device certification
演练播放槽是**软件替身**：链路 MUST NOT 写入帧槽位、MUST NOT 涉及 keeper 帧消费、MUST NOT 声称硬件播放或设备认证。其 `PlaybackAck` 记录 MAY 出现在决策日志，但摘要的 `devices` 与 `output_available` 恒为 false，帧只存在于内存。

#### Scenario: Summary never overclaims the output
- **WHEN** 演练播放场景为 `complete` 并因此记录 `PlaybackAck`
- **THEN** 摘要仍标注 `devices=false`、`output_available=false`、`real_tts=false`、`frame_slot=false`、`keeper_consume=false`，不得据此标设备或生产 Ready

### Requirement: Explicit render boundary in the rehearsal script
`render` 段 MUST 为可选且显式：缺失时链路 MUST 止于文本并如实标注 `delivery=none`、`playback_ack=false`；给出时 MUST 是完整闭包（manifest、config、恰三份资产、至少一项许可、非空 output_id、合法播放场景），任一缺失或非法 MUST 拒绝，MUST NOT 猜默认配置或代填身份。

#### Scenario: Absent or incomplete render closure
- **WHEN** 脚本不含 `render` 段，或含 `render` 但资产闭包/许可/输出身份/播放场景不完整或非法
- **THEN** 前者按文本链路运行并如实标注 `delivery=none`；后者在驱动任何会话状态前被拒绝

### Requirement: Bring-up operations are budgeted for the work they include

包含 schema 迁移的一次性 bring-up（`Store::open`、可信 owner `--provision`）SHALL 使用**具名的 bring-up
预算**，MUST NOT 与逐次调用的小预算共用取值；bring-up 预算 MUST 大于逐次调用预算，且 MUST ≤ ADR 0004 的
30s 上界（ADR 0038 的安全层不变量）。不含迁移的逐次调用 SHALL 保持小预算。bring-up 失败 MUST 指名
**步骤**与**已耗时**，MUST NOT 只报一个期限错误码。

#### Scenario: Fresh store with the full migration chain

- **WHEN** 在空数据根上执行 bring-up，需要跑完全部迁移脚本
- **THEN** 该操作在 bring-up 预算内完成；预算取值 MUST NOT 小于同仓同类迁移操作的既有取值

#### Scenario: Bring-up fails

- **WHEN** bring-up 的某一步失败，含预算耗尽
- **THEN** 错误指名该步骤与已耗时，使该次失败可判定，而不是只给出一个期限错误码

### Requirement: A judged-loggable harness observation reaches the hash chain

装配根 SHALL 提供一处把 harness 观测记录交给只追加哈希链的入口，且该入口 MUST 先经
`agent-harness` 的观测判定：只有判定为**可落日志**的记录才追加一条 `observation` 类条目。
追加的载荷 MUST **恰为**判定输出的已脱敏材料（层标签、脱敏字段、脱敏观众原文三项），
MUST NOT 携带记录中任何未经判定的原始字段——runtime 是这份材料的**传输者**，不是加工者。
追加 SHALL 复用装配根既有的单 owner 决策写入路径与逐次调用预算，MUST NOT 另开第二条写入路径。
追加之后链 SHALL 校验通过。

#### Scenario: A loggable record is appended and the chain verifies

- **WHEN** 五类观测之一的一条记录经判定为可落日志
- **THEN** 决策日志追加一条 `observation` 条目，其载荷的层标签等于该记录的层，且哈希链校验通过

#### Scenario: The appended payload is the judged material, not the raw record

- **WHEN** 一条可落日志记录的某字段值超过字段上限（判定将其脱敏为截断标记）
- **THEN** 追加载荷在该字段处为**截断标记**而非原值；把 `record.fields` 原样落日志的实现不满足本需求

### Requirement: A refused harness observation is named and leaves the chain untouched

判定为**不可落日志**的记录 MUST NOT 触发任何追加：入口 MUST 返回一处**具名拒绝**，逐条列出判定给出的
拒绝名与其层标签，并附上判定输出的已脱敏材料供调用方查看；该材料 MUST NOT 因随拒绝返回而落盘。
入口的返回 MUST 可区分「已落日志」与「已拒绝」。把拒绝实现为静默 `Ok`（既不追加也不具名）不满足本需求。

#### Scenario: A record carries a plaintext credential

- **WHEN** 记录的某字段值内含明文凭据字面量
- **THEN** 入口返回具名拒绝（含 `observation.plaintext_credential`），且决策日志**未新增**任何条目

#### Scenario: A required field is missing

- **WHEN** 某一层记录缺少该层规定的必需项之一
- **THEN** 入口返回具名拒绝（含 `observation.missing_required_field`），且决策日志未新增任何条目

#### Scenario: A refusal does not masquerade as success

- **WHEN** 记录被判为不可落日志
- **THEN** 入口的返回必须与「已落日志」可区分；以 `Ok` + 空载荷静默通过的实现不满足本需求

### Requirement: A rebuildable assembly reaches the hash chain as its own fragments

装配根 SHALL 提供一处把一次上下文装配交给只追加哈希链的入口，且该入口 MUST 先经 `agent-harness` 的
**两项**装配判定：只有**可调用**（§41.1–§41.3 的段序、不可裁段、裁剪位置、分段预算与片段标注全部通过）
**且可重建**（§41.5 的日志重建）的装配才追加一条 `assembly` 类条目。追加的载荷 MUST **恰为** §41.5
要求落账的**四项**——按记录**进入顺序**排列的片段（每项带其 `id`、来源标识 `source_ref` 与**字面正文**）、
各段预算占用、裁剪项与原因、模型与提示词版本；正文 MUST NOT 退化成指向当时内存的指针。
追加 SHALL 复用装配根既有的单 owner 决策写入路径与逐次调用预算，MUST NOT 另开第二条写入路径。
追加之后链 SHALL 校验通过。

#### Scenario: A rebuildable assembly is appended with its fragment bodies

- **WHEN** 一次装配经两项判定为可调用且可重建
- **THEN** 决策日志追加一条 `assembly` 条目，其载荷按进入顺序含每个被引用片段的标识、来源标识与**正文**，
  且哈希链校验通过

#### Scenario: A fragment body that only lives in memory is refused, not logged as a pointer

- **WHEN** 某被引用片段的正文不在交给判定的材料里（只存在于当时的内存）
- **THEN** 该装配被判为不可重建，入口具名拒绝且不追加；把正文落成指针（或 `null`）后追加的实现
  不满足本需求

#### Scenario: One of the two judgements refuses, and nothing is appended

- **WHEN** 一次装配可重建但不可调用（或缺段、或超预算）
- **THEN** 入口具名拒绝且不追加；只跑可重建那一项判定就放行的实现不满足本需求

### Requirement: A non-rebuildable assembly is named and leaves the chain untouched

判定为**不可重建**的装配 MUST NOT 触发任何追加：入口 MUST 返回一处**具名拒绝**，逐条列出判定给出的
拒绝名，并附上判定的三类问题标识（引用了却不在日志里的 id、正文只在当时的 id、正文含明文凭据的 id）
供调用方查看。入口的返回 MUST 可区分「已落日志」与「已拒绝」。把拒绝实现为静默 `Ok`（既不追加也不具名）
不满足本需求。

#### Scenario: The log carries a fragment the record never entered in that order

- **WHEN** 记录声明的进入顺序与日志自身携带的顺序不一致，且被引用片段齐全
- **THEN** 入口返回具名拒绝（含 `rebuild.order_differs`），且决策日志**未新增**任何条目

#### Scenario: A referenced fragment is absent from the log

- **WHEN** 记录引用的某个片段在日志里根本没有条目
- **THEN** 入口返回具名拒绝（含 `rebuild.fragment_not_logged`），且决策日志未新增任何条目

#### Scenario: A refusal does not masquerade as success

- **WHEN** 一次装配被判为不可重建
- **THEN** 入口的返回必须与「已落日志」可区分；以 `Ok` + 空载荷静默通过的实现不满足本需求

### Requirement: An assembly is rebuilt from the persisted chain alone

装配根 SHALL 提供一处**只读**入口，从只追加哈希链中读回 `assembly` 条目并重排其片段序列，判定依据 MUST
来自链本身而非调用方另给的切片；重排顺序 MUST 取**日志自身的顺序**，MUST NOT 先按记录顺序排序再比较。
重建 SHALL 复用 `agent-harness` 的装配判定，MUST NOT 另写第二份判定逻辑。读回 SHALL NOT 追加、修改或
重排链。

#### Scenario: A logged assembly is reproduced from the chain alone

- **WHEN** 一次可重建装配已追加进链，随后用仅携带标识与版本的记录从链重建
- **THEN** 重建结果可重建，且其片段 (标识, 来源标识) 序列与追加时的进入顺序一致

#### Scenario: Rebuilding reads the chain, not a caller-supplied slice

- **WHEN** 链中不存在任何 `assembly` 条目，而调用方持有一个非空记录
- **THEN** 重建报告记录引用的片段**全部**不在日志里，MUST NOT 因调用方能提供片段而判为可重建

### Requirement: An allowed promotion into supervised automatic reaches the hash chain as its enablement record

装配根 SHALL 提供一处把一次爬坡晋升交给只追加哈希链的入口，且该入口 MUST 先经 `agent-harness` 的晋升
判定：只有判定**放行**且判定产出了一份启用记录时，才追加一条 `approval` 类条目。追加的载荷 MUST
**恰为**判定放行的那份材料——机制名、起始阶段、目标阶段，以及启用记录自带的那组字段；
MUST NOT 夹带判定未审过的字段（尤其请求携带的那两份回归类别集）。追加 SHALL 复用装配根既有的单 owner
决策写入路径与逐次调用预算，MUST NOT 另开第二条写入路径。追加之后链 SHALL 校验通过。
**该条目按 `approval` 类落账**（critical 类）⇒ 写入失败 MUST 原样上报，MUST NOT 静默进遥测缓冲：
启用记录是「要么落地、要么被报告」的决定，不是可丢遥测。

#### Scenario: An allowed promotion into supervised automatic is appended

- **WHEN** 一次晋升到受监督自动经判定放行，且启用记录齐备
- **THEN** 决策日志追加一条 `approval` 条目，其载荷含机制、起始阶段与目标阶段，且哈希链校验通过

#### Scenario: The appended payload is the judged material, not the request's regression classes

- **WHEN** 一次放行的晋升，其请求携带了受影响的回归类别集与已通过类别集
- **THEN** 追加载荷**不含**这两份类别集；把它们一并落账的实现不满足本需求

#### Scenario: A promotion below the gated stage is allowed without a record

- **WHEN** 一次晋升的目标阶段在受监督自动**之前**，且确为前进
- **THEN** 入口返回「放行但无需记录」这一具名结局，且决策日志**未新增**任何条目

### Requirement: A refused promotion is named and leaves the chain untouched

判定为**拒绝**的晋升 MUST NOT 触发任何追加：入口 MUST 返回一处**具名拒绝**，逐条列出判定给出的拒绝名，
并附上判定给出的**全部**缺失回归类别与值内含明文凭据的字段名供调用方查看。入口的返回 MUST 可区分
「已落记录」「放行但无需记录」「已拒绝」三种结局。把「放行但无需记录」折进拒绝、把拒绝折进
「已落记录」、或把拒绝折叠成静默 `Ok` 的实现，均不满足本需求。拒绝路径 MUST NOT 追加任何条目，
且 MUST NOT 因其被返回而把凭据字面量写入链。

#### Scenario: A refused promotion appends nothing

- **WHEN** 一次晋升被判定拒绝（受影响的回归类别未全部通过、或启用记录值内含明文凭据）
- **THEN** 入口返回具名拒绝，且决策日志**未新增**任何条目

#### Scenario: Every missing regression category is named, not just the first

- **WHEN** 晋升声明的受影响类别里有多个未通过
- **THEN** 具名拒绝列出**全部**缺失类别；只报第一个的实现不满足本需求

#### Scenario: A refusal does not masquerade as success

- **WHEN** 一次晋升被判定拒绝
- **THEN** 入口的返回必须与「已落记录」可区分；以 `Ok` + 空载荷静默通过的实现不满足本需求

### Requirement: An assembly that may not be called is named and leaves the chain untouched

装配根 SHALL 在把一次装配交给只追加哈希链之前先跑 `agent-harness` 的 §41.1–§41.3 判定，且该判定
MUST NOT 被运行期短路：判定为**不可调用**（不可裁剪段缺失或被裁、段序错乱、裁剪落在前段、裁剪缺被裁项
或缺原因、某段缺自有配额、缺预算维度、不可裁段缺固定下限、任一分量超限、片段缺来源标识或作用域、
不可信来源被标为指令）时 MUST **具名拒绝**，且 MUST NOT 追加任何条目。拒绝 MUST 逐条列出两项判定
各自给出的拒绝名，**两类都报**，MUST NOT 只报先命中的那一类。入口的返回 MUST 可区分「不可调用」
「不可重建」与「两者皆拒」三种拒绝，MUST NOT 把其中任意两种折叠成同一种结局。

#### Scenario: An uncroppable segment is missing and nothing is appended

- **WHEN** 一次装配缺少 §41.1 的三个不可裁剪段之一
- **THEN** 入口具名拒绝（含 `assembly.uncroppable_segment_missing`），且决策日志**未新增**任何条目

#### Scenario: Both judgements refuse, and both are named

- **WHEN** 一次装配既不可调用（如裁剪落在前段）又不可重建（如某片段正文不在日志里）
- **THEN** 拒绝里同时出现 `assembly.*` 与 `rebuild.*` 两类拒绝名；
  只报其中一类、或报了一类就停下的实现不满足本需求

#### Scenario: The three refusals are distinguishable

- **WHEN** 分别构造「仅不可调用而可重建」「仅不可重建而可调用」「两者皆拒」的三次装配
- **THEN** 三次的返回 MUST 分属三种不同的具名结局；把它们折成同一个结局的实现不满足本需求

### Requirement: The logged payload carries per-segment usage and the crops with their reasons

追加的 `assembly` 条目其载荷 MUST 含 §41.5 列明的**四项**：进入的片段标识、**各段预算占用**、
**裁剪项与原因**、模型与提示词版本。各段预算占用 MUST 按段给出该段的 Token、字符与条数三个维度的用量，
并在该段自带配额时一并给出该配额；裁剪项与原因 MUST 按段**同时**给出被裁项标识与原因原文。
载荷的取值 MUST 全部来自两项判定**审过**的输入，runtime MUST NOT 自行推算、补零或改写；
判定未审过的字段（如片段的时效、数据/指令标注，以及判定产出的诊断清单）MUST NOT 落账。

#### Scenario: A cropped assembly lands with its usage and crops

- **WHEN** 一次装配的某可裁段被裁剪，且其被判定的输入同时记了用量、被裁项与原因
- **THEN** 落账载荷该段的裁剪记录含被裁项标识与原因原文，且各段的 Token、字符与条数用量齐备

#### Scenario: A crop record with only one half does not land

- **WHEN** 实现只落裁剪的原因而不落被裁项（或反之）
- **THEN** 该实现不满足本需求——§41.5 要求的是「裁剪项与原因」两半

#### Scenario: Usage is the judged input, not a recomputation

- **WHEN** 某段在判定输入里的用量与该段片段正文的实际长度不符
- **THEN** 落账的用量 MUST 是判定输入里的那一份；按正文重新计算或补零的实现不满足本需求

### Requirement: The two inputs must describe the same fragments

入口 MUST 校验判定输入的各段片段标识序列与其记录声明的进入片段标识序列相同；不同 MUST **具名拒绝**，
且 MUST NOT 追加任何条目。依据是 **§41.5 的四项描述的是同一次装配**：若把两组不一致的标识都落账，
同一条链记录会自相矛盾——「各段预算占用」说这一段进了某个片段，而「进入的片段标识」说进的是另一个。

#### Scenario: Disagreeing inputs are refused, not logged

- **WHEN** 判定输入里出现的片段标识序列与记录声明的进入片段标识序列不一致
- **THEN** 入口具名拒绝，且决策日志**未新增**任何条目；把两组标识一并落账的实现不满足本需求

#### Scenario: Equal sequences are required, not equal sets

- **WHEN** 两份输入含同一组标识但顺序不同
- **THEN** 具名拒绝——四项描述同一次装配，而进入顺序是其中一项已声明的要求，
  集合相等不足以放行

### Requirement: Logged assemblies are read back row by row, with their usage and crops

装配根 SHALL 提供一处**只读**入口，把链上的 `assembly` 条目**逐行**读回，每行携带该行落账的四项；
读回 MUST NOT 追加、修改或重排链。**逐行**是必需的：把多行合并成一个池会让「各段预算占用」失去
可归属的行。一行不可解析 MUST 是**具名失败**，MUST NOT 被静默跳过——静默跳过会让「读回了 N 行」
与「链上有 N 行」无法区分。**早于本次接线落下的行**（其载荷未含预算与裁剪两项）MUST 仍可被读回，
其缺失的两项 MUST 读作**空**，MUST NOT 被读作解析失败。

#### Scenario: Every logged row is read back with its own usage and crops

- **WHEN** 两次装配各自落链，其中一次含裁剪
- **THEN** 只读入口返回**两行**，每行各带自己的片段序列、预算占用与裁剪记录，互不合并

#### Scenario: An unparsable row is a named failure, not a skip

- **WHEN** 链上某条 `assembly` 条目的载荷不是本入口认可的形状
- **THEN** 入口返回具名错误；静默跳过该行而返回其余行的实现不满足本需求

#### Scenario: A row logged before the payload carried usage and crops

- **WHEN** 链上存在一条不含各段预算占用与裁剪两项的既有 `assembly` 条目
- **THEN** 该行仍被读回，其预算占用与裁剪读作**空**；把它读作解析失败、或跳过该行的实现不满足本需求

#### Scenario: Reading the log appends nothing

- **WHEN** 调用只读入口读回装配行
- **THEN** 决策日志**未新增**任何条目，链的既有序与哈希不变

### Requirement: A judged memory write lands in the real memory store, and a refused one is named and lands nothing

组合根 SHALL 提供一处把 §42.2 的记忆写入判定接上**真实记忆存储**的入口：**先**跑判定，
**放行**才把该记录写进记忆存储；**拒绝**时 MUST **具名拒绝**且 MUST **一列不落库**。
判定 MUST NOT 被短路，MUST NOT 在存储层重跑——一条门槛 MUST 只答一次（存储层只做存储层的事：
校验作用域标识、查权限、写列、把主键冲突具名报出）。落库的每一列 MUST 逐字取自判定**审过**的输入：
runtime MUST NOT 由正文重算任何一列、MUST NOT 补默认值、MUST NOT 落判定未审过或存储中无对应列的字段。
写入 MUST 复用组合根既有的单 owner 存储路径，MUST NOT 另开第二条写入路径。

#### Scenario: An admitted write lands exactly one row with the judged columns

- **WHEN** 一次记忆候选在来源、作用域、来源标识、时间与「叙述还是权威事实」的标注上都过门槛
- **THEN** 该记录落进记忆存储**恰一行**，且每一列与判定输入**逐字相等**

#### Scenario: A refused write is named and leaves the store empty

- **WHEN** 某候选的来源是不可信来源（工具返回、外部智能体返回、评论原文或模型草稿）
- **THEN** 入口**具名拒绝**，且记忆存储**未新增**任何行；先落库再判定的实现不满足本需求

#### Scenario: The capability switch is answered once, on the write face too

- **WHEN** 观众长期记忆未启用，而候选是一条观众记录
- **THEN** 写入被具名拒绝且零落库；只判读取面、写入面照旧放行的实现不满足本需求

### Requirement: The memory store read-back is a per-column fetch, not the read judgement

存储层 SHALL 提供一处**只读**入口，把记忆存储按作用域读回，逐列还原一条记忆记录的九个字段；
读回 MUST NOT 追加、修改或删除任何行，MUST NOT 施加 §42.3 的读取判定（越作用域、过期、撤销、
权限不足、近似顶替与配额）。
**撤销状态在记忆存储中没有任何承载列** ⇒ 依赖它的判定 MUST NOT 在缺少该事实时被假装执行；
该入口 MUST NOT 被读作「记忆读取面已接线」。

#### Scenario: Stored columns come back verbatim

- **WHEN** 一条记录已经落库，且按它自己的作用域读回
- **THEN** 读回入口返回该记录，九个字段**逐字相等**

#### Scenario: The read-back does not cross scopes

- **WHEN** 另一个会话存在记录，而按本会话读回
- **THEN** 返回结果 MUST NOT 含那一条记录

#### Scenario: The read-back judges nothing

- **WHEN** 某条已落库记录**已过期**
- **THEN** 读回入口**仍然**返回它——时效判定属 §42.3 的读取面，不由取数入口施加；
  返回结果里 MUST NOT 出现任何 §42.3 的条目级拒因
