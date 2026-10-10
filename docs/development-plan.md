# nai 开发任务与验收跟踪

基线：[V3.6](直播数字人%20%2B%20智能体系统技术方案%20V3.6.md)（ADR 0015 升基线；V3.5 仅追溯）。
更新日期：2026-10-09。验收唯一来源 [tests/registry.yaml](../tests/registry.yaml)。

## 当前检查点

已实现 **D01–D11**：工程/生成契约、存储/密钥、内核绑定、宿主/IPC、会话/媒体租约、有界调度与共享预算、API Provider 传输、
知识完整链路（算法/发布/冻结/远端门控）、智能体/守门评估内核与媒体样本时轴内核。
已补充 D12 模型包完整性前置检查、D13 平台事件准入逻辑；本轮 D14 本机控制/桌面客户端闭环实测通过。
ADR 0015 已把基线升为 V3.6 并完成勘误：验收切换为 tests/registry.yaml（S/A/R/O/X + release_line），
D01–D14 已回填映射（D12 标 blocked、D13 标 implemented、D14 标 tested，其余 D01–D11 均标 tested）。
Linux/WSL 当前全 workspace 324 项 Rust 测试、生成无漂移通过。本轮 engine-broker-frame-delivery 以真实 broker+keeper+render-worker 完成单帧投递与有界监督/故障撤权；contracts suite 实际完成 Rust/TS/C++ 编译及十三类消息双向互通。
`pnpm test:control-smoke` 实际启动自有控制进程并由桌面客户端读取状态、执行 mute/takeover，通过；不等于浏览器人工或声卡验收。
真实冒烟对 api.openai.com 验证传输/鉴权/429/403 映射；账户无余额，付费生成未完成（Blocked），未计费。
Windows provider/storage GNU 交叉检查缺 MinGW C 工具链（aws-lc-sys/SQLite），如实记录。
原生凭据、Named Pipe、AppContainer/Job Object、设备/SDK/GPU/安装认证未执行。

**D12 写实模型管线**：模型声明/内容完整性前置检查已实现；avatar-ort、离线导出与模型特定管线尚未实现。
真实模型包/授权与 Windows GPU 目标机证据缺失，G0–G3 未执行（Blocked）。
**D13 平台与网关**：连接/官方事件准入、断线重连与消费复检已实现/实测；官方 SDK、生产宿主接线和网关服务尚未实现。
**D14 控制 API 与桌面**：本机工程 status/session/mute/takeover 与桌面接线已实现；W09 增加 list/approve/reject API 与存储原子日志；完整开播预检、审批 GUI/证据详情/配置、正式 Tauri/受保护 IPC 尚未实现。
下一主线为 W05 output-keeper 与 W06 render-worker 的 Slice-0 接线，W09 审批/预检及 W10 进程强杀故障演练随依赖推进；D12/D13 真实集成证据继续保留 Blocked。
D11 已完成并归档：`openspec/changes/archive/2026-10-08-d11-media-basics/`。
Windows 独立窗口/音轨、虚拟摄像头/音频设备与真实媒体进程仍待设备里程碑。
D01 变更已归档为 `openspec/changes/archive/2026-10-07-d01-engineering-foundation/`，
10/10 任务完成，2 个主规格及归档任务均校验通过。
D02 变更已归档为 `openspec/changes/archive/2026-10-07-d02-versioned-contracts/`，
10/10 任务完成。
D03 变更已归档为 `openspec/changes/archive/2026-10-07-d03-storage-and-credentials/`，
10/10 任务完成。
D04 变更已归档为 `openspec/changes/archive/2026-10-07-d04-kernel-registration-and-binding/`，
9/9 任务完成。
D05 变更已归档为 `openspec/changes/archive/2026-10-07-d05-plugin-hosts-and-ipc/`，
9/9 任务完成。
D06 变更已归档为 `openspec/changes/archive/2026-10-07-d06-session-leases-and-cancellation/`，
6/6 任务完成，新增 4 条需求；8 个主规格与 D01–D06 六项归档通过校验。
D07 变更已归档为 `openspec/changes/archive/2026-10-07-d07-scheduling-and-budgets/`，
8/8 任务完成，新增 4 条需求；9 个主规格与 D01–D07 七项归档通过校验。
D08 变更已归档为 `openspec/changes/archive/2026-10-08-d08-api-provider/`，
8/8 任务完成，新增 4 条需求；10 个主规格与 D01–D08 八项归档通过校验。
D09 变更已归档为 `openspec/changes/archive/2026-10-08-d09-knowledge-pipeline/`，
11/11 任务完成，新增 10 条需求；10 个主规格与 D01–D09 九项归档通过校验。
D10 变更已归档为 `openspec/changes/archive/2026-10-08-d10-agent-and-guard/`，
8/8 任务完成，新增 7 条需求；10 个主规格与 D01–D10 十项归档通过校验。
D11 变更已归档为 `openspec/changes/archive/2026-10-08-d11-media-basics/`，
8/8 任务完成，新增 7 条需求；13 个主规格与 D01–D11 十一项归档通过校验。
详见 [工程流程](engineering.md)、[契约指南](contracts/README.md)、
[内核指南](kernel.md)、[宿主/IPC 指南](hosts.md)、[会话/租约指南](session.md)、[调度/预算指南](scheduler.md)、[Provider 指南](provider.md)、
[D01 实测报告](reports/2026-10-07-d01-engineering-foundation.md)、
[D02 实测报告](reports/2026-10-07-d02-versioned-contracts.md)、
[存储指南](storage.md)、[D03 实测报告](reports/2026-10-07-d03-storage-and-credentials.md)、
[D04 实测报告](reports/2026-10-07-d04-kernel-registration-and-binding.md) 和
[D05 实测报告](reports/2026-10-07-d05-plugin-hosts-and-ipc.md)、
[D06 实测报告](reports/2026-10-07-d06-session-leases-and-cancellation.md)、
[D07 实测报告](reports/2026-10-07-d07-scheduling-and-budgets.md)、
[D08 实测报告](reports/2026-10-07-d08-api-provider.md)、
[知识指南](knowledge.md)、[D09 基础报告](reports/2026-10-08-d09-knowledge-pipeline.md)、
[D09 发布报告](reports/2026-10-08-d09-knowledge-publication.md)、
[D09 完整实测报告](reports/2026-10-08-d09-knowledge.md)、
[D10 智能体/守门报告](reports/2026-10-08-d10-agent-and-guard.md)、
[媒体指南](media.md)、[D11 媒体基础报告](reports/2026-10-08-d11-media-basics.md)。
本轮 D12 前置范围见 [模型指南](models.md) 与 [完整性检查报告](reports/2026-10-08-d12-model-admission.md)。
前置变更已归档为 `openspec/changes/archive/2026-10-08-d12-model-admission/`，5/5 任务完成；14 个主规格与 12 项归档通过校验，D12 整体仍未完成。
本轮 D13 入口变更已归档为 `openspec/changes/archive/2026-10-08-d13-platform-ingress/`，5/5 任务完成；15 个主规格与 13 项归档通过校验，见 [平台指南](platform.md) 与 [实测报告](reports/2026-10-08-d13-platform-ingress.md)。
本轮 D14 本机控制变更已归档为 `openspec/changes/archive/2026-10-08-d14-local-control/`，6/6 任务完成；16 个主规格与 14 项归档通过校验，见 [控制指南](control.md) 与 [实测报告](reports/2026-10-08-d14-local-control.md)。D14 整体仍未完成。
本轮 D15 验收注册表与基线切换已归档为 `openspec/changes/archive/2026-10-08-d15-admission-registry/`，6/6 任务完成，主规格 `admission-registry` 生效；17 个主规格与 15 项归档通过校验，见 [实测报告](reports/2026-10-08-d15-admission-registry.md)。下一轮：decision_log 哈希链（R07 前置）与值守租约。

本轮 W02/W03（决策日志哈希链与值守租约，V3.6 工作项，不使用 D 编号）已归档为
`openspec/changes/archive/2026-10-08-w02-w03-decision-log-duty-lease/`，主规格 `decision-log-integrity` 与
`duty-lease` 生效；19 个主规格与 16 项归档通过校验，见
[实测报告](reports/2026-10-08-w02-w03-decision-log-duty-lease.md)。
R07 由 pending 更新为 implemented；S01 补值守租约证据。播放 ACK 生产端与桩 Provider 回放（W17）、
进程级强杀故障演练（R03 缺口）、output-keeper/render-worker（W05/W06）仍未开始。
接管审查补充 W02/W03 完整性修复：日志缓冲与新遥测同事务按序提交、critical不先冲刷遥测；静音/接管/停止清值守与自动模式、耗尽路径拒绝恢复自动。
实测新增6项回归（4 storage + 2 kernel），报告见 [审查与修复](reports/2026-10-08-w02-w03-admission-hardening.md)。本轮不升级R07验收状态。
修复变更已归档为 `openspec/changes/archive/2026-10-08-w02-w03-admission-hardening/`，5/5任务完成，19个主规格/17项归档校验通过。

本轮 W05/W10 输出看门狗演练已归档为 `openspec/changes/archive/2026-10-09-w05-w10-output-watchdog/`，6/6 任务完成，
主规格 `output-watchdog-process` 生效；20 个主规格与 18 项归档通过校验，见
[实测报告](reports/2026-10-08-w05-w10-output-watchdog.md) 与 [ADR 0016](adr/0016-output-watchdog-drill-boundary.md)。
R03 由缺口补为可移植证据（强杀源约10ms撤权、停续租578ms到期）、仍不标 tested；正式 W05 窗口/音轨/媒体协调与 W06 渲染、W10 真实引擎故障矩阵未开始。

W09 审批 API 子切片已实现/实测：唯一 Store owner 注入、list/read/approve/reject、200 候选入库上限、权限/epoch/哈希/CAS/seq/TTL 与 256 命令幂等账本；审批/拒绝与 critical 决策日志原子提交。
全 workspace 262 项 Rust 测试、portable、contracts、生成无漂移、前端检查和控制进程冒烟通过；见 [W09 报告](reports/2026-10-09-w09-approval-surface.md)。
已归档为 `openspec/changes/archive/2026-10-09-w09-approval-surface/`，6/6 任务完成；主规格 approval-workflow 生效，21 个主规格与 19 项归档校验通过。
S02 保持 implemented、O02 保持 pending；真实候选来源、审批 GUI/手机确认、证据详情、生产 root/媒体接线及 Windows 验收尚未完成。

W06 可移植2D核心已实现/实测：plugin层nai-avatar-2d、三层RGBA8/hash/许可准入、按样本窗口RMS/门限/攻击释放平滑、有限PCM/逐帧像素输出与epoch/取消/期限封闭。
9项新增测试与全workspace271项测试、portable、contracts、生成无漂移及前端检查通过；见 [2D指南](avatar-2d.md) 与 [W06报告](reports/2026-10-09-w06-avatar-2d-core.md)。
已归档为 `openspec/changes/archive/2026-10-09-w06-avatar-2d-core/`，6/6任务完成；22个主规格与20项归档校验通过。
下一主线：W05/W06共享内存数据面、独立worker/root装配与keeper帧消费/待机切换；A01保持implemented，实际设备/Windows帧率未验收。

W05/W06 帧槽位所有权与Linux tmpfs-file演练已实现/实测：provisional Protobuf ticket/commit/event、单owner状态机与代次隔离、16槽/8MiB/64MiB上限、SHA256完整性、超时断连退休整池；真实继承FD子进程写入/读回、部分写入强杀不产生Ready、新池绕过旧FD。
全workspace283项测试、portable、contracts十三类消息互通、schema无漂移、Windows GNU交叉检查及前端检查通过；见 [媒体指南](media.md)、[ADR 0017](adr/0017-frame-slot-drill-boundary.md) 与 [报告](reports/2026-10-09-w05-w06-frame-slots.md)。
已归档为 `openspec/changes/archive/2026-10-09-w05-w06-frame-slots/`，6/6任务完成；23个主规格与21项归档校验通过。
A01/R03保持implemented；独立render-worker/keeper生产装配、protected handle broker、命名管道身份与Windows映射仍待实现。

W05/W06 帧句柄授权 broker 与 keeper 帧消费/待机已实现/实测：broker 以 `/proc` 内核证据（声明 pid 存在、属 owner uid、持有 pool 文件）门控 producer/consumer 授权，未授权/错误 pid/超预算均无授权；keeper 在租约丢失、epoch 陈旧、peer 报错或池退休时切待机且 `output_available` 恒 false。
**连接对端未被认证**：`SO_PEERCRED`/`peer_cred` 在钉住的 rustc 1.96.0 上仍不稳定（E0658），本 workspace 禁 `unsafe` 且未引入新依赖；句柄交付仍为 spawn 继承，SCM_RIGHTS/Windows 命名管道未实现（Blocked，见 [ADR 0018](adr/0018-frame-handle-broker-boundary.md)）。
全workspace303项测试、portable、contracts十三类消息互通、schema无漂移与前端检查通过；见 [报告](reports/2026-10-09-w05-w06-frame-handle-broker.md) 与 [媒体指南](media.md)。
已归档为 `openspec/changes/archive/2026-10-09-w05-w06-frame-handle-broker/`，6/6任务完成；23个主规格与22项归档校验通过。
下一主线：独立 render-worker 生产装配与 composition root 接线；A01/R03 保持 implemented，不标 tested。

W05/W06 装配根与 render-worker 演练已实现/实测：`nai-runtime`（layer runtime）`Composition` 唯一存储 owner、显式 bootstrap 可信 owner 管理、运行时配置校验装配 Session/MediaGate/Registry/ApprovalBackend；control 与 `observe()` 固定先 P0 静音→撤权（epoch 递增）→critical 日志，日志失败不回退静音；值守授予/到期/撤权写 critical `duty_lease`。
`Supervisor` 驱动真实 keeper/render-worker 子进程（二进制路径显式传入）；`apps/render-worker --render-drill` 经 stderr 继承池 FD，`nai-avatar-2d` 真实像素写槽，`--ready` 预热解决冷启动超 250ms 槽位预算；不产生 PlaybackAck（见 [ADR 0019](adr/0019-composition-root-boundary.md)）。
全workspace318项测试、portable、contracts十三类消息互通、schema无漂移与前端检查通过；见 [运行时指南](runtime.md) 与 [报告](reports/2026-10-09-w05-w06-composition-root.md)。
已归档为 `openspec/changes/archive/2026-10-09-w05-w06-composition-root/`，6/6任务完成；24个主规格与23项归档校验通过。
下一主线：`apps/engine` 可执行+loopback HTTP 服务启动与 broker 装配接线；R01/R03/A01/S01 保持 implemented，不标 tested；设备/PlaybackAck/Windows 认证仍 Blocked。

引擎装配已实现/实测：控制面与装配根共享同一会话（撤权 epoch 立即对两端可见），`apps/engine` 以显式参数启动 loopback 控制服务、`--provision` 一次性可信 owner 配置、监督线程先续租再 `observe`（撤权后停止续租、不自动重签）、`--media-drill` 真实池+render-worker 提交与 consumer 读回、`--halt-file` 有界停机写 critical 日志。
全workspace323项测试、portable、contracts十三类消息互通、schema无漂移、fmt/clippy、前端typecheck、`pnpm test:engine-smoke` 与既有 `test:control-smoke` 通过；见 [引擎指南](engine.md)、[ADR 0020](adr/0020-engine-assembly-boundary.md) 与 [报告](reports/2026-10-09-engine-assembly.md)。
已归档为 `openspec/changes/archive/2026-10-09-engine-assembly/`，13/13任务完成；24个主规格与24项归档校验通过。
下一主线：protected IPC/信号停机替换 halt 文件、broker 在装配中接线（含 render-worker 侧客户端）与 keeper 帧投递；S09/R01/A01/O02 保持 implemented，不标 tested。

本轮 W10 引擎 broker 帧投递已归档为 `openspec/changes/archive/2026-10-09-engine-broker-frame-delivery/`，
见 [ADR 0021](adr/0021-engine-broker-frame-delivery.md) 与 [报告](reports/2026-10-09-engine-broker-frame-delivery.md)；
S09/R01/R03/A01 保持 implemented、O02 pending，均不标 tested（对端认证/SCM_RIGHTS/Windows 命名管道仍 Blocked）。

本轮 W10 A1 手工事件链路已实现/实测（ADR 0022）：`apps/engine` 新增 pipeline 模块
（归一化/去重 → 知识检索 → 智能体决策 → 守门四阶段 → 文本生成 → 决策日志哈希链），
`nai-provider-core` 新增 `TextGenerator` 接缝与 **rehearsal-only 确定性桩**（离线/可复现/无网络）；
以真实 engine 进程实测「同一手工事件 + 同一桩重放逐项一致、哈希链校验通过」，
守门缺失/陈旧 epoch/注入失败均不外发且先 P0 静音再撤权再 critical 日志。
workspace 测试、check-deps(17包)、portable(fmt/clippy)、registry 通过；contracts/schema 生成检查在本机
因 node 无法 spawn 子进程（EBUSY）未执行（本轮未改任何 protobuf/JSON schema/OpenAPI，catalog 通过），如实记录。
见 [引擎指南](engine.md)、[Provider 指南](provider.md)、[ADR 0022](adr/0022-engine-manual-event-pipeline-boundary.md)
与 [报告](reports/2026-10-10-engine-manual-event-pipeline.md)。
已归档为 `openspec/changes/archive/2026-10-10-engine-manual-event-pipeline/`，12/12 任务完成；24 个主规格与 26 项归档校验通过。
R07 取得桩回放判据、S01 补真实进程链路证据，均保持 implemented 不自动升级；真实 Provider、播放 ACK、设备与 Windows 认证未做。
下一主线：`engine-text-to-playback-ack`（TTS → 2D → 播放 ACK 段）。

本轮 `engine-text-to-playback-ack` 已实现/实测（ADR 0023）：链路终点从「文本」延到
「文本 → 语音合成 → 有界 `AudioBlock` 流 → 真实 RGBA 帧 → 双水位提交 → 实际播放完成 ACK」。
`nai-provider-core` 新增 `SpeechSynthesizer` 接缝与 **rehearsal-only 确定性语音桩**
（`rehearsal-speech-deterministic-v1`，样本总数只由文本与采样率决定，因此「预期样本总数」可在合成前
独立算出）；`nai-media-core` 新增 `playback` 播放通道，把「提交」与「播放完成」在类型与日志上分开，
帧总数与渲染器同一套整数算术；`apps/engine` 新增可选 `render` 闭包段（缺失即链路止于文本且行为不变，
给出即准入真实 2D 资产）。
只有「已提交 + `actual_local_playback_completed` + `played_sample_count = 预期样本总数」的 ACK 才记
`DecisionKind::PlaybackAck`；`partial`/`invalid`/`cancelled` 三个正常结局不记该 kind，摘要如实标
`playback_ack=false`；摘要恒标 `devices/output_available/real_tts/frame_slot/keeper_consume=false`，
帧只存在于内存（`delivery=memory`），演练播放槽是**软件替身**而非设备。
逐条门控：workspace 测试 317 项通过（13 项既有 Windows 平台差异失败，均不在本轮改动文件）、
check-deps(17 包)、registry、fmt、workspace clippy `-D warnings` 均通过；`cargo xtask check --profile
portable` 在本机**无法整体通过**（两个独立原因，见报告），故按其四步等价逐条单独执行并记录；
`schema --check`/`test --suite contracts` 仍因本机 node 无法 spawn 子进程（EBUSY）未执行（本轮未改任何
protobuf/JSON schema/OpenAPI，catalog 通过）。
见 [引擎指南](engine.md)、[媒体指南](media.md)、[Provider 指南](provider.md)、
[ADR 0023](adr/0023-engine-text-to-playback-ack-boundary.md) 与 [报告](reports/2026-10-10-engine-text-to-playback-ack.md)。
已归档为 `openspec/changes/archive/2026-10-10-engine-text-to-playback-ack/`，任务全数完成；
24 个主规格与 27 项归档校验通过。
R07 补「`PlaybackAck` 条目进入哈希链」证据、A01 补「文本→播放完成」链路证据，**均保持 implemented
不升级**（真实 TTS 与设备/硬件播放 ACK 仍缺）。
下一主线：真实 TTS/Provider 付费链路或帧槽位与 keeper 生产接线（`delivery` 从 `memory` 变为真实输出），
以及 protected IPC/信号停机替换 halt 文件；两者都需先明确设备与凭据条件。

本轮 `engineering-loop-platform-truths` 已实现/实测：把上一轮记的「13 项 Windows 平台差异」逐条追到
三个互相独立的根因，其中一项是**真缺陷**——`backup`/`restore` 各自 `File::open(..).sync_all()`，而 Windows 的
`FlushFileBuffers` 要求可写句柄，因此**备份与恢复在 Windows 上从未成功过**；抽出单一 `flush_durable` 供两处共用后，
storage `--lib` 由 4 项失败变为 **48 passed / 0 failed**。其余两项为**平台事实**：宿主对任何 stdin 为管道的子进程
都拒绝 spawn（`ERROR_PIPE_BUSY`），node `spawnSync` 同样受限（`EBUSY`）；原生子进程演练按同包 `frame_drill.rs` 的
既有约定改用 `#![cfg(target_os = "linux")]` 声明范围，并保留具名诊断；`0600` 断言则拆为 Unix 实测 +
非 Unix 特征化断言，不再把「碰巧通过」当成验证。另修 harness 自锁（检查逻辑移入 `xtask/src/lib.rs`，
测试步 `--exclude` 自身后再测其 lib，bin 仍被 clippy `--all-targets` 检查）与一处自相矛盾的时钟竞态。
`cargo xtask check --profile portable` **在本机首次整体通过：EXIT=0，63 个测试目标，321 passed / 0 failed**
（基线 272 passed / 13 failed）；宿主提交限制下需 `CARGO_BUILD_JOBS=2`，该开关未写进 portable 检查。
contracts/schema 生成检查与前端 pnpm 检查仍因本机工具链限制未执行。
见 [工程流程](engineering.md)、[存储指南](storage.md)、[ADR 0024](adr/0024-engineering-loop-platform-truths.md)
与 [报告](reports/2026-10-10-engineering-loop-platform-truths.md)。
已归档为 `openspec/changes/archive/2026-10-10-engineering-loop-platform-truths/`，任务全数完成；
主规格 `engineering-foundation`、`local-persistence` 增量生效。registry 不改动任何验收项状态。
下一主线：真实 TTS/Provider 付费链路或帧槽位与 keeper 生产接线（`delivery` 从 `memory` 变为真实输出），
以及 protected IPC/信号停机替换 halt 文件；两者都需先明确设备与凭据条件。

本轮 `w19-provider-substitution` 已实现/实测（ADR 0025）：把 registry A02「仅改配置换 Provider」从一句声明
变成**可执行判据**。`nai-provider-core` 传输接缝去硬编码——`client.rs` 新增 `AuthScheme`（`Bearer` 默认 |
`Header{name}`）、`extra_headers`、`provider_ref`（`Usage.provider_ref` 由 endpoint 改为稳定供应商身份），
`request()` 按 scheme 构造凭据头使共享路径对所有供应商相同；新增 `adapters/anthropic.rs`（Anthropic Messages）
与 `adapters/elevenlabs.rs`（ElevenLabs TTS）两种与既有形态不同的真实线上协议；新增 `binding.rs` 由配置字符串
选协议并派生凭据头/静态头，能力不符显式报错不降级；`tests/substitution.rs` 以两台本地 mock（无外网）用
**同一驱动函数**覆盖两家协议，同时断言「调用方可见一致」与「线上确实不同」（路径、凭据头名、请求体形状、
voice 在体内还是路径内），用量按供应商分别断言、不报 usage 的一律不编造，语音按句独立且不可达采样率显式拒绝。
provider-core 18 lib + 10 provider + 8 substitution = 36 项通过；零新增依赖。
`cargo xtask check --profile portable` 通过：EXIT=0，64 个测试目标，336 passed / 0 failed（基线 321）。
见 [Provider 指南](provider.md)、[ADR 0025](adr/0025-provider-substitution-boundary.md) 与
[报告](reports/2026-10-10-w19-provider-substitution.md)。
已归档为 `openspec/changes/archive/2026-10-10-w19-provider-substitution/`，任务全数完成；主规格
`api-provider-transport` 增量生效。A02/B03 补证据但**保持 implemented 不升级**（真实付费账号无余额 Blocked、
真实 TTS 设备与 engine 真实 provider 接线未做）；registry 不改动产品项状态。
下一主线：真实 TTS/Provider 付费链路（需凭据/余额）或帧槽位与 keeper 生产接线（`delivery` 从 `memory`
变为真实输出），以及 protected IPC/信号停机替换 halt 文件——均需先明确设备与凭据条件。

本轮 `w15-upgrade-core` 已实现/实测（ADR 0026）：把 W15 升级器基础从零落到**可执行编排**。
registry R05/R09/R10 由 `pending` 升 `implemented`（**不标 tested**——故障注入为内存演练宿主而非真实安装器/断电），
O05 按 O02 惯例只补证据**保持 pending**（其 `automation: ui` 而真实升级 UI 未建），S08 补撤销列表证据保持 `implemented`。
新增 shared 层 `crates/nai-upgrade-core`（零新增外部依赖），全部世界状态走可注入接缝
（`SignatureVerifier` / `RevocationList` / `SpaceProbe` / `ActiveSessionGate` / `SessionDrain` / `UpgradeHost` / `UpgradeJournal`）：
`verify.rs` 在**产生暂存之前**完成 schema→内容集合精确匹配→逐文件 SHA256→签名→撤销；
`revocation.rs` 把撤销列表做成**带期限的可信缓存**（命中拒绝、未过期放行、**过期拒绝**，安装与启动双入口）；
`preflight.rs` 六项检查三态化，**未知与不足同等拒绝**且错误码可区分；`plan.rs` 九步事务
（预检→暂存→备份→迁移→自检→原子换清单）+ **写前记录**的五状态 journal + 崩溃恢复幂等 + 回滚**导出**升级后写入；
`gates.rs` 门控与激活拆两次调用、降档**先排空后停用**、超 `rollback_range` 阻止静默回滚；
`cost.rs` 成本分项 `Known | Unknown`、**未知不冒充 0**。`nai-storage/src/upgrade.rs` 复用既有 `upgrade_journal` 表，
`xtask` 新增 `test --suite upgrade`。实测：upgrade-core 3 lib + 7 gates + 12 plan + 8 upgrade = 30 项通过；
storage lib 50 项；`cargo xtask check-deps` 18 包；clippy/fmt 干净；`cargo xtask check --profile portable`
EXIT=0（70 个测试目标，368 passed / 0 failed，基线 361）。见 [升级器指南](upgrade.md)、[ADR 0026](adr/0026-upgrade-core-boundary.md) 与
[报告](reports/2026-10-10-w15-upgrade-core.md)。已归档为 `openspec/changes/archive/2026-10-10-w15-upgrade-core/`。
未做边界：无真实签名链（`DeterministicVerifier` 为 sha2 替身）、无真实安装器/下载器、`semver` 只做交集判断、未接入装配根。

本轮 `w25-tier-idle-migration` 已实现/实测（ADR 0027）：补上 W15 的另一半，registry A08/A09/R11 由 pending 升 implemented
（**不标 tested**：A08/A09 为 `manual` 且需真实设备与长播，R11 需真实轻云实例）。**不新开 crate**——三项与 W15 共享同一条
不变量 §14.4「未启用的能力不占用进程、端口、显存或云请求」，拆开会让它有两个真相。
`residency.rs` 把足迹建模成**声明式**（封闭四族 `Process|Port|Vram|CloudRequest`）并**只在 `enable` 时物化**，
于是「未安装/未启用 ⇒ 零占用」是结构性质；只有 `Process`/`Port` 排他。空闲释放**按 TTL 分族**
（worker 30s / 端口随进程 30s / 未用预览 60s / 模型 300s，方向由重载代价决定），`reclaim` 只释放到期的并报告 `held_back`，
**否决**「场次一结束就全释放」；回收不动启用状态、不删数据、繁忙能力绝不回收。审计是**对公开快照值的纯函数**
`audit_residency`（六个稳定标记；`idle resource past its ttl` 是可达的泄漏检测，其余是改错时的守卫）。
`tier_plan` 把 Lite→Standard→Pro 做成**两段差集的并** ⇒ Lite→Pro 不需中间档，「不重装内核」是数据的性质。
`offline.rs` 缺件以 `CAPABILITY_NOT_READY` 拒绝并由 `missing_artifacts` 指认文件，之后复用同一条校验链；
「不会偷偷下载」是**编译期**保证（无 HTTP 客户端），**否决** `allow_fetch` 开关。`migration.rs` 搬动前拒绝三件事
（有活动会话 / 分布式 + SQLite 多写者 / 权限无映射即拒），搬动后复验权限集合、证据版本、数据哈希三项且错误码可区分
（**否决**「只比哈希」）；`OutputSwitch` 以 key 幂等 ⇒「输出切换不重复播报」。实测：upgrade-core 3 lib + 17 lifecycle +
11 migration + 12 plan + 8 upgrade = 51 项通过；`cargo xtask check-deps` 18 包；clippy/fmt 干净；
`cargo xtask check --profile portable` EXIT=0（数字见报告）。见 [部署指南](deployment.md)、
[ADR 0027](adr/0027-deployment-lifecycle-boundary.md) 与 [报告](reports/2026-10-10-w25-tier-idle-migration.md)。
未做边界：`Vram` 是声明式索赔而非真实配额；R06 长播、B04 延迟分位是独立主线。
下一主线：A11（Pro 多会话隔离），或 R06/B04 的设备侧测量与 O01 演示包。

本轮 `w26-cloud-disconnect` 已实现/实测（ADR 0028）：把 A10 的两条**否定式**判据落成可执行值，registry A10 由 pending 升 implemented
（**不标 tested**：`automation: fault` 的真实断网/弱网注入与真实云 SDK 未接）。「本地链路不依赖云」是**依赖方向图**上的传递闭包断言
（`LinkGraph` 节点分 Local/Cloud，`audit_local_autonomy` 对每个关键节点断言可达云集合为空；两跳依赖与直连同属可达；三类稳定标记
区分「依赖云」「不是本地」「未声明」）——**否决**「只查直连」「写成文档表格」「运行时探测作为唯一实现」。可用性是链路状态与归属的函数：
`(_, Local) => Available` 是一条恒等式，本地静音在断云时仍可用；`CapabilityGate::disconnect` 的循环**只遍历云条目**故本地不受影响，
`reconnect` 把云能力落到 `NeedsPrepare` 而非 `Active`（§27.3 不自动恢复旧 Auto，须显式 `activate`）。低云容量只能由
`Sizing::Measured(instances, load_pct)` 构造，`Unmeasured` 以 `CAPABILITY_NOT_READY` 拒绝且**无零默认值**（「没测过」与「测到 0」类型不同），
`assess` 报出**全部**越界族，超预算而策略为 `Absorb` 时 `audit_low_cloud` 报违规。实测：`tests/cloud.rs` 21 项通过；
`cargo xtask check-deps` 18 包；clippy/fmt 干净；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
见 [云与本地自治指南](cloud.md)、[ADR 0028](adr/0028-local-autonomy-boundary.md) 与 [报告](reports/2026-10-10-w26-cloud-disconnect.md)。
未做边界：真实断网/弱网注入与真实云 SDK 未接（云链路由宿主注入），容量数字是已测量声明的忠实表示而非本轮实测值。

本轮 `w27-multi-session-isolation` 已实现/实测（ADR 0029）：把 A11 的三句话落成**结构性**判据，registry A11 由 pending 升 implemented
（**不标 tested**：`automation: integration` 的真实多路会话与真实音频时序未做，准入用宿主给定的上限表）。落 **kernel**
（`crates/nai-kernel/src/multiplex.rs`，只依赖 contracts，18 包与依赖方向不变）——kernel 不许依赖 shared 层，且多会话是内核会话语义
（`session.rs`）的延伸。每会话命名空间以 `session_id` 为键并各有**独立 epoch**（`advance_epoch` 只推进被点会话）；到达 lane 的**唯一**入口
以**属主**为键，`requester ≠ owner` 即 `CONFLICT`，故「任务/音频/审批/预算/知识无串场」是**签名的性质**而非要记住的检查
（**否决**全局 lane 表+检查、类型级隔离、lane 不设属主）。共享面是**封闭三枚举** `ImmutableModel | KnowledgeSnapshot | ConnectionPool`，
活的 lane 状态**没有**可共享表示；`SharedPool::write` 永远 `PERMISSION_DENIED`（没有写路径才是「只读」的实现），连接池持有者上限满即
`RESOURCE_EXHAUSTED`，零上限在构造时即拒。`Admission` 四族（`GpuThroughput | Vram | AudioTiming | ApiQuota`）**独立判定**
（某族余量不能换另一族），`admit` 是**两阶段**——先按族汇总并对照全机总量检查，**全部通过才写用量**，任一族不足即 `RESOURCE_EXHAUSTED`
且不写任何用量（被拒会话为 0、已开播会话数字不变，对应「不能让已开播会话 OOM」），未声明族按 0。`audit_isolation` 检出无持有者租约与重复
session id。实测：`tests/multiplex.rs` 24 项通过；`cargo xtask check --registry` 通过；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
见 [多会话指南](multi-session.md)、[ADR 0029](adr/0029-multi-session-isolation-boundary.md) 与 [报告](reports/2026-10-10-w27-multi-session-isolation.md)。
未做边界：准入用宿主给定上限表、不探测硬件；跨节点会话路由与多会话 UI 未做。

本轮 `w28-talent-takeover` 已实现/实测（ADR 0030）：把 B05 从 registry 里「混音/回声未实现（W11）」这条**真的没写**的缺口落成 **media 层**的
结构性判据，registry B05 由 pending 升 implemented（**不标 tested**：真实声卡、真实 OBS/伴侣回声环境与真实设备默认变化未接入）。
落 `crates/nai-media-core/src/mixing.rs`（只依赖 contracts，18 包与依赖方向不变，仍无 HTTP 客户端）——§8.5 是**声音通路**语义，与 kernel
会话语义无交集，且 kernel 不许依赖 media；media 层自带 `MixError{code,reason}`（与 `UpgradeError`/`MultiplexError` 同形）。`takeover` 一次
做完四件（暂停自动、TTS 静音、清空队列并把**实际清空条数**作为报告值、可选接入真人轨），重复接管即 `CONFLICT`；「两轨不相加」由 `render()`
从**模式推导**静音轨，故 `{tts>0,talent>0}` **没有构造点**（否决两个增益 setter + 调用约定）；`CaptureRegistry` 只允许**一条**有效收音路径
（`acquire` 不看 owner/device 一律拒第二条——正是 OBS 与伴侣重复收音场景），`release` 只接受**当前代次**（旧租约 `STALE_EPOCH`，迟到释放掐不掉
新持有者）；`DeviceId` 是稳定不透明标识，`observe_default` 把默认设备变化报成 `Changed{from,to}` 并记住而**不静默跟随**；接管要求**新鲜通过**的
回声测试（未跑/运行中/失败→`CAPABILITY_NOT_READY`，过期→`EVIDENCE_EXPIRED`，未来时刻无效，TTL 5 分钟）；`resume_auto` 每次复核授权+事实版本+
值守租约，而 `on_talent_silence` **只读**返回当前模式，故「不因真人讲话结束而自动恢复」在 API 上**不可表达**（否决定时器与 `ResumeHint`）；
`audit_mix` 对**上报快照**做纯函数，六类稳定标记全部可构造。实测：`tests/mixing.rs` 18 项通过；`cargo xtask check --registry` 通过；
`cargo xtask check --profile portable` EXIT=0（数字见报告）。见 [混音指南](mixing.md)、[ADR 0030](adr/0030-talent-takeover-boundary.md) 与
[报告](reports/2026-10-10-w28-talent-takeover.md)。
未做边界：不打开设备、不采 PCM、不做回声抑制；keeper 进程内装配与真机验收未做。

本轮 `w29-operations-review` 已实现/实测（ADR 0031）：把 B04/R06/B06 的**判据口径**落成 **kernel** 的可执行判据（真值仍需设备与长时运行），
registry B04/R06/B06 由 pending 升 implemented（三项均**不标 tested**）。落 `crates/nai-kernel/src/metrics.rs` 与 `review.rs`（只依赖 contracts，
18 包与依赖方向不变）。**§12.5 那句写死的口径**落成两个不同的调用：`percentile` 只从**端到端样本**算，`stage_percentile` 各算各的，
`sum_of_stage_percentiles` 仅作**具名对照**且 `stage_sum_gap` 显式给出差距（否决插值分位与「端到端或阶段之和」开关）；分位用 nearest-rank
（不插值）故报告返回的必是真实样本、可与原始数据逐条对照；端到端样本少于 20 条以 `CAPABILITY_NOT_READY` 拒绝（十份样本的 P95 实际上就是
最大值）；`DESIGN_TARGETS` 收录 §12.5 的模板开口 1500ms / API 开口 3000ms / 控制响应 50ms，`evaluate_target` 返回 `Meets | Exceeds`，样本不足
返回 `Err` 而非达标。**长播判定**：窗口（2h/8h/24h）是**下界**，跨度或样本不足一律 `Insufficient` 并如实报出观察值，绝不报 `Stable`；比较两个
半段的**中位数**（末端尖峰不误判、起始低值不掩盖增长；否决回归斜率与首尾比较），`record` 要求时间非降序故趋势不能靠重排制造。**场次复盘报告**：
`SessionReview::build` 覆盖 §5.9 末条全部字段（守门命中分类 = §5.4 四阶段 × 四结果），构造即自洽（回答多于入选、拒答未被原因完全解释即拒），
`Ratio` 保留分子分母，缺测进 `unmeasured` 且留空而**不写 0**，未定价成本是 `CostView::Unknown` 而**不是 ¥0**；`audit_review` 对公开报告值做纯函数，
十条稳定标记全部可构造。实测：`tests/metrics.rs` 13 项、`tests/review.rs` 8 项通过；`cargo xtask check --registry` 通过；
`cargo xtask check --profile portable` EXIT=0（数字见报告）。见 [运营指南](operations.md)、[ADR 0031](adr/0031-operations-review-boundary.md) 与
[报告](reports/2026-10-10-w29-operations-review.md)。
未做边界：不采真实指标、不跑长播、不接真实场次数据、不做报告 UI；阈值为保守常量，未由真实数据校准。

### W30 黄金集知识质量门槛与回归门禁（B01）

B01 的**真值**（100 道贴近真实直播的问题、真实模型在这些题上的表现）需要外部场景包与真实模型评估，本轮不假装有；但 §6 的五条门槛与
§5.9 的三条生命周期语句此前的**判据口径**没有落点，于是「100 题黄金集未建」被执行成了「什么都不用做」。本轮把口径落进
`crates/nai-kernel/src/golden.rs`（只依赖 contracts，18 包与依赖方向不变，零新增依赖），registry B01 由 `pending` 升 `implemented`
（**不标 `tested`**：真实题集内容、真实模型评估与真实检索质量未接）。三处最容易含糊的地方被钉死：**①** §6 的「Recall@5 ≥90%」没有分母——
本轮定为**题级**（全部所需证据进前 5 才计命中），文档粒度召回只作**对照**且两口径分开呈现；若没有任何有证据用例，召回为**未测量**而非 0%。
**②** 「检索延迟、证据正确率、最终答案正确率分别报告」是一句**禁止令**——本轮落成三个独立字段，**没有任何合成方法**；`AnswerVerdict::Unknown`
（未判定）从正确率分母中**排除**。**③** 「必须通过相关回归才生效」是一道**门禁**而不是一次比较——本轮落成 `regression` 的四条拒绝：
版本不一致 `VERSION_INCOMPATIBLE`、候选任一门槛失败 `candidate_gate_failed`（基线不绿也不放行候选）、**逐用例**由通过变失败 `case_regressed`
（1000‰→989‰ 亦拒，不因平均分未降而放行）、召回或正确率低于基线 `recall_regressed`/`accuracy_regressed`。四条否定式判据（高风险事实不得自动
播报错误、无答案必须澄清或转人工、注入/越权/工具返回攻击不得有副作用、取消类必须以取消结束）为逐用例一票否决；检索 P95 复用 W29 的
`LatencySamples`（<20 样本即未测量，未测量的门槛**计为未达标**，恰好 300ms 判达标）；坏案例入池开放、出池须**具名确认人**
（`CaseOrigin::Promoted` 保存确认人，未确认候选无法伪装成 authored）；`audit_report` 对上报值做纯函数，五条稳定标记全部可构造。
实测：`tests/golden.rs` 24 项通过；`cargo xtask check --registry` 通过；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
见 [黄金集指南](golden-set.md)、[ADR 0032](adr/0032-knowledge-quality-gate-boundary.md) 与
[报告](reports/2026-10-10-w30-knowledge-quality-gate.md)。
未做边界：不做题集内容、不接真实模型评估（答案为调用方判定后注入）、不做评估 UI、不改检索管线。

### W31 黄金集载荷载体与就位门禁（B01 机制侧收口）

W30 之后「100 题黄金集未建」仍无落点：§30.1 指定的 `tests/rag-golden` 目录不存在，§6 的 100 题门槛标注为（设计值）且未指定生成方，
§5.9 把黄金集列为**场景包字段**。本轮给「题从哪里来」一个**机器可寻**的答案：**一处位置**（`tests/rag-golden/*.json`）+ **一份结构规范**
（`schemas/rag-golden/case-set.schema.json`）+ **一道就位门禁**（`cargo xtask check --golden [--dir]`）。门禁把三种结论**分开**报告：
目录内无 `*.json` ⇒ `CAPABILITY_NOT_READY`（**未提供**，指出被搜索目录，既不是通过也不是失败）、有文件但读不出 ⇒ 逐个报出**全部**结构缺陷
（文件名 + JSON 路径 + 稳定 token）、读出但不达 §5.9/§6 ⇒ 报全部 finding。`is_unsupplied()` 以 `files == 0` 为**唯一**判据，**否决**由
`sets.is_empty()` 推断（后者在「有文件但全被拒」时同样为真，会把「载荷是垃圾」误报成「从未提供」）。§5.9 的授权边界**前移**到载荷边界：
`origin.kind = promoted` 缺或空白 `confirmer` 在**读取载荷时**即拒，使含未确认候选的题集不会先参与召回统计、从而把题集缺陷误读成「系统在某类
问题上表现差」。落 `xtask`（tool 层）：复用 kernel 的 `GoldenSet`/`audit_coverage`/`MINIMUM_CASES`（**判定逻辑只有一个真相**，不复制类别表或
分位实现），且 xtask 的 **lib 目标在便携门控内**（`cargo test --package nai-xtask --lib`）故逻辑被工程门控覆盖；新增依赖边 `xtask → nai-kernel`
属 tool 层允许，`check-deps` 仍 **18 包**、方向不变，**外部依赖零新增**（复用已有 `serde_json`；YAML 需新增依赖且 Rust 侧读 YAML 无先例）。
**口径修正**：§655 的 B01 验收是「知识质量：**100 题集达到**第 6 章门槛」＝**实测达标**，题集不存在时不可能达成，故 registry B01 由
`implemented` **回调为 `blocked`**（已交付的判定机制与载荷载体在 notes 中完整保留）；**不新增 B01a/B01b 编号**（registry id 正则要求恰好两位
数字，拆分会使 `check --registry` 失败，且等于改写规格口径）。**本轮不产生任何题内容**：题集由场景包产出方按 §5.9 的三条来源（按场景人工
编写 / 随场景包发布 / 坏案例经具名人工确认回流）填入，`tests/fixtures/rag-golden/example-min.json` 是夹具（8 例最小集）不是真值。
实测：`cargo test -p nai-xtask --lib` 27 项通过（golden 17 项）；`check --golden` 对真实目录报 `files: 0` + `CAPABILITY_NOT_READY`、对夹具目录
报 `below_minimum_cases: 8 of 100`；`check --registry` 通过；`check-deps` 18 包；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
见 [黄金集指南](golden-set.md)、[ADR 0033](adr/0033-golden-set-payload-boundary.md) 与
[报告](reports/2026-10-10-w31-golden-set-payload.md)。
未做边界：不产生题内容、不自动出题（§5.9 要求人工确认）、真实模型评估与真实检索质量未接、不做评估 UI、schema 未接数据文件的自动校验
（载荷校验由 `check --golden` 承担）。
### W32 写实准入 Gate 判定骨架（B02 机制侧前置解耦）

B02 与 D12 的**真值**（合法完整模型 + Windows/CUDA 目标机 + 与直播工具并发实测）需外部条件，本轮不可达，两项保持 `blocked`。
但 §8.2 的表只有四行，每行的**判据**此前没有落点，于是「写实 Gate」在执行层等于一句无从判定的要求。本轮把 §8.2（并 §8.1 档位、§8.4 回退、
§26.5 执行后端、§12.5 帧率与音画偏差目标、§1311 纪律）的**判定口径**落进 `crates/nai-kernel/src/realistic.rs`（只依赖 contracts，
18 包与依赖方向不变，零新增依赖；复用 `metrics::LatencySamples` 与 `golden::Finding`）。四处最容易含糊的地方被钉死：
**①** 四门是**合取**而非可合成——G0/G1/G2/G3 各自 `Passed/Failed/Unmeasured`，总体 `Acceptance` 由四门派生，**无任何单分数**；
「缺测」既不是通过也不是零（帧率无窗口、音画偏差样本不足、温度未读一律 `Unmeasured` 且阻断 `Accepted`）。
**②** 「全 GPU」是**放置记录**的性质而非运行回执——`is_full_gpu()` 仅当放置记录非空且全为 GPU，`cuda_ep_registered` 只是审计线索
（§26.5「不以 CUDA EP 注册成功宣称全 GPU」）；宣称全 GPU 而无记录或存在 CPU 回落 ⇒ `Failed`，但**如实记录** CPU 回落且指标达标仍可 `Passed`。
**③** 「自动回退 2D 不等于写实验收通过」——回退是 §8.4 的**运营记录**、与准入门控**正交**，不进入 `verdict`/`acceptance`；
只有「以回退构成验收」的**声明**才违规（`fallback_claimed_as_acceptance`），回退缺授权/不在句间/未记日志分别被审计报出。
**④** §1311 纪律——无合法完整模型 ⇒ `Blocked`（缺模型却有门判通过被审计报出），NotApplicable 只用于**不要求写实**的档位（Lite）；
Standard/Pro 声明不适用被审计报出；**无豁免路径**。此外：G0 五类权利（权重/代码/训练素材/肖像/声音）依据**空白不算成立**
（「许可引用存在不等于证据通过」）；G1 四段对齐 + §26.5 精度有序（非 FP32 在 FP32 之前认证 ⇒ 拒；INT8 无量化校准 ⇒ 拒）；
G2 目标机 + 与直播工具并发 + 帧率**窗口下限**是否稳定达 25fps（否决会被突发拉高的平均）+ 音画偏差 P95 ≤80ms（<20 样本即未测量，
恰好 80ms 判达标）；G3 七场景且**通过须有证据**。`audit_gate` 对报告字段做纯函数，十四条稳定标记全部可构造。
**不新增 xtask 命令与证据目录**：§30.1 只为 RAG 黄金集指定了 `tests/rag-golden`，为写实 Gate **臆造**证据目录属过度绑定；
G2 真值来自目标机实测而非仓库载荷，待真实模型轨立项后再补载体（**无需改本模块判定**）。
实测：`tests/realistic.rs` 37 项通过；`cargo xtask check --registry` 通过；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
见 [写实准入指南](realistic-gate.md)、[ADR 0034](adr/0034-realistic-gate-verdict-boundary.md) 与
[报告](reports/2026-10-10-w32-realistic-gate-verdict.md)。
未做边界：不含任何真实证据（权重/授权文件/ORT 推理/目标机测量均由外部提供）、不接 ORT/CUDA/直播工具、不改
`model-package-integrity`（完整性 ≠ 认证）、不给写实进度分；B02 与 D12 保持 `blocked`。

W33 手机监控面判定骨架已实现/实测（ADR 0035）：**O03** 的真值（M0b PoC 定路线 A/B + 真机最小形态通过 S09）需外部条件，本轮不可达，
O03 保持 `blocked`（并**补齐了原先缺失的 blocked 原因**）；本轮做的是 **O03 的判定骨架**——§3.1 L5 与 §10.2 的越界口径此前**全库无落点**。
落 `crates/nai-kernel/src/monitoring.rs`（只依赖 contracts，18 包与依赖方向不变，零新增依赖），复用 `golden::Finding`。
**「不持有媒体时钟与密钥」落成白名单投影而非黑名单**：`MobileView` 结构上只含十个白名单字段，`LiveState` 的 `media_clock`/`secret`/
`raw_audio`/`raw_video`/`broadcast_control` 在它之上没有落脚处；`MobileView::field_names()` 与 `Exposure::ALLOWED` 标签集合被断言相等
（给视图加字段而漏登记即测试失败），`LiveState::sensitive_field_names()` 与投影字段集合被断言不相交（**否决**事后剔除：给 `LiveState`
加字段就会默认流出）。**最小形态是四项清单**（看板/审批/接管/紧急静音），缺项逐条指名，预览与推送后置不算缺陷。**「它不是手机开播」**
（§10.2）与「手机只做监控管理」（§742）落成禁止暴露类别 `BroadcastControl`。**预览四条件仅在启用时受检**（按需、真正编码的低码率、
关闭立即释放编码器与带宽、不得挤占正式出画），未启用时一律不产生缺陷（**否决**把「不适用」读成「不合规」）。**中继终止 TLS 不得宣称
端到端**：`relay_terminates_tls` 且宣称 E2E ⇒ 违规、宣称 E2E 而协议未审核 ⇒ 违规，**如实披露中继可读而不宣称 E2E 不算缺陷**（与 W32
「不以 CUDA EP 注册宣称全 GPU」同形）。**路线 A/B 前置逐条**（A 缺 HTTPS 域名或证书 / B 缺应用分发 / Tauri 2 成熟度未验证）；
`readiness` 三态且 `Defective` 优先——审计非空⇒`Defective`、否则未定案⇒`AwaitingRouteDecision`（**未定案是诚实状态而非缺陷**）、
已声明路线无违规⇒`Ready`。**远程命令带 S09 契约**（身份/会话与 epoch/序号/TTL/实际 ACK 缺项指名报出）且**高危不得开放给观察者**；
紧急静音可达、启用状态齐备、知识速补走审核流逐条可判。`audit_surface` 是描述符的纯函数，**十九条**稳定标记逐条可构造。
实测：`tests/monitoring.rs` 34 项通过；`cargo xtask check --registry` 通过；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
见 [手机监控面指南](mobile-monitoring.md)、[ADR 0035](adr/0035-mobile-monitoring-surface-boundary.md) 与
[报告](reports/2026-10-10-w33-mobile-monitoring-surface.md)。
未做边界：不含任何真实交付（移动端应用/证书/分发/中继/真机 S09 集成均由外部提供）、不替 M0b 做路线决策、不引入传输层、
不改 `platform` 模块（它管平台事件门）、不新增 xtask 命令与证据目录；O03 保持 `blocked`，S09 保持 `tested`。

W34 设备接入判定骨架已实现/实测（ADR 0036）：**O04**（安装与设备）三条验收（§1329「干净 Windows 安装且运行库可取得；音频设备变更不重复收音；
升级可回退」）的真值需外部条件（受控 Windows 机器 + 真实音频设备 + 真实回退演练），O04 保持 `blocked`；其 registry notes 此前只有一句
「Windows 安装未执行」。**其中两条已有落点**——安装侧由 `nai-upgrade-core` 的 `manifest`/`preflight` 覆盖、回退由 `plan`/`gates` 覆盖——
本轮补的是此前**全库没有落点**的**设备侧**（§1424 缺口清单原文「没有设备与硬件接入的抽象，也没有物理安全联锁」）。
落 `crates/nai-kernel/src/device.rs`（只依赖 contracts，18 包与依赖方向不变，零新增依赖），复用 `golden::Finding`。口径被钉死之处：
**①** 设备能力靠**声明**（§1469：动作枚举/取值范围/速率限制），缺项指名设备与动作。**②** 执行类必须**安全联锁 + 本地手动覆盖**（§1469/§1497），
纯 `Sense` 不适用——**否决**把「不适用」读成「不合规」（同 W33 预览未启用）。**③** 物理执行类推导 `risk_3`、**默认禁用**（§34.4）、需本地确认与
独立开关（§1469），`Sense` 标注物理执行为分类错误。**④** 有效风险 = `max(声明, 推导)`，**声明低于推导即拒**（§35.3）；声明方**可以**主动提高风险，
但**不能**自我降级。**⑤** 驱动与 SDK **不进内核进程**（§34.4）落成 `DeviceHostPlacement`，声明即违规（独立声明，不受是否有设备门控）。
**⑥** 热插拔/断线/重复/超时四类事件各有处置（§1498）且**写决策日志**（§1497），处置挂 `keeps_single_capture_path`；未声明设备时**不存在设备事件**，
故条件化。**⑦** 「音频设备变更不重复收音」（O04、§8.5「只允许一条有效收音路径」）落成 `duplicate_event_opens_second_capture_path`——判到**结果**，
不是「处置了」。**⑧** INV-23「实际权限不得超出清单声明与用户授权」落成使用侧审计（宿主强制执行、不靠自觉），未声明只报一次、不重复报越权。
**⑨** `readiness` 三态 `Blocked`/`Defective`/`Ready`，**先取审计**（`Defective` 优先，`Blocked` 判据是审计为空且无声明）。
`audit_host` 是描述符的纯函数，**十六条**稳定标记逐条可构造。
实测：`tests/device.rs` 31 项通过；`cargo xtask check --registry` 通过；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
见 [设备接入指南](device-admission.md)、[ADR 0036](adr/0036-device-admission-boundary.md) 与
[报告](reports/2026-10-10-w34-device-admission.md)。
未做边界：不含任何真实设备/驱动/SDK/WASAPI/PCM 混音/安装器、不重复 `preflight` 的安装检查、不重复 `CaptureRegistry` 的租约互斥、
不新增 xtask 命令与证据目录；O04 保持 `blocked`（安装侧另有 `nai-upgrade-core` 覆盖）。

W35 平台生命周期判定骨架已实现/实测（ADR 0037）：**P01**（平台生命周期）的验收（§1328「伴侣启动、退出、重连、切场次不产生旧房间播报；权限撤销停止相关功能」）
真值需外部条件（官方 SDK 认证 + 生产 composition root 平台宿主接线 + 真实启动/退出/端口冲突与网关），P01 保持 `blocked`；其 registry notes 此前只有一句
「官方 SDK/生产宿主未实现（D13）」。**D13 已交付的是平台事件准入**（`nai-kernel::platform` 的 `PlatformLink` 判「一个事件能不能进」），
本轮补的是此前**全库没有落点**的**桥接进程生命周期**。落 `crates/nai-kernel/src/bridge.rs`（只依赖 contracts，18 包与依赖方向不变，零新增依赖），
复用 `golden::Finding`。口径被钉死之处：**①** §27.1「桥接只做」四项落成**封闭集**，越界职责逐条指名（`duty_outside_bridge_scope`）。
**②** §1328 四类转换各有处置、**各清播报授权**、各写决策日志；未声明桥接职责时不存在转换，故条件化。
**③** 「不产生旧房间播报」判**两个面**——处置面 `transition_keeps_stale_broadcast` + 状态面 `stale_room_broadcast_permitted`
（授权绑定**房间与连接代次**，`may_broadcast` 是运行时判据）；状态面**不受门控**。
**④** §347 退出转待机（`exit_without_standby`）与重连不恢复旧自动权限，**都落在转换处置的声明上**（否决另设描述符级引擎状态）。
**⑤** §349 能力快照 `unknown` 取**最严格**（读作已授予即报出），「**只**关闭对应功能」判**两个方向**——欠关闭 `feature_active_without_capability`
与因能力变更过关闭 `feature_stopped_despite_capability`（后者带 `stopped_by_capability_change` 前置，避免误报用户主动关闭）。
**⑥** §119 平台中断**三态**（撤自动、保手工、保回放），条件化于 `bridge_lost`（Provider 等降级不欠此规则）。
**⑦** §347 **原文七类**情形（多次启动/端口占用/SDK 失联/消息重复/旧房间事件/时间漂移/权限撤销）各有处置与记录。
**⑧** §27.1/§1075 官方 SDK 只经固定路径与签名/nonce/令牌校验加载，条件化于「声明已核验」；无官方 SDK 时如实记
`UNVERIFIED_EXTERNAL_DEPENDENCY`、**不用 fake connector 冒充**（该错误码此前在 workspace 内从未被使用）；回调不同步等待 LLM 或磁盘。
**⑨** `readiness` 三态且**先取审计**；支点是三条**不受门控**的独立判据（陈旧房间授权、伪造替代、功能开关双向）。
`audit_bridge` 是描述符纯函数，**十九条**稳定标记逐条可构造。
实测：`tests/bridge.rs` 44 项通过；`cargo xtask check --registry` 通过；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
见 [平台生命周期指南](platform-lifecycle.md)、[ADR 0037](adr/0037-platform-lifecycle-boundary.md) 与
[报告](reports/2026-10-10-w35-platform-lifecycle.md)。
未做边界：不含任何真实交付（伴侣进程/官方 SDK 加载/握手/连接/网关/推流均由外部提供）、不重复 `PlatformLink` 的事件准入判据、
不改 `platform.rs`/`session.rs`、不新增 xtask 命令与证据目录；P01 保持 `blocked`。

下一主线：能在 portable 落地的机制项已全部完成（含 B02、O03 的判定骨架与 O04 的设备侧、P01 的平台生命周期判定骨架）；剩余 **O01**（需演示包与真实音频输出）、
**O02/O05**（UI 验收，按既有惯例 `pending`）与阻塞项 **B01**（题集与真实模型评估）、**B02/D12**（模型与目标机证据）、**O03**（M0b 定案与真机 S09）、
**O04**（干净 Windows 安装、真实音频设备变更与真实回退；安装侧已由 `nai-upgrade-core` 覆盖）、**P01**（官方 SDK 与生产宿主；判定机制已由 W35 交付，D13）均需外部条件。

## 任务依赖与状态

| ID | 任务 | 依赖 | 状态 | 完成证据 |
|---|---|---|---|---|
| D01 | 工程与 ADR | V3.5 | 已实现；portable 实测通过；Windows 未实测 | [报告](reports/2026-10-07-d01-engineering-foundation.md)、workspace/锁文件、9 项测试 |
| D02 | 契约与生成 | D01 | 已实现；Linux/WSL 契约实测通过；Windows 未实测 | [报告](reports/2026-10-07-d02-versioned-contracts.md)、27 能力、三语言生成/互通 |
| D03 | 存储和密钥 | D02 | 已实现；Linux/WSL 16 项实测通过；Windows 原生凭据 Blocked | [报告](reports/2026-10-07-d03-storage-and-credentials.md)、34 表、WAL 恢复、Windows 交叉检查 |
| D04 | 内核注册与绑定 | D02 | 已实现；Linux/WSL 21 项实测通过；Windows 未实测 | [报告](reports/2026-10-07-d04-kernel-registration-and-binding.md)、闭包/视图/冻结/降级 |
| D05 | 宿主与 IPC | D02/D04 | 已实现；Linux/WSL 23 项实测通过；Windows 原生边界 Blocked | [报告](reports/2026-10-07-d05-plugin-hosts-and-ipc.md)、帧/握手/ACL/生命周期/加载守卫 |
| D06 | 会话/租约/取消 | D03–D05 | portable 机制已实现；14 项新增实测通过；Windows 设备 Blocked | [报告](reports/2026-10-07-d06-session-leases-and-cancellation.md)、epoch/任务取消/命令去重/媒体门控/CAS；实际音轨待 D11 |
| D07 | 调度与预算 | D06 | 已实现；19 项新增 Rust 实测通过；真实 Provider 账单待 D08 | [报告](reports/2026-10-07-d07-scheduling-and-budgets.md)、有界队列/公平/债务/TTL/共享配额/持久派发 claim |
| D08 | API Provider | D02/D03/D07 | 传输/adapter 已实现；11 项新增实测通过；真实付费生成 Blocked（账户无余额） | [报告](reports/2026-10-07-d08-api-provider.md)、HTTP/SSE/二进制流/重试/取消/usage 接线；真实端点错误映射验证 |
| D09 | 知识完整链路 | D03/D08 | 已实现；发布/冻结/远端门控实测通过；真实远端 Blocked | [完整报告](reports/2026-10-08-d09-knowledge.md)、knowledge suite 55 项；已归档 |
| D10 | 智能体/守门 | D06–D09 | 已实现；评估内核实测通过；实际守门插件与真实平台未接入 | [报告](reports/2026-10-08-d10-agent-and-guard.md)、decision/四阶段 guard/skill/memory/show、178 项 Rust；已归档 |
| D11 | 媒体基础与 2D | D05/D06 | 媒体时序内核已实现；32 项媒体测试通过；Windows 设备 Blocked | [报告](reports/2026-10-08-d11-media-basics.md)、样本时轴/时钟/缓冲/录制/ACK、199 项 Rust；已归档 |
| D12 | 写实模型管线 | D11 + 完整合法模型 | 前置完整性检查已实现/实测；推理/导出未开始；真实 G0–G3 Blocked | [报告](reports/2026-10-08-d12-model-admission.md)、6 项新增测试；不等于写实管线完成 |
| D13 | 平台与网关 | D05/D06 + 官方资料/条件 | 准入逻辑已实现/实测；SDK/网关未开始；真实平台认证 Blocked | [报告](reports/2026-10-08-d13-platform-ingress.md)、8 项新增 Rust 测试、平台 epoch/代次三语言互通 |
| D14 | 控制 API 与桌面 | D06/D10/D11 | 本机工程状态/撤权闭环已实现实测；完整预检/审批/配置和 Tauri/IPC 未开始 | [报告](reports/2026-10-08-d14-local-control.md)、9 项新增 Rust + 5 项客户端测试、实际进程冒烟；不等于 D14 整体完成 |
| D15 | 验收注册表与基线切换 | D01–D14 + ADR 0015 | registry.yaml + D→映射已实现/实测；决策日志/值守租约下一轮 | [本轮报告](reports/2026-10-08-d15-admission-registry.md)、`cargo xtask check --registry`、44 项验收回填 |
| D16 | （V3.5 遗留：Pro 资源与团队） | D07/D14 | 未开始 | 后续按 V3.6 R3 切片推进 |
| D17 | 升级器（V3.5 遗留 → V3.6 W15） | D03–D05/D14 | 已实现（R05/R09/R10；真实安装器/签名链未接，不标 tested） | [报告](reports/2026-10-10-w15-upgrade-core.md)、[ADR 0026](adr/0026-upgrade-core-boundary.md) |
| D18 | （V3.5 遗留：远程渲染与扩展宿主） | 对应契约/profile | 未开始 | 后续按 V3.6 R2/R3 推进 |
| D19 | （V3.5 遗留：安装与完整认证） | 前述目标能力 | 未开始 | 后续按 V3.6 R1–R3 与 registry 验收推进 |

D 表是 V3.5 追溯；自 ADR 0015 起开发按 V3.6 切片发布线（Slice-0/R1/R2/R3/实验）与
`tests/registry.yaml` 验收推进，不再新增 D 编号。旧 T/E/U 仅追溯，不再新增。
任务顺序不缩减 Standard/Pro 功能目标。D12/D13 外部条件未满足时，继续独立任务；
对应真实认证保留 Blocked，不用演练结果替代。

## V3.6 切片工作项进展

| 工作项 | 目标 | 验收 | 状态 |
|---|---|---|---|
| W00 | 工程基线、xtask、契约管线、ADR 模板 | A04 | 已实现/实测（D01–D02 范围） |
| W01 | 契约 v1 冻结与三语言生成 | 生成无 diff | 已实现/实测（D02；provisional 状态待补） |
| W02 | 内核骨架与会话租约 | S03；值守租约 | 值守与控制撤权门控已实测；定时驱动/独立媒体接线待runtime |
| W03 | 存储与决策日志 | R04、R07 | 哈希链及原子有序冲刷/critical优先已实测；R07 桩 Provider 回放已在真实引擎链路实测（见 W10 行）；`PlaybackAck` 条目已进入哈希链（演练替身），真实设备播放 ACK 生产端仍待 |
| W04 | 宿主与 IPC | S11、A06 | 已实现/实测（D05）；wasm/remote 只留契约 |
| W05 | output-keeper | R01、R03、S12 | 看门狗演练进程已实现/实测：强杀源约10ms撤权、停止续租578ms到期、partial卡读250ms期限、keeper重启需更高epoch floor；帧槽位所有权与tmpfs-file传输见W06行；引擎broker授权keeper只读FD单帧消费与待机见W10行；正式窗口/音轨/媒体协调和P0设备独立通道未开始 | [报告](reports/2026-10-09-engine-broker-frame-delivery.md) |
| W06 | 2D render-worker | A01 | 2D软件像素生产核心与帧槽位所有权/tmpfs-file传输已实现/实测；引擎经broker声明spawn pid并继承写FD完成真实单帧见W10行；独立worker/keeper装配、protected handle broker与Windows帧率未完成 | [报告](reports/2026-10-09-w05-w06-frame-slots.md) |
| W07 | Provider 核心与适配器 | A02 前置 | 已实现/实测（D08）；新增 `TextGenerator` 接缝与 rehearsal-only 确定性桩（W10 A1），以及 `SpeechSynthesizer` 接缝与 rehearsal-only 语音桩（W10 A2）；真实付费生成与真实 TTS Blocked |
| W08 | 默认智能体、守门与手工平台 | S01–S04 | 已实现/实测（D10/D13） |
| W09 | 最小桌面控制台 | S02、R01、O02 | loopback 控制与审批 API 子切片已实现/实测；审批 GUI/真实候选来源/完整预检与生产 root 接线未开始 |
| W10 | 集成与故障注入 | R01–R03、R07、A01 | 引擎真实装配：控制面+监督+broker/keeper单帧投递、强杀撤权、预热期halt不提交已实测；手工事件链路（归一化/检索→决策→守门→生成→决策日志哈希链，同事件同桩重放一致，守门缺失/陈旧epoch/注入失败不外发）以真实 engine 进程实测，见 [报告](reports/2026-10-10-engine-manual-event-pipeline.md)；链路终点已延到「语音合成→真实RGBA帧→双水位提交→播放完成ACK」（仅完整完成落 `PlaybackAck`，演练软件替身非设备），见 [报告](reports/2026-10-10-engine-text-to-playback-ack.md)；真实 TTS、设备/硬件播放ACK与正式IPC未做 | [报告](reports/2026-10-10-engine-text-to-playback-ack.md) |

W11 及之后（R1 工作项）在对应切片启动时建任务卡；本表不预建未到切片的目录或空占位。

## D01 首轮交付范围

1. 创建 Rust workspace 与前端 workspace；按 ADR 0001 命名。
2. 锁定 Rust、Node、pnpm 及 OpenSpec，生成真实锁文件；契约生成工具组合由 D02 选定并验证。
3. 明确 portable 默认构建与 Windows 原生构建边界；不引入 GPU/SDK 作为普通 CI 前置依赖。
4. 建立 `nai-xtask` 与 `cargo xtask check-deps`，基于真实 metadata 检查依赖方向。
5. 建立与已实现内容匹配的 CI；未实现的任务命令不得用空成功占位。
6. 运行 fmt、编译、依赖检查及已有前端类型检查，记录命令与结果。

## D02 契约准备重点

- 唯一消息源使用 Protobuf；配置/插件/模型清单使用 JSON Schema；HTTP 使用 OpenAPI。
- 覆盖信封、错误、能力目录及子能力、事件、候选、审批、知识、媒体、会话状态。
- epoch/时间与大整数在 TS 侧采用字符串或明确安全转换。
- IPC 长度先验、64KiB 初始控制载荷限制、未知字段与版本兼容须可测试。
- 生成器与运行时版本统一锁定；重新生成无 diff 后才进入消费者实现。

## 认证追踪规则

- T01–T16：业务、故障、媒体、平台、成本与移动链路。
- E01–E10：同内核、扩展、隔离、资源及存储维护。
- U01–U12：安装升级、恢复、回滚与部署迁移。
- 写实 G0–G3：来源/授权、正确性、实时性、操作与稳定性。

T/E/U 与 G0–G3 当前全部尚未执行，D01–D04 工程/契约/存储/内核检查不替代产品认证。后续按实际 profile 建立逐项矩阵，包含版本、环境、输入集、
日志、结果与外部阻塞。NotApplicable 只用于 profile 未启用能力，不能掩盖未完成目标。
