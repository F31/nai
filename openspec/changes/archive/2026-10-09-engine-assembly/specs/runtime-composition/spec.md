# Spec Delta

## ADDED Requirements

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

`--media-drill` SHALL 只使用真实资产与真实子进程：`/dev/shm` 私有池、预热后的 render-worker、
reserve→commit→consumer 读回 owned 像素，并把结果记为 telemetry 证据。缺少 tmpfs、
缺少资产闭包或子进程失败 MUST 显式报错，MUST NOT 静默降级或用合成数据代替。
broker 的 UDS 授权与 keeper 帧投递路径本轮未接线，MUST NOT 被表述为已完成。

#### Scenario: Missing tmpfs or asset closure
- **WHEN** 非 Linux、缺少 `/dev/shm`、缺少 `--asset/--license` 或 worker 未按时就绪
- **THEN** 引擎显式报错并退出非零，不产生任何提交或 Ready 声明

#### Scenario: Drill commit
- **WHEN** 演练完成一次 reserve→commit
- **THEN** 引擎在决策日志记录 `media_drill_committed`（含 buffer_ref、SHA256 与字节数），
  且不产生 PlaybackAck
