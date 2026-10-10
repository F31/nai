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
（`Arc<Mutex<Session>>`），使撤权产生的 epoch 递增对两侧立即可见。锁竞争 MUST 以显式冲突错误
处理，且 MUST NOT 阻塞或回退已执行的媒体静音。

#### Scenario: HTTP revoke visible to supervisor
- **WHEN** 控制面执行静音/接管使会话撤权
- **THEN** 装配根随后签发的租约与记录的决策日志使用递增后的 epoch；keeper 持有的旧 epoch 租约失效

#### Scenario: Session lock busy
- **WHEN** 会话锁被占用时到达控制或故障处置
- **THEN** 返回显式冲突错误；已执行的 P0 静音不回退

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
子进程故障后 MUST NOT 自动重启输出。

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
