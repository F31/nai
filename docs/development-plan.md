# nai 开发任务与验收跟踪

基线：[V3.7](直播数字人%20%2B%20智能体系统技术方案%20V3.7.md)（ADR 0042 升基线；V3.6/V3.5 仅追溯）。
更新日期：2026-10-10。验收唯一来源 [tests/registry.yaml](../tests/registry.yaml)。

## 当前检查点

已实现 **D01–D11**：工程/生成契约、存储/密钥、内核绑定、宿主/IPC、会话/媒体租约、有界调度与共享预算、API Provider 传输、
知识完整链路（算法/发布/冻结/远端门控）、智能体/守门评估内核与媒体样本时轴内核。
已补充 D12 模型包完整性前置检查、D13 平台事件准入逻辑；本轮 D14 本机控制/桌面客户端闭环实测通过。
ADR 0015 已把基线升为 V3.6 并完成勘误（V3.6 后由 ADR 0042 升为 V3.7）：验收切换为 tests/registry.yaml（S/R/A/B/O/P/M/X + release_line），
D01–D14 已回填映射（D12 标 blocked、D13 标 implemented、D14 标 tested，其余 D01–D11 均标 tested）。
Linux/WSL 当前全 workspace 324 项 Rust 测试、生成无漂移通过。本轮 engine-broker-frame-delivery 以真实 broker+keeper+render-worker 完成单帧投递与有界监督/故障撤权；contracts suite 实际完成 Rust/TS/C++ 编译及十三类消息双向互通。
`pnpm test:control-smoke` 实际启动自有控制进程并由桌面客户端读取状态、执行 mute/takeover，通过；不等于浏览器人工或声卡验收。
真实冒烟对 api.openai.com 验证传输/鉴权/429/403 映射；账户无余额，付费生成未完成（Blocked），未计费。
Windows provider/storage GNU 交叉检查缺 MinGW C 工具链（aws-lc-sys/SQLite），如实记录。
原生凭据、Named Pipe、AppContainer/Job Object、设备/SDK/GPU/安装认证未执行。

**D12 写实模型管线**：模型声明/内容完整性前置检查已实现；avatar-ort、离线导出与模型特定管线尚未实现。
真实模型包/授权与 Windows GPU 目标机证据缺失，G0–G3 未执行（Blocked）。
**D13 平台与网关**：连接/官方事件准入、断线重连与消费复检已实现/实测；官方 SDK、生产宿主接线和网关服务尚未实现。
**D14 控制 API 与桌面**：本机工程 status/session/mute/takeover 与桌面接线已实现；W09 增加 list/approve/reject API 与存储原子日志；W38 增加桌面审批面板（批准二次确认、动作前失效复核、如实披露文本缺口）与 §30.1 指定的 UI 层 `tests/ui`；完整开播预检、证据详情、模式切换界面与升级提示、正式 Tauri/受保护 IPC 尚未实现。
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

本轮 W02/W03（决策日志哈希链与值守租约，V3.7 工作项，不使用 D 编号）已归档为
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

W36 真实 Provider 接入纪律与真实调用证据已实现/实测（ADR 0038）：真实凭据到位后风险从「机制不存在」变为「接入会污染仓库或把事实读错」，
本轮把纪律固化成**可执行、可检查**的形态而非写进文档靠自觉。落 **`xtask/src/secrets.rs`**（tool 层，无依赖限制）与 **`scripts/provider-smoke.mjs`**
（Node 22，零新增依赖），**`provider-core` 零改动**、18 包与依赖方向不变：**①** §30.1/§1469/S07 落成 `cargo xtask check --secrets`，
扫描域取 **git 会发布的内容**（`git ls-files --cached --others --exclude-standard`）——被忽略的本地 `.env` **不在**域（否则本机无法持有凭据），
而**运行期新建、尚未暂存**的文件**在**域（否则检查只能在错误几乎铸成之后才发现），**两条测试各钉一个方向**；按**前缀＋最小长度表**
（`sk-`/`wk-`/`pk-`/`rk-` 32、`ghp_`/`github_pat_` 30、`glpat-` 18、`xox[baprs]-` 20、`AKIA` 16、`AIza` 30、`Bearer` 32）匹配，
命中只报**文件/行号/前缀族**、**不回显匹配内容**；扫描器源码**不得自命中**（测试夹具运行期拼装）。**②** 接入**只是配置**：
两家真实端点都是 OpenAI 兼容，`Protocol::OpenAiCompatible` 已同时 `serves` 文本与语音，故 `Protocol` 枚举、适配器集合与 `provider-core` **全部零改动**，
只新增一份 `ProviderProfile`。**③** 缺凭据 **= `CAPABILITY_NOT_READY`** 非零退出、打印缺失**变量名**、**不写出**任何证据或音频，
**绝不回退** `RehearsalTextGenerator`/`RehearsalSpeechSynthesizer`（§30.1 点名「用固定素材冒充」，D5）。**④** **可用性是账户状态**（F2）：
同端点逐模型记录，模型甲 `200`、模型乙 `403 insufficient_user_quota`（报文余量 `＄-0.000260`）、模型丙 `503 model_not_found`（渠道缺失记作**渠道事实**、
**不是**凭据无效），结论**只覆盖被测模型**（D6）。**⑤** 证据**只含事实、连凭据前缀都不含**：主机名/路径/方法/协议名/凭据**方案名**/状态码/网关错误码/
用量字段名/字节数/media type，**不记**请求头值、请求行原文与凭据片段（D7）。**⑥** 音频证据是**真实字节**，字节数取自响应体、media type 取自响应头，
空体/错误报文判为**未产出证据**（D8）。**⑦** 契约差异**以实测为准**（F3：音色裸名 `alex` 被 `400`＋`20047 Invalid voice.` 拒，端点要求的
`model:voice` 完整形式返回 `audio/mpeg`），**不静默归一化、不重试掩饰**，4xx 连同错误码如实记录且**不**读作「端点不可用」（D9）。
**⑧** 主机差异是**环境事实**（F4：本机对文本端点报 `CRYPT_E_REVOCATION_OFFLINE`，对语音端点不报），记为**主机**注记、**不**读作端点事实（D10）。
**⑨** **冒烟不是验收**（D11）：连通成功**只**新增连通性事实，**不改** B01/O01 登记状态。实测：`check --secrets` 全部待发布文件**零命中**；
`cargo test -p nai-xtask --lib` **35 项**通过；两条真实链路经**本栈**（`ProviderProfile → ProviderClient → 适配器 → 预算网关`）走通
（文本 `chunks=2 chars=2 terminal_chunk=true`；语音 `chunks=33` 且落真实音频字节；**本轮实测基于 `agnes-3.0-flash`**，
W37 修正适配器角色并改用 `agnes-2.5-flash` 后证据已重生成）；`check --registry` 通过；`check-deps` 18 包；
`cargo xtask check --profile portable` EXIT=0（数字见报告）。见 [接入指南](provider-live.md)、[ADR 0038](adr/0038-live-provider-evidence-boundary.md) 与
[报告](reports/2026-10-10-w36-live-provider-evidence.md)。未做边界：不挑选供应商、不保存凭据、不替账户充值；**不覆盖未知前缀的令牌**（不得据此宣称「仓库无任何密钥」）；
日志/诊断包/崩溃转储扫描仍未建（S07 保持 `implemented`）；不新增证据目录约定（§30.1 未为真实 Provider 证据指定目录）；
B01 保持 `blocked`（真实模型可用是**必要前提而非达标**）、O01 保持 `pending`（演练仍需人工）。

W37 文本请求的轮次呈现已实现/实测（ADR 0039）：W36 用真实凭据跑两条冒烟时，**经本栈**的文本链路在 `agnes-2.5-flash` 上被
`400 No user query found in messages` 拒绝。根因在**我们自己的适配器**——`permitted_context` 这个字段，Anthropic 适配器作为
`role: "user"` 发出（有注释说明理由），**OpenAI 适配器却作为 `role: "system"` 发出**，**同一语义字段两个相反角色**。W19 把该差异
当作「实测到的供应商差异」用测试钉住并写进对照表，但**记录差异不等于判断对错**；真实网关给出裁决（同一网关同一请求体，
`agnes-3.0-flash` 200、`agnes-2.5-flash` 400）⇒ **system-only 不是普遍合法形状**。本轮把 `openai.rs` 的 `messages[0].role`
由 `system` 改为 `user`（与 Anthropic 一致），并**把性质写进规格**（`api-provider-transport` 新增需求「permitted context 以恰好一条
user 轮呈现」，2 场景）而不只是改一行。**否决**「system+user 两条」（适配器 MUST NOT 自拟提示文本 = 发明内容）、**否决**「角色可配置」
（角色是最小合法形状而非供应商差异；做成配置就是把「哪种角色对」变成可配置的正确性）。**第二种问题**：修好角色后本栈冒烟返回
`chunks=1 chars=0`——链路通了却**没有可见输出**。实测根因是**推理模型与 token 上限的交互**（同一端点：`max_tokens=32` ⇒ 可见
`content` 为空、`reasoning_content` 67 字符、`finish_reason=length`；`max_tokens=256` ⇒ `"\n\n收到"`、542 字符、`stop`）：模型先流
`reasoning_content`（思考）再流可见 `content`，上限只够思考时冒烟会在**每项检查都通过**时报出**空心成功**。故两处冒烟的
`max_tokens` 由 32 提到 **256**（附注释说明理由），**不**把「可见输出为空」做成失败判据（那是参数与模型的交互，不是连通性故障）。
**第三种修正**：`sse_delta_keys` 原取**首帧**，推理模型的 `reasoning_content` 帧完全不被记录 ⇒ 改为记**所有增量帧键的并集**
（普通模型 `["content","role"]`、推理模型 `["content","reasoning_content","role"]`）。契约与共享组件**不变**（`Protocol`/`ProviderProfile`/
`ProviderClient`/SSE 解码/用量映射/预算接线），零新增依赖，18 包与依赖方向不变。勘误：`docs/provider.md` 对照表与 W19/W36 报告已加注记。
实测：`cargo test -p nai-provider-core --offline` **36 项**通过（lib 18 + provider 10 + substitution 8）；两处冒烟文本链路
由 400 变为 **200 且有可见输出**（本栈 `chunks=3 chars=4 terminal_chunk=true`）；`check --secrets` 680 文件零命中；`check --registry` 通过；
`check-deps` 18 包；portable 门控 EXIT=0（数字见报告）；前端 tsc×2 + `vite build` + desktop 5 项测试通过。
见 [ADR 0039](adr/0039-provider-message-turn.md) 与 [报告](reports/2026-10-10-w37-provider-message-turn.md)。
未做边界：不改 Anthropic 适配器、不改其余协议路径与用量字段名、不引入角色配置开关；B01 保持 `blocked`、O01 保持 `pending`（冒烟不是验收）。

W38 桌面审批面与 §30.1 的 UI 层已实现/实测：W09 交付了审批 **API** 与存储原子日志，但**没有界面**，且设计 §30.1 为 UI 层指定的
`tests/ui` 目录在仓库里**从未存在过**。本轮把两者都补上，并在摸清事实时撞到**两条硬约束**。**F1（未满足的验收）**：设计 §662（O02）与
§425（T-14）要求「审批按钮显示最终文本」，本栈做不到——`utterances` 表只保存 `text_hash`（`migrations/sqlite/0001-foundation.sql:10`），
审批列表接口也只返回哈希/元信息引用/期限/证据计数（`crates/nai-control-api/src/lib.rs:540`；`schemas/openapi/control.yaml:84-97`、
`ResourceView`/`ResourceItem` :279-299），载荷里**没有文本字段**。做法是界面**如实说**（`FINAL_TEXT_GAP` 写明「本载荷不含最终文本、
身份为候选哈希」并直接指向 §662），**不**把哈希当文本展示、**不**为此临时发明字段；补齐需存储侧保存可展示文本并同步契约，**不在本轮范围**，
已写进 O02 notes 的未满足项。**F2（时效）**：候选的有效性绑定 TTL 与会话代次（`crates/nai-storage/src/store.rs:731` 的 `list_pending_approvals`
在 SQL 里就按 `u.generation_epoch=?3` 与 `u.expires_at_ms>?5` 过滤），故「界面还显示着、但已经不该批」是正常会发生的事 ⇒ 动作前**复核新鲜度**
（`approvalFreshness` 比较面板载入时的快照与动作前的实时快照：会话换代/已不在最新队列/无有效期/已过期 ⇒ **不发请求**并给具名理由与下一步）。
决策 D1–D8：界面侧复核而非信任快照、批准二次确认而拒绝单步（批准是高危方向）、看不见文本就如实说、UI 层放 §30.1 **指定**的 `tests/ui`
且**不引入 DOM 库**、未实现的断言登记并点名前置、不改服务端与契约（Rust 侧**零改动**）、登记状态不变、零新增依赖。落
`apps/desktop/src/console.ts`（无 DOM 的可测逻辑：`approvalFreshness`/`nextApprovalStep`/`describeCommandResult`/`runApprovalDecision`/`UI_ASSERTIONS`）
与 `control.ts` 的 `approvals()`/`decideApproval()`（真实 `ApprovalCommand`：`candidate_hash` 取候选哈希、`hash_algorithm: sha256-canonical-v1`、
`seq` 单调、epoch/revision 取自真实视图、TTL 有界），界面为待审批面板 + 批准二次确认 + 拒绝。`tests/ui` **15 项**通过（11 项审批面：失效不发请求、
两步确认、命令字段逐字、假 ACK 不变成已批准且不重试、严格解析、控制 ACK 口径；4 项断言声明：恰为四条、状态确定、未实现点名前置）。
§30.1 四条里「模式切换条件」（缺 `POST /sessions/{id}/mode` 实现）与「升级与回滚提示」（缺 `nai-upgrade-core` 状态来源）登记为**未实现**并点名前置。
实测：`check --secrets` 689 文件零命中；`check --registry` 通过；`check-deps` 18 包；`check --profile portable` EXIT=0；两处 `tsc --noEmit` EXIT=0；
`vite build` 30 模块；desktop 5 项 + `test:ui` 15 项通过。见 [UI 层](ui.md)、[控制面指南](control.md) 与
[报告](reports/2026-10-10-w38-desktop-approval-surface.md)。未做边界：**不测渲染**（不引入 DOM 库 ⇒ 不构成浏览器/人工 UI 验收）；
`local_control` 未装配审批 backend（手工点开界面仍返回 `CAPABILITY_NOT_READY`）；真实候选来源/证据详情页/手机审核/生产 root 接线仍未建；
**O02 保持 `pending`**（界面出现不等于验收达标）、O05 未动。
（**计数勘误**：此前报告的「82 个测试目标」把 cargo 的 `Running ...nai-xtask.exe check --profile portable` 包装行多算了一个，
精确重数为 **81（64 Running + 17 Doc-tests）**；`675 passed / 0 failed` 不变。W36/W37 报告已加注记。）

W39 攻击者可控输入面的随机输入测试已实现/实测（S11 的「模糊」这一半）：S11 验收名是「IPC **模糊**与路径穿越」，
状态 `tested`，而同一行 notes 写着「**模糊测试待建**」，全仓库 `grep -ri fuzz` **零命中**——验收名里的一半没实现却被标成 `tested`，
属**登记与事实不一致**，比缺功能更危险。审计发现四个攻击者可控入口中**两个此前零测试**（`Envelope`/`CapabilityInvocation`/
`CancelRequest` 解码；`Handshake` 畸形 body），路径面只有 1 项手写用例，且「**撤销/期限先于内容解释**」的顺序不变式
此前只用**一个特定输入**钉过。本轮落 `crates/nai-plugin-hosts/tests/untrusted_input.rs`（**8 项**，**实现一行未改**、**零新增依赖**）：
断言**全域性与有界性**（任意字节 ⇒ `Ok` 或具名 `ErrorCode`、不 panic、不按输入无界分配；超长长度前缀在分配前拒；
取消/期限对**任意**输入都优先）与两条结构性质（随机 body 往返逐字节相同；**被拒的帧终止连接**，其后的合法帧不被继续服务）。
**非空洞性守卫**是关键：语料**结构化**构造危险形状并统计**实际命中**，计数为零即**测试失败**；守卫做过**反向自检**
（阈值调到不可能命中 ⇒ 两个测试确实失败并打印计数，随后恢复）。实测命中：帧流 `bodies=683/clean_eof=179/oversized=1982/truncated=1935`；
结构化 Envelope `invocation_decoded=1569/version_incompatible=601/invalid_argument=846/denied_or_absent=225/completed=185/cancel_acks=191`；
路径 `rejected=1927/accepted=121/traversal_rejected=651`。过程中 harness 逼出两处**真实语义**：**①** ACL 拒绝与能力缺失是
**result 级**错误——`dispatch` 把 `CapabilityResult{end:StreamError}` 写回对端后返回 `Ok(())`，故 `serve` 返回 `Ok` 不代表放行，
**只看返回值会把 225 次拒绝误计为成功**（harness 因此改为解析宿主写出的帧，并断言宿主只输出良构帧）；**②** 空 payload 是**合法**的
`CancelRequest`（protobuf 默认值），「看起来是垃圾」的帧会走到取消路径并收到 `CancelAck`。**口径**：这不是覆盖率导向模糊测试
（无 cargo-fuzz/libFuzzer、无 nightly、无独立语料目录），一律称「随机输入测试」；**阴性结果不是安全证明**——
本轮**未发现**缺陷，**不**据此宣称不存在缺陷；不新增仓库级证据目录（§30.1 未为模糊测试指定，per-crate `tests/` 是既有惯例）。
实测：`cargo test -p nai-plugin-hosts --offline` 6+3+8 全通过（约 0.5s）；`check --secrets/--registry/check-deps/--profile portable` 全通过。
见 [报告](reports/2026-10-11-w39-ipc-untrusted-input.md) 与 [宿主/IPC 指南](hosts.md)。未做边界：不测传输层对端身份
（`SO_PEERCRED` 在钉住 rustc 上仍不稳定、Windows Named Pipe peer PID/AppContainer/Job Object 未实测，Blocked）；
随机语料计数**不**是分支覆盖证据。

本轮 `w40-golden-eval-runner` 已实现/实测（ADR 0040）：把 B01 达标链条上**仓库内可控的两段**补齐——参考答案判据与评估执行体。
题集修订升到 2，每题**必填** `expected`（`must_include` 至少一条、`must_exclude` 可选），判定是 kernel 纯函数 `judge_answer`
（归一化子串匹配、**否定式优先**、空判据返 `Unknown` 而非 `Correct`）；**修订 1 不做双读**——同时接受两版会让一份漏写 `expected` 的
旧题集读成「没有参考答案要求」，静默改变用例含义（与 ADR 0033 拒绝未知字段同一理由）。新增执行体
`xtask/examples/golden_eval.rs`（与 `live_provider_smoke.rs` 同一惯例：真实链路放 `examples/`、不进 portable）：复用门禁**自己的**读取器
（`read_case_set`/`case_set_files` 与审计共用 `parse_case_set`，故「读得进」与「门禁放行」是同一件事），真实 `TextGenerator` 逐题生成并用
`GoldenCase::answer_verdict` 判定；缺题集/结构不合规/缺凭据一律 `CAPABILITY_NOT_READY` 且**不发起任何调用**（凭据只报变量名），真实调用失败退出 2。
**只驱动生成侧**：检索召回、检索 P95 与行为类判据（澄清/转义、无副作用、取消）具名打印为未驱动且**不填零**，并明示「不是 B01 达标」——
**否决**「用答案文本猜 `outcome` 喂 `evaluate`」：那会造出字段齐全但含义失真的报告（行为类**假失败**、注入类**假通过**，本路径没有副作用通道故恒为空）。
实测：夹具 `tests/fixtures/rag-golden/example-min.json`（修订 2）8 例 → **8/8 judged / 0 correct**（占位问题与夹具参考答案不匹配，正确率无意义，
只证链路通）；临时探针 1 例 → **1/1 correct**（证明判定双向往返可用）。缺口从**四段降到两段**（①100 题题集内容、④真实知识库内容，均外部投入），但**两段的充分性不同**：①到位即让门禁放行并出生成侧
`answer_accuracy`（零代码改动）；④到位**本身不产生**检索侧结论——未驱动项是**常量**（`golden_eval.rs` 的 `UNDRIVEN`，无条件打印），
检索侧与行为类判据要出数**仍需扩展执行体或另接采样器**。零新增包（仍 18），真实接线仅 `[dev-dependencies]` 故 portable 覆盖与耗时不变。
见 [报告](reports/2026-10-11-w40-golden-eval-runner.md)、[黄金集指南](golden-set.md) 与 [ADR 0040](adr/0040-golden-eval-runner-boundary.md)。

本轮 `w41-frontend-gate-profile` 已实现/实测（[ADR 0041](adr/0041-engineering-profile-frontend-boundary.md)）：`engineering-foundation` 规格从 D01 起就要求
「workspace 编译、测试和**前端检查**可执行」，但任务工具里没有这个入口——`check --profile portable` 六步全在 Rust 侧，前端一行都没有。
前端检查一直存在（两个前端包各自的 `typecheck`/`build`、桌面包 `test`、根 `test:ui`），却**从未被门控覆盖**：历次记录都把它们与 portable
**并列**写成人工结果，并列本身就说明「有人记得才有一次」。故缺口的准确性质是「前端检查**没有门控入口**」——portable 跑绿不能排除前端已破。
新增 **`cargo xtask check --profile full` = `portable` 全部步骤 + 前端门控**；`portable` 语义与历史口径不变（它的名字承诺「只要 Rust 工具链就能跑」），
两种 profile 共用 `engineering_checks`，故「full 包含 portable 全部步骤」是代码结构事实而非需同步的清单。`xtask/src/frontend.rs` 只管何时跑与失败怎么报；
`scripts/frontend-gate.mjs` 是「前端怎么跑」的单一真源：命令取自各包 `package.json` 的 `scripts`，只把可执行名解析成真实入口（已知仅 `node`/`tsc`/`vite`），
且一律经**当前 node** 运行其 JS 入口——npm 的 `.bin` 垫片是无扩展名的 POSIX 脚本，Windows 上不可执行。不认识的可执行与未实现的 shell 语法一律具名拒绝，
glob 自行展开且**空匹配即失败**；无 `node`/依赖不可解析/某步非零分别以 `FRONTEND_TOOLCHAIN_MISSING`/`FRONTEND_DEPS_MISSING`/`FRONTEND_STEP_FAILED` 具名失败，**绝不静默跳过**。
实测：`check --profile full` **EXIT=0**，前端六步全过（`vite v7.1.9` 30 modules、desktop `1..5`、`test:ui` `1..15`），Rust 侧 82 个测试目标、**695 passed / 0 failed**（较 W40 +1）。
三条非空洞自检（注入类型错误 / 未知可执行 / 未实现 shell 语法）均按预期失败并随后还原。
见 [报告](reports/2026-10-11-w41-frontend-gate-profile.md)。

本轮 `w42-ci-complete-profile` 把上述门控**接进流水线**（W41 有意留下的「另行决定」）：CI 改前只有 `portable`（Linux）/
`windows-local`（Windows）两条 profile 分支，加两平台各一次的根 `pnpm typecheck`/`build`，**不含** `test:ui`、`frontend-gate`
或 `profile full`——桌面包单测（5 项）与 `tests/ui`（15 项）因此**在 CI 中从不运行**。本轮两个 job **统一 `check --profile full`**
并删除被其逐包覆盖的两步根命令（保留等于让同一检查存在第二处定义）；`windows-local` 在工具中保留、CI 不再用它
（在 Windows runner 上 `full` 是它的严格超集）。规格新增两条需求：CI SHALL 执行完整工程 profile；CI MUST NOT 在 profile
之外另定义前端命令。验证：Node **解析 YAML** 的结构断言 **13/13**；本机 `check --profile full` **EXIT=0**（82 目标 / 695 passed，
与 W41 一致、零源码改动）；`validate --all --strict` **37 项**。**观测（原写「未经观测」，同日复核订正）**：仓库是 public，Actions 结果可**匿名**读（`api.github.com`），
且 `on: push` 使每次 push 自动触发——本次 run `38099556545` **两 job 全绿**（含第 10 步 `check --profile full`，ubuntu 与 windows），
故流水线**已实证**而非「未经观测」。但同次核对发现 ubuntu 侧 `Portable foundation` 步**长期间歇失败**
（近 30 次 run 中 13 次为该单点失败、同 run 的 windows 侧均通过，且同一代码在不同提交上时绿时红 ⇒ 非确定性），
定位需 admin 权限下的 job 日志。见 [报告](reports/2026-10-11-w42-ci-complete-profile.md) 的**勘误**段。

本轮 `w43-ci-failure-annotation` 补上「注释位可读、内容是空的」那一段：annotations API 匿名可读却只有
`Process completed with exit code 1.`，而日志端点需 admin（403）⇒ 间歇失败只能定位到步级。门控步改
`shell: bash` + **`set -o pipefail`** + `tee`，其后加 `if: failure()` + `continue-on-error: true` 的诊断步跑
`scripts/ci-diagnose.mjs`（信号行优先 + 尾部兜底，逐行转成 `::error::`；日志缺失/为空不输出注释）。
**`set -o pipefail` 是必需的而非风格问题**——`cmd | tee f` 的退出码恒 0，实测无它时失败路径 `EXIT=0`，
CI 会永远绿且看上去一切正常。端到端**两个方向**都实测：正常路径 run `38108824691` 两平台
`Complete[success]`+`Publish[skipped]`；失败路径（临时分支换成 `check --profile bogus`）run `38108847930`
两平台 `Complete[failure]`+`Publish[success]`，**匿名**读到 xtask 的具名拒绝信息（Windows 上另抓到 cargo 的
`error: process didn't exit successfully …`）⇒ 通道成立且两平台都成立；两平台门控步都 failure 反证
`pipefail` 在 Windows 的 Git Bash 下同样生效。probe 分支已删除；代价是仓库永久留下一个失败 run 记录。
**本轮只解决「查不到原因」，未解决那个间歇失败本身**。见 [报告](reports/2026-10-11-w43-ci-failure-annotation.md)。

W44 让这条通道**第一次真实使用**：W43 的勘误提交 `81ce88b` 触发 ubuntu 失败（run `38110014189`，第 11 步诊断步成功），
**匿名**读到 check-run `114383450000` 的 12 条注解，**首次拿到测试级结论**——`test halt_during_media_drill_prewarm_never_commits_a_frame ... FAILED`。
但 11 条里只有 1 条是信号（`thiserror` 编译行因**裸 `error` 子串**命中、一个**通过**的测试因名字含 `unimplemented` 命中并把上下文拖入、
信号稀少时被无关行填满配额），故 W44 修的是**摘要质量**：判据由「含不含某个词」改为**「有没有报告失败」**
（新增噪音类 `test <name> ... ok|ignored`、`error` 收紧为 `\berror(\[|:)`、选行以信号为中心且**绝不**填充、新增目标头前查），
并新增**截断自证**——本次采集到的日志**只有 11 行**而 job 跑了 **2 分 50 秒**、且无 `test result:` 汇总行，
现在这种形态会输出 `::warning::` 并给出总行数（**截断成因未定，如实登记**）。
同时**撤回一条历轮沿用的旁证**：失败的测试带 `#[cfg(target_os = "linux")]`（engine 6 个测试里 4 个如此，Windows 只跑 2 个），
故「本机 Windows 从未复现」是**结构性必然**而非非确定性证据（报告正文的判定依据是「同一份代码时红时绿」，不受影响）。见
[报告](reports/2026-10-11-w44-ci-diagnose-signal-to-noise.md)。

W48 **修掉**了 W44 定位到断言行的那条 ubuntu 间歇失败（`halt_during_media_drill_prewarm_never_commits_a_frame`）。
定性先说清：**engine 的拆除路径本来就是完整的**（`DrillGuard::tick` 在每个轮询点检查 graceful halt 与 epoch 撤销），
所以问题**不在**「engine 不会中止」，而在**测试没能保证 halt 真的先到**——`Engine::start` 返回后测试才写 halt 文件，
而 supervision 线程此时可能已把 drill 跑完并提交；断言「halt 先于提交 ⇒ 不得记录提交」里的那个**先后顺序是靠墙钟赌出来的**。
W48 **不放宽断言**，而是让前提成立：新增 Linux 专用夹具 `park_asset_on_fifo`（把首个 avatar 素材换成**无写端命名管道**，
render-worker 在 prewarm 按序读素材、`File::open` 即阻塞，于是它**不可能**走到提交），并在写 halt 前
`wait_for_worker_pid`（复用既有 `/proc` 扫描，`wait_for_keeper_pid` 抽成通用 `wait_for_supervised_pid`）确认
worker **确已进入 prewarm**——否则 halt 可能落在更早的 keeper 握手里，drill 根本到不了 prewarm，测试会**假通过**。
夹具同形（不带管道）在 `media_drill_produces_a_committed_frame_through_the_real_worker` 里确实提交过帧，
故它移除的是**时序赌博**，不是提交能力。**如实披露**：被修测试与夹具都是 `#[cfg(target_os = "linux")]`，本机（Windows）
**不编译**它们，故本机门控只能证明「未误伤可移植目标」，**成立与否以 CI 的 ubuntu job 结论为准**。见
[报告](reports/2026-10-11-w48-deterministic-prewarm-halt-drill.md)。

W49 收尾了 **O01 与 O05 中「外部依赖最少」的那一半**，并**不改两者的登记状态**（O02 的 W38 先例：逻辑/界面出现
不等于验收达标）。**O01** 的缺项原是「演示包未建」，此前全库没有任何落点（`preset`/`演示包` 两词在
`crates/`、`apps/`、`schemas/` 里一次都没出现过）；W49 把它落成**判定机制**——
`crates/nai-kernel/src/demo.rs`（与 O03 的 `monitoring.rs`、O04 的 `device.rs` 同层同形）把 §615 演示包五要素
（**授权的**演示形象/示例知识/示例场景/Provider 预设/一键连通测试与成本预估）与 §612 两条时间目标落成
`audit_demo_package`（**十六条**稳定标记）+ `DemoReadiness` 三态，要点是：**未声明即条件化**（尚未交付不是违规，
是 `Blocked`；声明了任一部分则其余成为欠项）、**「授权的」是要求而非形容词**（有形象而无授权引用是缺陷）、
**连通不等于达标**（常量桩与把冒烟当验收各报一条）、**未知成本须说明原因**（未知 ≠ 0）、
**时间戳早于安装就绪是不自洽**而非「跑得快」。`tests/demo.rs` 19 项，含「一组构造描述的审计并集**恰为**这十六条」
的并集断言；零新增依赖、零 xtask 命令与证据目录（§30.1 未为演示包指定目录）。**O05** 的缺项原是「成本显示未建」，
W49 补上呈现层 `apps/desktop/src/upgrade.ts`（零 DOM，与 `console.ts` 同形）：未知分项呈现为**原因**（绝不呈现为 0）、
部分和呈现为**下界+未知计数**并明说不是总额、载荷严格解析（十条具名拒绝）；`tests/ui` 34 项全通过。
**两者都保持 `pending`**：O01 的授权形象内容、示例知识与场景正文、一键连通真实接线与本机人工演练仍为**外部**；
O05 的控制面尚未暴露 `nai-upgrade-core` 的 plan 与成本来源（与 §30.1 第 4 条「升级与回滚提示」**同一缺失前置**），
故呈现层**未接入任何界面**，真实渲染与生产接线未做。见
[报告](reports/2026-10-11-w49-o01-o05-in-repo-closeout.md)、[docs/demo-package.md](demo-package.md)、[ADR 0044](adr/0044-demo-package-readiness-boundary.md)。

下一主线：能在 portable 落地的机制项已全部完成（含 B02、O03、P01 的判定骨架与 O04 的设备侧、W36 的真实链路接入纪律、W37 的线上形状修正、
W38 的桌面审批面/UI 层、W39 的攻击者可控输入面随机测试、W40 的 B01 判据+执行体、W41 的完整工程 profile（前端门控）与 W42 的 CI 接线、W49 的 O01 判定机制与 O05 呈现层，S11 与 B01 的登记缺口均已补）；轨道二的自备资源**已有真实凭据落地**（文本／语音两条真实链路经本栈走通并产出真值证据，文本模型标准化为 `agnes-2.5-flash`）；剩余 **O01**（**W49 已交付判定机制**；仍需授权的演示形象内容、示例知识与场景正文、一键连通的真实接线，以及约 10 分钟首句演练与约 30 分钟真实输出验证的**人工执行**）、
**O02/O05**（UI 验收，按既有惯例 `pending`；**O05 的呈现层 W49 已落地但未接入界面**、O02 的「显示最终文本」受存储只存哈希所限）与阻塞项 **B01**（**判定机制、载荷载体、参考答案判据与评估执行体均已就位**；缺 100 题集内容与真实知识库内容，均为外部投入）、**B02/D12**（模型与目标机证据）、
**O03**（M0b 定案与真机 S09）、**O04**（干净 Windows 安装、真实音频设备变更与真实回退；安装侧已由 `nai-upgrade-core` 覆盖）、
**P01**（官方 SDK 与生产宿主；判定机制已由 W35 交付，D13）均需外部条件。
（**W46 已补一处登记缺口**：基线把开放平台与技能市场的 M01–M18 列为「唯一验收编号体系」的一部分（§37.5/§38.10，且 §1.4 明写
R1 含 `M01–M07、M13–M16、M18`、R2 含 `M08–M12、M17`），而注册表一条未列、id 家族白名单 `^(S|R|A|B|O|P|X)[0-9]{2}$` 也不含 `M`。
现已登记 18 条 `pending`（在册未开始）并扩白名单、加家族守卫；**不改任何既有 status**。见
[报告](reports/2026-10-11-w46-register-m-series.md)。）
仓库内仍可控的剩余缺口：**S08 的真实签名链**（**W47 已消除「SBOM」这个无定义空位**——立 `supply-chain-sbom`
规格并落地 `cargo xtask sbom`：CycloneDX 1.5、零新增依赖、离线、逐字节确定，产物自带范围声明；
但基线同样要求的**依赖漏洞与许可政策审计**（需 advisory 数据源与允许/拒绝清单）仍缺，故 **S08 保持 `implemented` 不升级**。
见 [报告](reports/2026-10-11-w47-sbom-supply-chain.md) 与 [ADR 0043](adr/0043-sbom-generation-and-scope.md)）；
**ubuntu 侧 CI 的间歇性失败**（W42 登记事实、W43 补上观测通道；**W44 由该通道首次真实捕获**，失败已定位到**测试级**——
`halt_during_media_drill_prewarm_never_commits_a_frame`，且该测试为 Linux-only，故「本机 Windows 从未复现」属**结构性必然**；
但**根因类别已定**：推送后该 run 在 ubuntu 上再次失败，新脚本产出 **11 条注解全部是信号**且**无截断警示**，
并给出精确到行的根因——`panicked at apps/engine/tests/engine.rs:446:5`，即**第二个**断言
（halt 后决策日志不得再出现 `media_drill_committed`），**不是** `halt_and_wait` 的 15 s 上限、**不是 hang**；
机制是 `Engine::start` 返回后测试才写 halt 文件，而 supervision 线程此时**可能已把 drill 跑完并提交了帧** ⇒
**halt 与 drill 的时序竞态**。**W48 已修**（**不放宽断言**：给被测 render-worker 一个确定性的 prewarm 阻塞点——
把首个 avatar 素材换成**无写端命名管道**使 worker 钉死在 prewarm，且**等到 worker 进程真的被派生**才发 halt；
断言与决策链校验一字不改。该测试与夹具均为 Linux-only，本机门控**不能**证明修法成立，**以 CI ubuntu 结论为准**）；
**截断成因仍未定**）。
同日 `064d754`（**只改文档**）的 run 为 **ubuntu `success` / windows `failure`**：同一份 Rust 代码在 ubuntu 上时红时绿
（`ca47168` 红、`064d754` 绿）⇒ 非确定性再添一例；**windows 侧失败是 10-10 以来首次**，落在 `crates/nai-runtime` 的 4 个测试上
（`StorageError { DeadlineExceeded, "operation stopped" }` / `{ PermissionDenied, "scope permission missing" }`）⇒
与 ubuntu 侧不同测试、不同根因，**但同属「并行 + 共享资源 + 慢主机」下的时序脆弱**。
**把前端纳入工程门控**已由 W41 关闭（`check --profile full` 覆盖两包 typecheck/build、桌面包单测与 `tests/ui`），
其**流水线接线**已由 W42 关闭（CI 两个 job 统一执行 `full`，此前 20 项前端测试在 CI 中从不运行）。

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
| D16 | （V3.5 遗留：Pro 资源与团队） | D07/D14 | 未开始 | 后续按 V3.7 R3 切片推进 |
| D17 | 升级器（V3.5 遗留 → V3.7 W15） | D03–D05/D14 | 已实现（R05/R09/R10；真实安装器/签名链未接，不标 tested） | [报告](reports/2026-10-10-w15-upgrade-core.md)、[ADR 0026](adr/0026-upgrade-core-boundary.md) |
| D18 | （V3.5 遗留：远程渲染与扩展宿主） | 对应契约/profile | 未开始 | 后续按 V3.7 R2/R3 推进 |
| D19 | （V3.5 遗留：安装与完整认证） | 前述目标能力 | 未开始 | 后续按 V3.7 R1–R3 与 registry 验收推进 |

D 表是 V3.5 追溯；开发按切片发布线（Slice-0/R1/R2/R3/实验，基线版本经 ADR 0042 升为 V3.7）与
`tests/registry.yaml` 验收推进，不再新增 D 编号。旧 T/E/U 仅追溯，不再新增。
任务顺序不缩减 Standard/Pro 功能目标。D12/D13 外部条件未满足时，继续独立任务；
对应真实认证保留 Blocked，不用演练结果替代。

## V3.7 切片工作项进展

| 工作项 | 目标 | 验收 | 状态 |
|---|---|---|---|
| W00 | 工程基线、xtask、契约管线、ADR 模板 | A04 | 已实现/实测（D01–D02 范围） |
| W01 | 契约 v1 冻结与三语言生成 | 生成无 diff | 已实现/实测（D02；provisional 状态待补） |
| W02 | 内核骨架与会话租约 | S03；值守租约 | 值守与控制撤权门控已实测；定时驱动/独立媒体接线待runtime |
| W03 | 存储与决策日志 | R04、R07 | 哈希链及原子有序冲刷/critical优先已实测；R07 桩 Provider 回放已在真实引擎链路实测（见 W10 行）；`PlaybackAck` 条目已进入哈希链（演练替身），真实设备播放 ACK 生产端仍待 |
| W04 | 宿主与 IPC | S11、A06 | 已实现/实测（D05）；wasm/remote 只留契约；W39 补齐 S11 的「模糊」一半（随机输入测试 8 项、实现零改动） |
| W05 | output-keeper | R01、R03、S12 | 看门狗演练进程已实现/实测：强杀源约10ms撤权、停止续租578ms到期、partial卡读250ms期限、keeper重启需更高epoch floor；帧槽位所有权与tmpfs-file传输见W06行；引擎broker授权keeper只读FD单帧消费与待机见W10行；正式窗口/音轨/媒体协调和P0设备独立通道未开始 | [报告](reports/2026-10-09-engine-broker-frame-delivery.md) |
| W06 | 2D render-worker | A01 | 2D软件像素生产核心与帧槽位所有权/tmpfs-file传输已实现/实测；引擎经broker声明spawn pid并继承写FD完成真实单帧见W10行；独立worker/keeper装配、protected handle broker与Windows帧率未完成 | [报告](reports/2026-10-09-w05-w06-frame-slots.md) |
| W07 | Provider 核心与适配器 | A02 前置 | 已实现/实测（D08）；新增 `TextGenerator` 接缝与 rehearsal-only 确定性桩（W10 A1），以及 `SpeechSynthesizer` 接缝与 rehearsal-only 语音桩（W10 A2）；真实付费生成与真实 TTS Blocked |
| W08 | 默认智能体、守门与手工平台 | S01–S04 | 已实现/实测（D10/D13） |
| W09 | 最小桌面控制台 | S02、R01、O02 | loopback 控制与审批 API 子切片已实现/实测；W38 补上桌面审批面板与 UI 层 `tests/ui`（15 项）；真实候选来源、证据详情、完整预检与生产 root 接线未开始；O02 保持 pending |
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
