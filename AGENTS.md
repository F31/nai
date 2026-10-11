# nai development instructions

- 项目英文名为 `nai`，仓库为 https://github.com/F31/nai。命名映射见
  `docs/adr/0001-project-name-and-design-baseline.md`。
- `docs/design.md` 指向唯一实现基线
  `docs/直播数字人 + 智能体系统技术方案 V3.6.md`（ADR 0015 升基线并勘误）。历史版本（V3.5 等）只供参考；
  不自行拼接旧条款覆盖 V3.6。规范变更记录 ADR。V3.6 的 dh-* 命名不采纳，工程标识一律 nai-*。
- 验收唯一来源为 `tests/registry.yaml`（V3.6 S/A/R/O/X + release_line，D→新编号映射自建）。
  后续任务卡以 registry 验收为准；旧 T/E/U 编号仅追溯，不再新增。
- 保持统一稳定内核；Lite/Standard/Pro 为能力预设，
  Local/Hybrid/Distributed 为部署配置，不创建三套核心实现。
- LLM/TTS 使用 API。Python 仅用于明确离线模型制作/导出；Windows
  正式运行包不得依赖 Python 推理，不以本地 LLM/TTS 替代 API 故障处理。
- 先修改 schemas 并生成契约，再修改消费者。跨语言消息用 Protobuf，
  配置/清单用 JSON Schema，HTTP 用 OpenAPI；生成代码不得手工修改。
- kernel 不依赖具体插件、Tauri 或平台 SDK；插件实现之间禁止直接依赖。
  composition root 负责装配，共享算法库按明确职责复用。
- 所有队列有界，调用有 deadline/cancellation；播放复检 session、epoch、
  审批对象哈希、事实版本和租约。未知平台能力不可视为已授权。
  守门缺失拒绝外发，模板与缓存也须门控。
- 控制与媒体分离；PCM/帧不经 JSON/Base64 控制总线。P0 静音不等待普通
  网络、数据库或模型锁；媒体独立执行失联租约静音。
- 进程内模块没有原生故障隔离。SDK/GPU/第三方代码按声明宿主隔离；
  签名不等于安全，路径、凭据、IPC 与宿主权限必须实际执行。
- 程序/模型升级仅在维护窗口激活；迁移须有一致备份、journal、兼容范围
  和崩溃恢复。回滚不得静默丢弃提交后的新事实和审计。
- mock 仅用于 tests 或明确演练。SDK/模型/授权/凭据缺失时保留显式
  Unavailable/Blocked；不可用假数据给真实集成或认证标 Passed。
- Local 不要求云主机、Redis/NATS、独立向量服务或 Kubernetes。
  新增常驻依赖须记录需求、资源/维护成本及适用部署 profile。
- 不提交密钥、许可不明的权重/素材、生产观众数据或大型生成产物。
- 每轮选择完整可验证任务，运行匹配范围的检查，并更新
  `docs/development-plan.md` 与 `docs/reports/`。状态区分实现、实测、Blocked
  和 NotApplicable；未实现的功能保持未开始，不能冒充外部阻塞。
- 状态机、迁移、IPC、媒体取消须有意义测试；发布组合执行完整 T/E/U
  门槛。Linux/WSL 逻辑检查不替代 Windows 安装、设备、SDK 或 GPU 验收。

- 使用 OpenSpec SDD：每轮先建立 proposal/specs/design/tasks，校验后实施，
  验证后勾选并归档。`openspec/config.yaml` 为项目上下文，`openspec/specs/`
  为已归档增量规格，不隐式覆盖 V3.5；基线/架构变更记录 ADR。
- 使用项目锁定 CLI：`pnpm exec openspec`。流程与已实现命令见
  `docs/engineering.md`。OpenSpec 格式校验不替代实际编译、测试或认证。

D01–D11 已建立工程、生成契约、存储/密钥、内核绑定、宿主/IPC、会话/媒体租约、有界调度/共享预算、API Provider 传输、
知识完整链路、智能体/守门评估和媒体样本时轴：cargo xtask check-deps、
check --profile portable/windows-local/full、schema [--check]、test --suite contracts/storage/kernel/hosts/session/scheduler/provider/knowledge/upgrade
及前端 typecheck/build 可执行。契约工具要求 protoc 3.21.12，完整 C++ suite 另需同版
libprotobuf；详见 docs/contracts/README.md、docs/hosts.md、docs/session.md、docs/scheduler.md、docs/provider.md、docs/knowledge.md、docs/upgrade.md、docs/deployment.md 与 docs/kernel.md。
真实付费 Provider 生成受账户余额约束（冒烟显示无余额，Blocked）；真实音轨/媒体进程接线待设备里程碑。其他 suite/认证/打包命令仍待实现，
执行前核实对应 workspace 与 scripts；不可添加空成功占位。

D11 媒体样本时轴已实测并归档：audio/clock/ring/resample/record/ack 与双水位 commit；
全 workspace 199 项 Rust 测试通过。openspec/changes/d11-media-basics 已归档为
2026-10-08-d11-media-basics，主规格 media-basics-timeline 生效。
Windows 独立窗口/音轨、虚拟摄像头/音频设备与真实播放仍 Blocked（WSL 不验收），设备侧认证未执行。
详见 docs/media.md、docs/reports/2026-10-08-d11-media-basics.md。

D12 前置模型包完整性检查已实现/实测：nai-plugin-sdk::model 检查声明闭包、内容 SHA256、精确运行时与预算/取消。
`cargo test -p nai-plugin-sdk --offline` 可执行；全 workspace 205 项 Rust 测试通过。
本轮已归档为 2026-10-08-d12-model-admission，主规格 model-package-integrity 生效。
D12 avatar-ort/离线导出与真实推理尚未实现；完整合法模型/Windows GPU 证据缺失，G0–G3 未执行（Blocked）。
详见 docs/models.md 与 docs/reports/2026-10-08-d12-model-admission.md；内容检查成功不授予 Ready 或认证。

D13 平台事件入口已实现/实测：nai-kernel::platform 连接准入、官方事件权限/epoch/代次复检、断线清权与重连。
PlatformEvent 新增 optional generation_epoch 与 connection_generation，已生成 Rust/TS/C++；十类消息互通与全 workspace 213 项 Rust 测试通过。
调用方在入队和消费前都须复检；可信 root 独立核验身份/真实房间/权限，连接成功不恢复自动播报。
官方 SDK、生产平台宿主接线及网关服务尚未实现；真实平台/Windows 伴侣权限证据缺失，认证 Blocked。
详见 docs/platform.md 与 docs/reports/2026-10-08-d13-platform-ingress.md。

D14 首轮本机控制已实现/实测：nai-control-api 显式 loopback HTTP 状态/session/mute/takeover，桌面 ControlClient 接线。
`cargo test -p nai-control-api --offline`、`pnpm --filter @nai/desktop test`、`pnpm test:control-smoke` 可执行；全 workspace 222 项 Rust 测试通过。
OpenAPI SessionCommand 必需 device_id/seq；校验 schema 与 TS 从源生成。只有五个工程操作标记已实现；其他接口无成功占位。
先 EmergencyHandle 撤权再 try_lock/CAS；控制 ACK 不代表硬件静音/播放完成。
Local 正式 protected IPC 默认不变；完整预检/审批/配置、Tauri/IPC、真实设备接线尚未实现，Windows/生产认证未执行。
详见 docs/control.md、ADR 0014 与 docs/reports/2026-10-08-d14-local-control.md。

D15 验收注册表与基线切换已实现/实测：ADR 0015 把 V3.6 升为唯一实现基线（勘误清单生效，dh-* 命名不采纳），
tests/registry.yaml 为验收唯一来源（S/A/R/O/X + release_line，D01–D14 映射自建并回填）。
`cargo xtask check --registry` 与 `cargo xtask check --profile portable` 可执行；17 个主规格、15 项归档校验通过。
决策日志 decision_log（R07 前置）与值守租约（600ms 短期限）为下一轮；播放 ACK 段保持 Draft，不冒充完整 R07。
详见 docs/adr/0015-v36-baseline-and-admission-registry.md、docs/reports/2026-10-08-d15-admission-registry.md 与 tests/registry.yaml。

W02/W03 决策日志与值守租约已实现/实测（按 V3.6 §31 工作项推进，不再新增 D 编号）：
nai-storage 新增 0005 迁移、`decision_log` 只追加哈希链、critical/telemetry 分级写入与有界缓冲、`verify_decision_chain`；
nai-kernel 新增值守租约（显式确认、心跳不续期、到期 600ms 内撤权、重连不恢复）。
备份契约随迁移升至 schema 5 并重新生成 TS。R07 由 pending 更新为 implemented，S01 补值守证据。
`cargo test --workspace --offline`、`cargo xtask check --registry`/`--profile portable`、`schema --check` 可执行。
桩 Provider 回放与播放 ACK 生产端待 W17，进程级强杀故障演练（R03 缺口）与 output-keeper/render-worker（W05/W06）未开始。
详见 docs/reports/2026-10-08-w02-w03-decision-log-duty-lease.md、docs/storage.md、docs/session.md 与 docs/development-plan.md。

W02/W03 接管完整性修复已实测：遥测缓冲+新条目同事务按序提交、critical不先冲刷遥测；Mute/Takeover/Stop清值守并退出自动，耗尽路径拒绝恢复自动。
全 workspace 238 项Rust测试、portable与生成无漂移通过；R07仍implemented，root定时驱动/播放ACK/真实设备/进程强杀仍待接线。
详见 docs/reports/2026-10-08-w02-w03-admission-hardening.md；下一主线 W05/W06/W10，不新增D编号。

W05/W10 输出看门狗演练已实测并归档：apps/output-keeper 仅 --watchdog-drill 显式启动，私有继承stdio，独立10ms租约轮询、
16×64KiB有界输入、250ms帧期限、仅Rehearsal租约、设备恒Unavailable；真实子进程强杀约10ms撤权、停止续租578ms到期、
partial卡读按期关闭、keeper重启需更高epoch floor。MediaGate::with_epoch_floor 支持无授权floor恢复。
12类消息三语言互通、全 workspace 250 项 Rust测试与生成无漂移通过。R03由缺口补为可移植证据，仍不标tested。
正式窗口/音轨/共享内存/命名管道身份/真实设备故障矩阵仍在W05/W06/W10后续，Windows认证未执行。
本轮已归档为 2026-10-09-w05-w10-output-watchdog（20个主规格/18项归档）。详见
docs/reports/2026-10-08-w05-w10-output-watchdog.md、docs/adr/0016-output-watchdog-drill-boundary.md。

W09 审批 API 子切片已实现/实测：storage list/read/reject 与 approve 原子 critical 日志；每会话/epoch 最多200待审批候选，control-api 接线 list/approve/reject、生成 ApprovalCommand 校验及共享256条幂等账本。
ApprovalBackend::new 接收 root 的 Arc<Mutex<Store>>，不得重开 Store 绕过恢复撤权；每请求新建 Operation 并 try_lock，关键日志失败事务回滚。
全 workspace 262 项 Rust测试、portable、contracts十二类消息三语言互通、schema无漂移与前端检查通过；S02保持implemented，O02保持pending。
已归档为2026-10-09-w09-approval-surface（21个主规格/19项归档校验通过）。
真实候选来源、生产root/媒体接线、审批GUI/高危二次确认/手机审核未实现，Windows认证未执行。
详见 docs/control.md、docs/reports/2026-10-09-w09-approval-surface.md 与 docs/development-plan.md。

W06 可移植2D核心已实现/实测：plugins/avatar-2d（nai-avatar-2d，plugin层）校验三层RGBA8素材闭包/SHA256/root许可，按PCM样本窗口RMS/噪声门限/攻击释放平滑生成实际RGBA帧。
`cargo test -p nai-avatar-2d --offline` 9项测试、全workspace271项Rust测试、14包依赖边界/portable、contracts十二类消息互通、schema无漂移与前端检查通过。
已归档为2026-10-09-w06-avatar-2d-core，主规格avatar-2d-rendering生效；22个主规格/20项归档校验通过。
输入/渲染均复检epoch/许可/取消/期限；返回owned像素与FrameMetadata不授予Ready/播放ACK。A01保持implemented。
独立render-worker/共享内存/keeper帧消费与生产root尚未实现；Windows窗口/设备/实际帧率未验收，不能用fixture素材替代合法生产形象包。
详见 docs/avatar-2d.md、docs/reports/2026-10-09-w06-avatar-2d-core.md 与 docs/development-plan.md。

W05/W06 帧槽位所有权与Linux tmpfs-file演练已实现/实测：media-core frame_slots单owner状态机/代次隔离/SHA256/超时断连退休整池；
provisional FrameSlotTicket/Commit/Event三语言互通（提交分支boxed）；显式nai-shared-frame-drill经继承FD真实子进程写入/读回，部分写入强杀不产生Ready、新池绕过旧FD。
全workspace283项测试、portable、contracts十三类消息、schema无漂移、Windows GNU交叉检查与前端检查通过；A01/R03保持implemented。
tmpfs-file是有复制工程后端，非mmap/零拷贝/正式共享内存；独立render-worker/keeper装配、protected handle broker、命名管道身份与Windows映射未实现。
详见 docs/media.md、docs/adr/0017-frame-slot-drill-boundary.md、docs/reports/2026-10-09-w05-w06-frame-slots.md 与 docs/development-plan.md。

W05/W06 帧句柄授权 broker 与 keeper 帧消费/待机已实现/实测：media-core `frame_slots::broker` 门控 producer/consumer 授权
（依据内核 `/proc` 证据：声明 pid 存在、属 owner uid、持有 pool 文件；UDS 仅作活性触发），未授权/错误 pid/超预算均无授权；
`frame_slots::consumer` 给出七类待机原因且不产生 PlaybackAck；keeper 新增 `FramePath`、`--frame-drill`（池 FD 经 stderr 继承、只读重开）与
`OutputWatchdogStatus.presenting/standby_reason`，租约丢失/peer 报错/池退休切待机且 `output_available` 恒 false。
**连接对端未被认证**：`SO_PEERCRED`/`peer_cred` 在钉住的 rustc 1.96.0 上仍不稳定（E0658，实测 E0658），本 workspace 禁 `unsafe`、
未引入新依赖；句柄交付仍为 spawn 继承，SCM_RIGHTS 与 Windows 命名管道未实现（Blocked，ADR 0018）。
全workspace303项测试、portable、contracts十三类消息互通、schema无漂移与前端检查通过；A01/R03保持implemented（不标tested）。
详见 docs/media.md、docs/adr/0018-frame-handle-broker-boundary.md 与 docs/reports/2026-10-09-w05-w06-frame-handle-broker.md。

W05/W06 装配根与 render-worker 演练已实现/实测：crates/nai-runtime（layer runtime）装配唯一存储 owner、
显式 bootstrap 可信 owner 管理、运行时配置校验、Session/MediaGate/Registry/ApprovalBackend；
control 与 Supervisor::observe 固定先 P0 静音→撤权（epoch 递增）→critical 决策日志，日志失败不回退静音；
值守授予/到期/撤权写 critical duty_lease。apps/render-worker（layer media）显式 --render-drill：
池 FD 经 stderr 继承，PCM 原始字节与 protobuf ticket 经文件传入，nai-avatar-2d 真实像素写槽，
stdout 仅 FrameSlotEvent；--ready 预热解决冷启动超 250ms 槽位预算；不产生 PlaybackAck/Ready。
全workspace318项测试、portable、contracts十三类消息互通、schema无漂移与前端检查通过；
R01/R03/A01/S01 保持 implemented（不标 tested）。
apps/engine 可执行、loopback HTTP 服务启动、broker 装配接线、真实设备与 PlaybackAck 生产端未实现。
详见 docs/runtime.md、docs/adr/0019-composition-root-boundary.md 与 docs/reports/2026-10-09-w05-w06-composition-root.md。

引擎装配已实现/实测：新增 apps/engine（layer app）以显式参数启动真实装配——Composition::prepare、
ControlApi::with_shared_session（控制面与装配根共享同一 Session，撤权 epoch 立即对监督/租约/日志
可见）、loopback serve 与 stdout 控制地址行；`--provision <profile.json>` 为可信 owner 一次性配置
（建工作区/授权/写运行时 profile 后退出）；监督线程 spawn keeper 后每 tick 先续租再 observe，控制面
撤权后停止续租（renewal_halted_after_revoke）且不自动重签，续租失败不静默退出；`--media-drill`
（Linux）建 /dev/shm 私有池 + 预热 render-worker + reserve/原子 ticket + owner commit + consumer
读回 owned 像素并记 telemetry；`--halt-file` 触发有界停机（撤 token→静音+撤权+critical 日志→回收
子进程→停 HTTP→退出 0）。nai-storage 新增只读 decision_entries。
全workspace323项测试、portable、contracts十三类消息互通、schema无漂移、fmt/clippy、前端 typecheck、
pnpm test:engine-smoke 与既有 test:control-smoke 通过；S09/R01/A01/O02 保持 implemented（不标 tested）。
未接线：protected IPC/信号停机（signal-hook-registry 不在离线缓存、禁 unsafe，故用 halt 文件）、
broker UDS 授权与 keeper 帧投递、PlaybackAck 生产端、真实设备、Windows 与性能认证。
详见 docs/engine.md、docs/adr/0020-engine-assembly-boundary.md 与 docs/reports/2026-10-09-engine-assembly.md。

引擎 broker 帧投递接线已实现/实测（ADR 0021，勘误上轮）：apps/engine 的 `--media-drill` 改为
FrameBroker 门控的真实 render-worker→keeper 单帧——root 声明 spawn pid，broker 按 /proc 证据
（pid 存活、属 pool owner、持有 pool 文件）授权；keeper 经 stderr 继承**只读**池 FD 并触发 UDS，
render-worker 继承写 FD、`--ready` 预热、reserve/原子 ticket/commit，root accept 后 pin Reading、
`begin_consume`、relay 给 keeper，匹配 `consumed_buffer_ref`/`consumed_sha256` 诊断后 release；
预授权/等待期 DrillGuard 每 5ms 续租/observe/复检 epoch 与 halt，worker 事件经有界队列非阻塞轮询，
卡读不阻塞期限；失败先 P0 静音→撤权→critical 日志→有界回收，Supervisor 析构兜底。
OutputWatchdogStatus 新增 consumed_buffer_ref/consumed_sha256（诊断，非 PlaybackAck/Ready，
output_available 恒 false）。broker `require()` 只复检 authorized&&!retired（授权在 grant 时一次性
建立、disconnect 撤销），保留「producer 写完即退出、root 事后提交」语义。
全workspace324项测试、check-deps(17包)、portable、registry、contracts十三类消息、schema无漂移、
fmt/clippy、前端 typecheck、engine-smoke/control-smoke 通过；S09/R01/R03/A01 保持 implemented、
O02 pending，均不标 tested。勘误：engine-assembly 归档 tasks 3.2/3.3/3.4 与 proposal 更正为实际
的停止续租、引擎内消费（本轮补齐 broker/keeper 接线）、halt 文件停机。
仍 Blocked/未接线：连接对端未认证（SO_PEERCRED E0658，ADR 0018 不升级）、SCM_RIGHTS/Windows 匿名管道、
protected IPC/信号停机（signal-hook-registry 无离线缓存、禁 unsafe）、PlaybackAck 生产端、真实设备/
窗口帧率与 Windows 认证。详见 docs/adr/0021-engine-broker-frame-delivery.md、
docs/reports/2026-10-09-engine-broker-frame-delivery.md 与 docs/engine.md。

W10 A1 手工事件链路已实现/实测（ADR 0022）：apps/engine 新增 pipeline 模块
（归一化/去重 → 知识检索 → 智能体决策 → 守门四阶段 → 文本生成 → 决策日志哈希链），
编排留在 app 层（不新建核心 crate、不改 kernel；check-deps 17 包方向不变，无 kernel→runtime、无 plugin→plugin）；
nai-provider-core::text 新增 TextGenerator 接缝与 rehearsal-only 确定性桩（rehearsal-deterministic-v1，
离线/可复现/无网络），经 CallBudget 走 reserve→dispatch→finish，不绕过预算/取消/usage，已派发请求不重发。
以真实 engine 进程集成测试实测：手工事件经守门写入可校验决策日志、同事件同桩重放逐项一致（replay_matched）、
哈希链校验通过（chain_verified）；守门缺失/守门不通过/陈旧 epoch/无知识命中均不外发；
--inject 注入知识/守门/生成/存储失败时先 P0 静音→撤权（epoch 递增）→critical 日志→有界退出，存储失败不回退静音。
summary 显式标注 rehearsal:true / real_provider:false / devices:false / playback_ack:false / output_available:false，
链路止于文本（未渲染/未投递/未播放）。
workspace 测试、check-deps(17包)、portable(fmt/clippy)、registry 通过；contracts/schema 生成检查在本机因 node
无法 spawn 子进程（EBUSY）未执行（本轮未改任何 schemas，catalog 通过），如实记录。
R07 取得桩回放判据、S01 补真实进程链路证据，均保持 implemented 不自动升级；真实 Provider/播放 ACK/设备/Windows 认证未做。
已归档为 2026-10-10-engine-manual-event-pipeline（24个主规格/26项归档校验通过）。详见 docs/adr/0022-engine-manual-event-pipeline-boundary.md、
docs/reports/2026-10-10-engine-manual-event-pipeline.md、docs/engine.md 与 docs/provider.md。

W10 A2 文本→语音→2D→播放完成已实现/实测（ADR 0023）：链路终点从「文本」延到
「文本 → 语音合成 → 有界 AudioBlock 流 → 真实 RGBA 帧 → 双水位提交 → 实际播放完成 ACK」。
三段职责各归其层，编排仍在 apps/engine：nai-provider-core::speech 新增 SpeechSynthesizer 接缝与
rehearsal-only 确定性语音桩（rehearsal-speech-deterministic-v1，样本总数只由文本与采样率决定，
因此「预期样本总数」在合成前即可独立算出，「完整播放」才可被核验而非回读自证）；
nai-media-core::playback 新增播放通道，把「提交」与「播放完成」在类型与日志上分开，帧总数用与软件渲染器
相同的整数算术，1 基帧序必须严格连续，未提交即声明进度、身份/样本数不符、已取消后再完成一律被拒。
apps/engine 新增可选 render 闭包段（manifest/config/恰三资产/许可/voice/output_id/playback）：
缺失时链路行为与上轮完全一致（delivery=none、playback_ack=false、无音频/无帧），
给出时按脚本目录解析相对路径并在驱动任何会话状态前完成 2D 素材准入（不代填默认资产/voice/输出身份）。
守门若改写过候选文本（候选哈希被清空）则事件停为 guard_superseded 且不发声。
只有「已提交 + actual_local_playback_completed + played_sample_count = 预期样本总数」才记
DecisionKind::PlaybackAck；partial/invalid/cancelled 三个正常结局（非故障注入）不记该 kind，
摘要如实标 playback_ack=false；摘要恒标 devices/output_available/real_tts/frame_slot/keeper_consume=false，
帧只在内存（delivery=memory），演练播放槽（rehearsal-sink）是软件替身而非设备，其 PlaybackAck 记录
不得被表述为设备/硬件认证。--inject speech 需要脚本给出 render 段（无法生效的注入被拒绝而非忽略），
失败仍走「P0 静音→撤权→critical 日志→有界退出」。新增测试：provider speech 5 项、media playback 8 项、
engine pipeline 集成 6 项（含「帧总数 == 由声明样本总数推出的帧数」的独立核验）。
workspace 测试 317 项通过（13 项既有 Windows 平台差异失败，均不在本轮改动文件）、check-deps(17包)、
registry、fmt、workspace clippy(-D warnings) 通过；**cargo xtask check --profile portable 在本机无法整体通过**
（① 该包装命令在 clippy 之后必须重链自己的 nai-xtask.exe，而 Windows 锁定运行中的可执行文件 → LNK1104；
② 其内层 cargo test --workspace --locked 含上述 13 项既有失败），故按等价四步逐条单独执行并如实记录；
contracts/schema 生成检查仍因 node 无法 spawn 子进程（EBUSY）未执行（本轮未改任何 schemas，catalog 通过）。
R07 补「PlaybackAck 条目进入哈希链」证据、A01 补「文本→播放完成」链路证据，均保持 implemented 不升级
（真实 TTS 与设备/硬件播放 ACK 仍缺）。详见 docs/adr/0023-engine-text-to-playback-ack-boundary.md、
docs/reports/2026-10-10-engine-text-to-playback-ack.md、docs/engine.md、docs/media.md 与 docs/provider.md。

工程校验回路的平台事实与真缺陷已收敛/实测：上一轮记的「13 项 Windows 平台差异」逐条追到三个互相独立的根因，其中一项是真缺陷。
（1）**真缺陷**：backup 与 restore 各自调用 `File::open(..).sync_all()`，而 Windows 的 `FlushFileBuffers` 要求可写句柄
（`PermissionDenied(os error 5)`；Linux 对只读 fd 的 fsync 合法）——**备份与恢复在 Windows 上从未成功过**。
抽出单一 `flush_durable`（可写句柄 + sync_all）供两处共用，不做平台分叉；storage `--lib` 由 4 项失败变为 48 passed/0 failed，
备份恢复用例单独重复 8/8 通过。上一轮把它记为「`fs::rename` 覆盖语义差异」，那是把压平后的错误文案当成了结论。
（2）**平台事实**：宿主对**任何** stdin 为管道的子进程都拒绝 spawn（`ERROR_PIPE_BUSY`；独立程序 spawn `cmd /c echo` 可复现；
`CreatePipe` 直建可用、piped stdout/stderr 可用），同一受限能力使 node `spawnSync` 返回 `EBUSY`。
按同包 `frame_drill.rs` 的既有约定，`apps/output-keeper/tests/faults.rs` 与 engine 的 `/proc`+`kill` 监督演练改用
文件级/用例级 `#![cfg(target_os = "linux")]` 声明范围，并保留 `host_stdin_pipe_refusal` 具名诊断（签名须为 `-> Child`，
`fn(E) -> !` 无法作为 `unwrap_or_else` 的 `fn(E) -> T`）。不做运行期探测后提前 return——libtest 无 skipped 结局，那会把「没跑」显示成 `ok`。
（3）**没验证却被写成通过**：`0600` 断言在 Windows 上因别的原因碰巧通过；拆为 `#[cfg(unix)]` 实测与
`#[cfg(not(unix))]` 特征化断言（把「Windows 尚无 ACL 检查」的缺口钉住，该检查仍未实现）。
（4）**harness 自锁**：检查逻辑移入 `xtask/src/lib.rs`，`main.rs` 收成 CLI 外壳，测试步改为
`cargo test --workspace --exclude <env!("CARGO_PKG_NAME")>` 加 `-p <同名> --lib`；包名取自包自身以免改名后静默空排除，
bin 目标仍被 clippy `--all-targets` 检查（回避的只是链接）。
（5）错误词表按类别分派（`io::ErrorKind` / SQLite `ErrorCode`），不搬 OS 本地化文本，使「平台差异」这类误判不再成立；
另修一处自相矛盾的时钟竞态（改用持久化边界推进，遵同文件「不用 flaky sleep」的既有约定）。
`cargo xtask check --profile portable` **在本机首次整体通过：EXIT=0，63 个测试目标，321 passed / 0 failed**（基线 272 passed / 13 failed）。
宿主提交限制（`os error 1455` / `LNK1102`）下需 `CARGO_BUILD_JOBS=2`；该开关属 cargo 原生能力，未写进 portable 检查。
contracts/schema 生成检查与前端 pnpm 检查仍因本机工具链限制未执行；registry 不改动任何验收项状态
（A04 等工程项证据补强，产品项不因测试变绿升级）。
详见 openspec/changes/engineering-loop-platform-truths/、docs/reports/2026-10-10-engineering-loop-platform-truths.md 与 docs/engineering.md。

W19 第二家 LLM 与 TTS Provider 与替换验证已实现/实测（ADR 0025）：把 registry A02「仅改配置换 Provider」从声明变为可执行判据。
nai-provider-core 传输接缝去硬编码：`client.rs` 新增 `AuthScheme`（`Bearer` 默认 | `Header{name}`）、`extra_headers`、`provider_ref`
（`Usage.provider_ref` 由 endpoint 改为稳定供应商身份）；`request()` 按 scheme 构造凭据头使共享路径对所有供应商相同，
校验为**拒绝而非事后清洗**（头名 RFC 9110 token、禁覆盖 authorization/host/content-length/content-type、禁重复、禁 CR/LF/NUL、有界）。
新增 `adapters/anthropic.rs`（`POST /messages`、`x-api-key`+`anthropic-version`、SSE 按 JSON `type` 分派、`input_tokens`/`output_tokens`
在适配器边界映射进封闭单位表）与 `adapters/elevenlabs.rs`（`POST /v1/text-to-speech/{voice}?output_format=pcm_{rate}`、`xi-api-key`、
裸二进制 PCM、字符成本取 `x-character-count`）；新增 `binding.rs` 由配置字符串选协议并派生 auth/静态头，`ProviderProfile` 不含凭据，
`LlmClient`/`SpeechClient` 为调用方持有的能力切面，能力不符显式 `InvalidArgument` 不降级。
`tests/substitution.rs` 两台本地 mock（无外网）：**同一驱动函数**覆盖两家协议，同时断言「调用方可见一致」与「线上确实不同」
（路径/凭据头名/请求体形状/voice 在体内还是路径内）；用量按供应商分别断言，不报 usage 的一律不编造；
语音按句独立（两次均从 `seq=0` 起、`sample_start` 恒 0、各自结算一条 usage）与不可达采样率/非单声道显式拒绝并释放预留。
provider-core 18 lib + 10 provider + 8 substitution = 36 项通过；零新增依赖（复用既有 `tiny_http`）。
`cargo xtask check --profile portable` 通过：EXIT=0，64 个测试目标，336 passed / 0 failed（基线 321）。
A02/B03 补证据但保持 implemented **不升级**（真实付费账号无余额 Blocked、真实 TTS 设备与 engine 真实 provider 接线未做）。
详见 docs/adr/0025-provider-substitution-boundary.md、docs/reports/2026-10-10-w19-provider-substitution.md 与 docs/provider.md。

W15 升级器基础已实现/实测（ADR 0026）：新增 shared 层 `crates/nai-upgrade-core`（零新增外部依赖），
把 V3.6 §14.3 的九步升级流程落成可执行编排，registry R05/R09/R10 由 pending 升 implemented（**不标 tested**：
故障注入为内存演练宿主而非真实安装器/断电），O05 按 O02 惯例只补证据保持 pending（其 `automation: ui` 而真实升级 UI 未建），
S08 补撤销列表证据保持 implemented。核心边界全部可注入——`SignatureVerifier` / `RevocationList` / `SpaceProbe` /
`ActiveSessionGate` / `SessionDrain` / `UpgradeHost` / `UpgradeJournal`，因此升级器零宿主依赖、portable 下可完整测试。
`verify.rs` 在**产生暂存之前**完成 schema→内容集合精确匹配（缺失/额外/重复拒绝）→逐文件 SHA256→签名→撤销；
`revocation.rs` 把撤销列表做成**带期限的可信缓存**（命中拒绝、未过期放行、**过期拒绝**，安装与启动双入口），
否决了「做成集合」；`preflight.rs` 六项检查（依赖闭包/平台+架构/运行时矩阵/双份磁盘 `install+migrated`/凭据）
三态化，**未知与不足同等拒绝**且错误码可区分（`CAPABILITY_NOT_READY` / `RESOURCE_EXHAUSTED`）；
`plan.rs` 九步事务 + **写前记录**的五状态 journal（`prepared/migrated/checked/committed/rolled_back`，
单记录原地 UPSERT，`backup_ref`/`error_code` 不被抹除）+ 崩溃恢复幂等（`checked` 比较清单身份消歧）+ 回滚**导出**
升级后事实与审计；`gates.rs` 门控与激活**拆两次调用**（被门控的包不失效）、降档**先排空后停用**且不触碰数据、
超 `rollback_range` 以 `VERSION_INCOMPATIBLE` 阻止静默回滚；`cost.rs` 成本分项 `Known | Unknown`，**未知不冒充 0**，
`total()` 部分和不当作总额。`nai-storage/src/upgrade.rs` 复用既有 `upgrade_journal` 表；`xtask` 新增 `test --suite upgrade`。
实测：upgrade-core 3 lib + 7 gates + 12 plan + 8 upgrade = 30 项通过；storage lib 50 项；`cargo xtask check-deps` 18 包；
clippy `--all-targets` 与 `cargo fmt --all --check` 干净；`cargo xtask check --profile portable` EXIT=0（见报告）。
未做边界（四处写明）：**无真实签名链**（`DeterministicVerifier` 为 sha2 演练替身）、无真实安装器/下载器、
`semver` 只做交集判断、未接入装配根。
详见 docs/adr/0026-upgrade-core-boundary.md、docs/reports/2026-10-10-w15-upgrade-core.md 与 docs/upgrade.md。

W25 部署生命周期已实现/实测（ADR 0027）：在 `crates/nai-upgrade-core` 内补齐 W15 的另一半——**装完占多少、闲置怎么放手、
离线与换部署形态怎么办**，registry A08/A09/R11 由 pending 升 implemented（**不标 tested**：A08/A09 为 `manual` 且需真实设备与长播，
R11 需真实轻云实例。不新开 crate，因为三项与 W15 共享同一条不变量 §14.4「未启用的能力不占用进程、端口、显存或云请求」）。
`residency.rs` 把「能力占哪些资源」建模成**声明式足迹**（封闭四族 `Process|Port|Vram|CloudRequest`）并**只在 `enable` 时物化**，
于是「未安装/未启用 ⇒ 零占用」是结构性质而非承诺；只有 `Process`/`Port` 排他（`Vram`/`CloudRequest` 允许两个能力各自持有同一模型/端点）。
空闲释放**按 TTL 分族**（worker 30s / 端口随进程 30s / 未用预览云请求 60s / 模型显存 300s，方向由重载代价决定），
`reclaim` 只释放到期的并把仍保留的计入 `held_back`——**否决**「场次一结束就全释放」（代价是下一句一次完整模型重载）；
回收不动启用状态、不删数据、繁忙能力绝不回收。审计做成**对公开快照值的纯函数** `audit_residency`（宿主可审计自己的视图，
且每条规则都能被手工构造的状态真实触发），六个稳定标记中 `idle resource past its ttl` 是可达的泄漏检测，其余是改错时的守卫。
`tier_plan` 把 Lite→Standard→Pro 做成**两段差集的并**，故 Lite→Pro 不需要中间档，「不重装内核」是数据的性质而非需要记住的规则。
`offline.rs` 的离线前置是「每个声明文件都在本地」，缺件以 `CAPABILITY_NOT_READY` 拒绝并由 `missing_artifacts` 指出是哪个文件，
随后复用**同一条**校验链；「不会偷偷下载」是**编译期**保证（crate 无 HTTP 客户端），**否决** `allow_fetch` 开关。
`migration.rs` 在搬动前拒绝三件事（有活动会话 / 分布式目标 + SQLite 多写者 / 权限无映射即拒，不静默丢弃），
搬动后复验三项且错误码可区分（权限集合 `INVALID_ARGUMENT`、证据版本 `VERSION_INCOMPATIBLE`、数据哈希 `INVALID_ARGUMENT`）——
**否决**「只比哈希」（哈希一致证明不了权限被正确映射）；`OutputSwitch` 以调用方给的 key 幂等，即「输出切换不重复播报」。
实测：upgrade-core 3 lib + 17 lifecycle + 11 migration + 12 plan + 8 upgrade = 51 项通过；`cargo xtask check-deps` 18 包；
clippy `--all-targets` 与 `cargo fmt --all --check` 干净；`cargo xtask check --profile portable` EXIT=0（见报告）。
未做边界：`Vram` 是声明式索赔而非真实配额；R06 长播、B04 延迟分位是独立主线；A10 云断连与 A11 多会话由后续变更完成（见 W26/W27）。
详见 docs/adr/0027-deployment-lifecycle-boundary.md、docs/reports/2026-10-10-w25-tier-idle-migration.md 与 docs/deployment.md。

W26 云断连与低云容量已实现/实测（ADR 0028）：把 A10 的两条**否定式**判据（V3.5 E07/E08）落成可执行值，registry A10 由 pending 升 implemented
（**不标 tested**：`automation: fault` 的真实断网/弱网注入与真实云 SDK 未接）。仍落在 `crates/nai-upgrade-core`（shared 层，零新增依赖，18 包）。
`cloud.rs` 的 `LinkGraph` 把系统建模成依赖方向图（节点分 `Local|Cloud`，边 `from depends_on to`），`audit_local_autonomy` 对每个关键节点求**传递闭包**
并断言可达云集合为空——**两跳依赖与直连同属可达**，三类稳定标记区分「依赖云」「不是本地」「未声明」（**否决**「只查直连」「写成文档表格」「运行时探测作为唯一实现」）。
可用性是链路状态与**归属**的函数：`(_, Local) => Available` 是一条恒等式，故本地静音在断云时仍可用；`CapabilityGate::disconnect` 的循环**只遍历云条目**，
本地能力连遍历都不进（「本地不受影响」是作用域事实），`reconnect` 把云能力落到 `NeedsPrepare` 而非 `Active`——§27.3「不自动恢复旧 Auto」由类型强制，
须一次显式 `activate()`。低云容量只能由 `Sizing::Measured(instances, load_pct)` 构造，`Unmeasured` 以 `CAPABILITY_NOT_READY` 拒绝且**无零默认值**
（「没测过」与「测到 0」类型不同），`assess` 报出**全部**越界族（不隐藏后项），超预算而策略为 `Absorb` 时 `audit_low_cloud` 报违规。
实测：`tests/cloud.rs` 21 项通过；`cargo xtask check --registry` 通过；`cargo xtask check --profile portable` EXIT=0（73 个测试目标，见报告）。
详见 docs/adr/0028-local-autonomy-boundary.md、docs/reports/2026-10-10-w26-cloud-disconnect.md 与 docs/cloud.md。

W27 Pro 多会话隔离已实现/实测（ADR 0029）：把 A11 的三句话落成**结构性**判据，registry A11 由 pending 升 implemented
（**不标 tested**：`automation: integration` 的真实多路会话与真实音频时序未做，准入用宿主给定的上限表）。落 **kernel** 的 `multiplex.rs`
（只依赖 contracts，18 包不变）——kernel 不许依赖 shared 层，且多会话是内核会话语义（`session.rs`）的延伸；kernel 只能依赖 contracts，
故自带 `MultiplexError{code,reason}`（与 `UpgradeError` 同形）。每会话命名空间以 `session_id` 为键并各有**独立 epoch**（`advance_epoch`
只推进被点会话）；到达 lane 的**唯一**入口以**属主**为键，`requester ≠ owner` 即 `CONFLICT`，故「任务/音频/审批/预算/知识无串场」是
**签名的性质**而非检查（**否决**全局 lane 表+检查、类型级隔离、lane 不设属主）。共享面是**封闭三枚举**（不可变模型/只读知识快照/受控连接池），
活的 lane 状态没有可共享表示；`SharedPool::write` 永远 `PERMISSION_DENIED`（没有写路径才是「只读」的实现），连接池持有者上限满即
`RESOURCE_EXHAUSTED`、零上限在构造时即拒。`Admission` 四族（`GpuThroughput | Vram | AudioTiming | ApiQuota`）**独立判定**（某族余量不能
换另一族），`admit` 是**两阶段**——先按族汇总并对照全机总量检查，**全部通过才写用量**，任一族不足即 `RESOURCE_EXHAUSTED` 且不写任何用量
（被拒会话为 0、已开播会话数字不变，对应「不能让已开播会话 OOM」），未声明族按 0。`audit_isolation` 检出无持有者租约与重复 session id。
实测：`tests/multiplex.rs` 24 项通过；`cargo xtask check --registry` 通过；`cargo xtask check --profile portable` EXIT=0（74 个测试目标，见报告）。
详见 docs/adr/0029-multi-session-isolation-boundary.md、docs/reports/2026-10-10-w27-multi-session-isolation.md 与 docs/multi-session.md。

W28 真人接管音轨与回声已实现/实测（ADR 0030）：把 B05 从「混音/回声未实现（W11）」这条真的没写的缺口落成 **media 层**的结构性判据，
registry B05 由 pending 升 implemented（**不标 tested**：真实声卡、真实 OBS/伴侣回声环境与真实设备默认变化未接入）。落
`crates/nai-media-core/src/mixing.rs`（只依赖 contracts，18 包与依赖方向不变，仍无 HTTP 客户端）——§8.5 是**声音通路**语义，与 kernel 会话语义
无交集，且 kernel 不许依赖 media；media 层自带 `MixError{code,reason}`（与 `UpgradeError`/`MultiplexError` 同形）。`takeover` 一次做完四件
（暂停自动、TTS 静音、清空队列并把**实际清空条数**作为报告值、可选接入真人轨），重复接管即 `CONFLICT`；「两轨不相加」由 `render()` 从**模式
推导**静音轨，故 `{tts>0,talent>0}` **没有构造点**（否决两个增益 setter + 调用约定）；`CaptureRegistry` 只允许**一条**有效收音路径（`acquire`
不看 owner/device 一律拒第二条——正是 OBS 与伴侣重复收音场景），`release` 只接受**当前代次**（旧租约 `STALE_EPOCH`）；`DeviceId` 是稳定不透明
标识，`observe_default` 把默认设备变化报成 `Changed{from,to}` 并记住而**不静默跟随**；接管要求**新鲜通过**的回声测试（未跑/运行中/失败→
`CAPABILITY_NOT_READY`，过期→`EVIDENCE_EXPIRED`，未来时刻无效，TTL 5 分钟）；`resume_auto` 每次复核授权+事实版本+值守租约，而
`on_talent_silence` **只读**返回当前模式，故「不因真人讲话结束而自动恢复」在 API 上**不可表达**（否决定时器与 `ResumeHint`）；`audit_mix`
对**上报快照**做纯函数，六类稳定标记全部可构造。
实测：`tests/mixing.rs` 18 项通过；`cargo xtask check --registry` 通过；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
详见 docs/adr/0030-talent-takeover-boundary.md、docs/reports/2026-10-10-w28-talent-takeover.md 与 docs/mixing.md。

W29 运营度量与场次复盘已实现/实测（ADR 0031）：把 B04（整链路延迟分位）、R06（长播无资源增长）、B06（场次复盘报告）的**判据口径**
落成 kernel 的可执行判据（真值仍需设备与长时运行），registry 三项由 pending 升 implemented（**均不标 tested**）。落
`crates/nai-kernel/src/metrics.rs` 与 `review.rs`（只依赖 contracts，18 包与依赖方向不变）。**§12.5 那句写死的口径**落成两个不同的调用：
`percentile` 只从**端到端样本**算，`stage_percentile` 各算各的，`sum_of_stage_percentiles` 仅作**具名对照**且 `stage_sum_gap` 显式给出差距
（否决插值分位与「端到端或阶段之和」开关）；分位用 nearest-rank（不插值）故返回的必是真实样本、可与原始数据逐条对照；端到端样本少于 20 条以
`CAPABILITY_NOT_READY` 拒绝（十份样本的 P95 实际上就是最大值）；`DESIGN_TARGETS` 收录 §12.5 的模板开口 1500ms / API 开口 3000ms / 控制响应
50ms，`evaluate_target` 返回 `Meets | Exceeds` 且样本不足返回 `Err` 而非达标。**长播判定**：窗口（2h/8h/24h）是**下界**，跨度或样本不足一律
`Insufficient` 并如实报出观察值，绝不报 `Stable`；比较两个半段的**中位数**（末端尖峰不误判、起始低值不掩盖增长；否决回归斜率与首尾比较），
`record` 要求时间非降序故趋势不能靠重排制造。**场次复盘报告**：`SessionReview::build` 覆盖 §5.9 末条全部字段（守门命中分类 = §5.4 四阶段 ×
四结果），构造即自洽（回答多于入选、拒答未被原因完全解释即拒），`Ratio` 保留分子分母，缺测进 `unmeasured` 且留空而**不写 0**，未定价成本是
`CostView::Unknown` 而**不是 ¥0**；`audit_review` 对公开报告值做纯函数，十条稳定标记全部可构造。
实测：`tests/metrics.rs` 13 项、`tests/review.rs` 8 项通过；`cargo xtask check --registry` 通过；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
详见 docs/adr/0031-operations-review-boundary.md、docs/reports/2026-10-10-w29-operations-review.md 与 docs/operations.md。

W30 黄金集知识质量门槛与回归门禁已实现/实测（ADR 0032）：把 B01 从「100 题黄金集未建（阶段 2）」这条口径空缺落成 kernel 的可执行判据
（真值仍需外部题集与真实模型评估），registry B01 由 pending 升 implemented（**不标 tested**）。落 `crates/nai-kernel/src/golden.rs`（只依赖
contracts，18 包与依赖方向不变，零新增依赖）——落 kernel 而非 shared 的 `nai-knowledge-core`，因为 §6 的检索 P95 要用 W29 已落在 kernel 的
`LatencySamples`，放 shared 要么反向依赖 kernel 要么复制一份分位（两个 P95 是必然结果）；比例同样复用 `review::Ratio`。三处最容易含糊的地方被
钉死：**①** 「Recall@5 ≥90%」没有分母 → 定为**题级**（全部所需证据进前 `RECALL_K=5` 才计命中），文档粒度 `evidence_doc_recall_at_5` 只作
**对照**且两口径分开呈现（否决单报文档粒度）；没有有证据用例时召回为**未测量**而非 0%。**②** 「检索延迟、证据正确率、最终答案正确率分别报告」
是一句**禁止令** → 三个独立字段、**没有任何合成方法**（否决加权质量分）；`AnswerVerdict::Unknown`（未判定）从正确率分母**排除**。
**③** 「必须通过相关回归才生效」是一道**门禁** → `regression` 四条拒绝：版本不一致 `VERSION_INCOMPATIBLE`（跨版本不比较）、候选任一门槛
失败 `candidate_gate_failed`（基线不绿也不放行候选）、**逐用例**由通过变失败 `case_regressed`（1000‰→989‰ 亦拒，不因平均分未降而放行）、
召回或正确率低于基线 `recall_regressed`/`accuracy_regressed`。四条否定式判据为逐用例一票否决（`HighRiskFact` 只对自动播报生效——澄清/转人工/
拒答皆正确；`MustClarifyOrEscalate` 把「自动播报」与「拒答但未澄清」分开报；`MustNotAct` 逐条列出副作用；`MustCancel` 记结束方式）；黄金集绑定
场景版本、覆盖 §5.9 八类并受 `MINIMUM_CASES=100` 约束，`audit_coverage` 在**运行前**报用例配置缺陷（不污染召回）；检索 P95 <20 样本即未测量
且门槛**计为未达标**、恰好 300ms 判达标；坏案例入池开放、出池须**具名确认人**（`CaseOrigin::Promoted` 保存确认人，未确认候选无法伪装成
authored）；`audit_report` 对上报值做纯函数，五条稳定标记全部可构造。
实测：`tests/golden.rs` 24 项通过；`cargo xtask check --registry` 通过；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
详见 docs/adr/0032-knowledge-quality-gate-boundary.md、docs/reports/2026-10-10-w30-knowledge-quality-gate.md 与 docs/golden-set.md。

W31 黄金集载荷载体与就位门禁已实现/实测（ADR 0033）：给「100 题黄金集从哪里来」一个**机器可寻**的答案，并修正 B01 的验收口径。
**口径修正**：§655 的 B01 验收原文是「知识质量：**100 题集达到**第 6 章门槛」＝**实测达标**；W30 标 `implemented` 的准确含义只是「判定机制
就位」，题集不存在时达标不可能达成，故 registry B01 由 `implemented` **回调为 `blocked`**（已交付的判定机制与载荷载体在 notes 中完整保留）。
**不新增 B01a/B01b 编号**——registry 的 id 正则 `^(S|R|A|B|O|P|X)[0-9]{2}$` 要求恰好两位数字，拆分会直接使 `check --registry` 失败，且等于
改写规格口径（超出本仓库授权）。载体三件：**①一处位置**＝§30.1 指定的 `tests/rag-golden/*.json`（该目录此前不存在）；**②一份结构规范**＝
`schemas/rag-golden/case-set.schema.json`（`case_set_schema` const 1、`scenario_version`、`cases`；`expectation` 五形
evidence/high_risk_fact/must_clarify_or_escalate/must_not_act/must_cancel；`origin` 两形 authored/promoted+**非空 confirmer**；
`additionalProperties:false` 全面拒未知字段——被忽略的拼写错误会读成「字段缺失」并静默改变用例含义）；**③一道就位门禁**＝
`cargo xtask check --golden [--dir <目录>]`，把三种结论**分开**报告：目录内无 `*.json` ⇒ `CAPABILITY_NOT_READY`（**未提供**，指出被搜索目录，
既不是通过也不是失败）、有文件但读不出 ⇒ 逐个报出**全部**结构缺陷（文件名 + JSON 路径 + 稳定 token）、读出但不达 §5.9/§6 ⇒ 报全部 finding
（`below_minimum_cases`/`category_uncovered`/`expectation_category_mismatch`）。`is_unsupplied()` 以 **`files == 0` 为唯一判据**，
**否决**由 `sets.is_empty()` 推断——后者在「有文件但全部被拒」时同样为真，会把「载荷是垃圾」误报成「从未提供」，这正是本轮要消除的混淆。
§5.9 的授权边界**前移**到载荷边界：`origin.kind = promoted` 缺或空白 `confirmer` 在**读取载荷时**即拒（空白与缺失同罚），使含未确认候选的
题集不会先参与召回统计、从而把题集缺陷误读成「系统在某类问题上表现差」。落 `xtask`（**tool 层**）：复用 kernel 的
`GoldenSet`/`audit_coverage`/`MINIMUM_CASES`，**判定逻辑只有一个真相**（不复制类别表/最低题数/分位），且 xtask 的 **lib 目标在便携门控内**
（`cargo test --package nai-xtask --lib`）故逻辑被工程门控覆盖——这是选 tool 层而非 shared 的 `nai-knowledge-core` 的关键：kernel 被禁止
读文件（只依赖 contracts），shared 层不依赖 serde。新增依赖边 `xtask → nai-kernel` 属 tool 层允许（deps.rs 对 tool 层不设禁止规则），
`check-deps` 仍 **18 包**、方向不变，**外部依赖零新增**（复用已有 `serde_json`；YAML 需新增外部依赖且 Rust 侧读 YAML 无先例，仓库唯一的
`tests/registry.yaml` 由 Node 侧读取）。该命令**不并入** `check --profile portable`：业务题集未建不应让工程基础门控变红。**本轮不产生任何题
内容**——题集是场景包载荷（§5.9 场景包含「预算与黄金集」⇒ 逐场景各一份），三条人工来源＝按场景编写（§6「至少 100 个贴近真实直播的问题」，
整条门槛标注（设计值））/ 随场景包发布 / 坏案例**经具名人工确认**回流；`tests/fixtures/rag-golden/example-min.json` 是**夹具**（8 例最小集，
验证加载器本身）**不是**真值，且不在默认位置故不被默认命令读取。
实测：`cargo test -p nai-xtask --lib` **27 项**通过（golden 17 + 原 deps 8 + schema 2）；`check --golden` 对真实目录报 `files: 0` +
`CAPABILITY_NOT_READY`（退出 1）、对夹具目录读入 8 例并报 `below_minimum_cases: 8 of 100`（退出 1）——两种结论确实分开；`check --registry`
通过；`check-deps` 18 包；`cargo xtask check --profile portable` EXIT=0（数字见报告）。
详见 docs/adr/0033-golden-set-payload-boundary.md、docs/reports/2026-10-10-w31-golden-set-payload.md 与 tests/rag-golden/README.md。

W32 写实准入 Gate 判定骨架已实现/实测（ADR 0034）：B02 与 D12 的**真值**（合法完整模型 + Windows/CUDA 目标机 + 与直播工具并发实测）
需外部条件，本轮不可达，两项保持 `blocked`；本轮做的是 **B02 的判定骨架**——§8.2 的表只有四行，每行的判据此前无落点。
落 `crates/nai-kernel/src/realistic.rs`（只依赖 contracts，18 包与依赖方向不变，零新增依赖），复用 `metrics::LatencySamples`（G2 音画偏差
P95 一个真相）与 `golden::Finding`（同一报告词汇，`Finding::new` 提升为 `pub`）。**四门是合取、各自独立**：G0 `SourceAndRights` / G1
`Correctness` / G2 `Realtime` / G3 `Stability` 各自 `Passed/Failed/Unmeasured`，总体 `Acceptance` 由四门**派生**（Lite 未启用⇒`NotApplicable`、
缺模型⇒`Blocked`、任一 `Failed`⇒`NotAccepted`、任一 `Unmeasured`⇒`Unmeasured`、全 `Passed`⇒`Accepted`），**没有任何单分数**。
**缺测既不是通过也不是零**（帧率无窗口、音画偏差样本不足 `MIN_PERCENTILE_SAMPLES=20`、温度未读一律 `Unmeasured` 且阻断 `Accepted`）。
**G0**：权重/代码/训练素材/肖像/声音五类权利各为 `Established{basis}`/`Unverified`/`Absent`，**依据空白不算成立**（「许可引用存在不等于
证据通过」，D12 自身结论），任一 `Absent`、链路不可取得、或存在**留在未声明 Python 服务**的缺组件 ⇒ `Failed`。**G1**：预处理/音频特征/
生成/后处理四段在**固定输入**下对齐，任一段 `Diverged`⇒`Failed`，未运行或固定输入未指明⇒`Unmeasured`；§26.5 **精度有序**——非 FP32
精度在 FP32 之前认证⇒`Failed`（`precision_without_fp32_baseline`），INT8 无量化校准报告⇒`Failed`（`int8_without_calibration`）。
**G2**：目标机未指明⇒`Unmeasured`，未与直播工具并发⇒`Failed`；帧率以**窗口下限**判是否稳定达 `MIN_STABLE_FPS=25`（**否决**会被突发拉高的
平均），音画偏差 `LatencySamples::percentile(P95)` ≤ `FRAME_SKEW_P95_LIMIT_MS=80`（<20 样本 `Unmeasured`，恰好 80ms 判达标）；
显存/CPU/温度任一未测⇒非 `Passed`；**「全 GPU」是 `PlacementReport` 放置记录（非空且全 GPU）的性质**，`cuda_ep_registered` 只是审计线索
（§26.5「不以 CUDA EP 注册成功宣称全 GPU」），宣称全 GPU 而无记录或存在 CPU 回落⇒`Failed`，但**如实记录** CPU 回落且指标达标仍可 `Passed`。
**G3**：静音/取消/句边界/长播/OOM/进程重启/形象切换与输出恢复七场景，任一 `Failed`⇒`Failed`、未运行⇒`Unmeasured`、**声明通过而证据引用
空白**⇒`Failed`（`scenario_passed_without_evidence`）。**2D 回退与准入正交**：`FallbackRecord` 不进入 `verdict`/`acceptance`，
只有「以回退构成验收」的**声明**违规（`fallback_claimed_as_acceptance`；§8.2「自动回退 2D 不等于写实验收通过」），回退缺授权/不在句间/
未记日志分别被审计报出。**§1311 纪律**：缺模型⇒`Blocked`（缺模型却有门判通过被审计报出 `gates_evaluated_without_model`），
NotApplicable 只用于**不要求写实**的档位（Lite），Standard/Pro 声明不适用被审计报出（`not_applicable_for_target_capability`），
**无豁免路径**。`audit_gate` 对报告字段做纯函数，十四条稳定标记全部可构造。**不新增 xtask 命令与证据目录**：§30.1 只为 RAG 黄金集指定
`tests/rag-golden`，为写实 Gate **臆造**证据目录属过度绑定；G2 真值来自目标机实测而非仓库载荷，待真实模型轨立项后再补载体。
实测：`tests/realistic.rs` **37 项**通过；`cargo xtask check --registry` 通过；`check-deps` 18 包；`cargo xtask check --profile portable` EXIT=0。
详见 docs/adr/0034-realistic-gate-verdict-boundary.md、docs/reports/2026-10-10-w32-realistic-gate-verdict.md 与 docs/realistic-gate.md。

W33 手机监控面判定骨架已实现/实测（ADR 0035）：**O03** 的真值（M0b PoC 定路线 A/B + 真机最小形态通过 S09）需外部条件，本轮不可达，
O03 保持 `blocked`（并**补齐了原先缺失的 blocked 原因**——原 notes 仅记「Blocked 原因待 registry 标注」）；本轮做的是 **O03 的判定骨架**，
§3.1 L5 与 §10.2 的越界口径此前**全库无落点**（`nai-control-api` 是桌面 loopback 传输面，其文件头明说不做设备认证/预检；
`nai-kernel::platform` 管的是平台事件门）。落 `crates/nai-kernel/src/monitoring.rs`（只依赖 contracts，18 包与依赖方向不变，
零新增依赖），复用 `golden::Finding`（同一报告词汇）。**「只发命令、看状态，不持有媒体时钟与密钥」落成白名单投影而非黑名单**：
`MobileView` 结构上只含十个白名单字段（`mode`/`current_utterance`/`pending_seconds`/`event_source`/`api_status`/`budget_status`/
`device_status`/`pending_approvals`/`alerts`/`enable_status`），`LiveState` 的 `media_clock`/`secret`/`raw_audio`/`raw_video`/
`broadcast_control` 在它之上**没有落脚处**；`MobileView::field_names()` 与 `Exposure::ALLOWED` 标签集合被断言相等（**给视图加字段而漏登记
即测试失败**），`LiveState::sensitive_field_names()` 与投影字段集合被断言不相交（**否决**事后剔除：给 `LiveState` 加字段就会默认流出）。
**最小形态是四项清单**（`MobileCapability::MINIMUM_FORM`＝看板/审批/接管/紧急静音），缺项逐条指名（`minimum_form_incomplete`），
预览与推送后置不算缺陷。**「它不是手机开播」**（§10.2）与「手机只做监控管理」（§742）落成禁止暴露类别 `BroadcastControl`
（`exposes_broadcast_control`），与媒体时钟/密钥/原始音视频同族。**预览四条件仅在启用时受检**（按需、真正编码的低码率、关闭立即释放
编码器与带宽、不得挤占正式出画），未启用时**一律不产生缺陷**（**否决**把「不适用」读成「不合规」）。**中继终止 TLS 不得宣称端到端**
（§10.2）：`relay_terminates_tls` 且 `claims_end_to_end` ⇒ `e2e_encryption_claimed_with_relay_terminated_tls`、宣称 E2E 而协议未审核
⇒ `e2e_encryption_claimed_without_reviewed_protocol`，**如实披露中继可读而不宣称 E2E 不算缺陷**（与 W32「不以 CUDA EP 注册宣称全 GPU」
同形）。**路线 A/B 前置逐条**：A 缺有效 HTTPS 域名或证书 ⇒ `relay_pwa_without_secure_context`（Service Worker 需安全上下文）、
B 缺应用分发 ⇒ `native_shell_without_distribution`、Tauri 2 成熟度未验证 ⇒ `native_shell_tauri_maturity_unverified`。
`readiness` 三态且 `Defective` **优先**——审计非空⇒`Defective`、否则路线 `Undecided`⇒`AwaitingRouteDecision`（**未定案是诚实状态而非
缺陷**）、已声明路线且无违规⇒`Ready`（**否决**未定案即缺陷或即就绪，也否决未定案掩盖越界）。**远程命令带 S09 契约**（§86 INV-14
与 §636：身份、会话与 epoch、序号、TTL、实际 ACK，缺项 ⇒ `command_contract_incomplete` 并指名命令）且**高危不得开放给观察者**
（`high_risk_command_open_to_observer`，**S09「观察者执行高危命令被拒」**）；真实防重放/去重/ACK 与吊销即时生效属 S09 集成验收，
不在本模块。运营项：紧急静音不可达 ⇒ `emergency_mute_unreachable`（§10.2 最小形态、§37.3 紧急停用）、未提供启用状态 ⇒
`enable_status_missing`（§37.3）、知识速补未接审核发布流程 ⇒ `knowledge_patch_without_review_flow`（**仅在声明 `KnowledgePatch` 能力时判**）。
`audit_surface` 是描述符的纯函数，**十九条**稳定标记逐条可构造，并以「恰为这十九条」断言防止出现未登记的额外标记。
**不新增 xtask 命令与证据目录**：§30.1 为 UI 层指定 `tests/ui`、为 RAG 黄金集指定 `tests/rag-golden`；手机监控面真值来自真机与 M0b PoC
而非仓库载荷，**臆造** `tests/mobile-monitoring` 属过度绑定。实测：`tests/monitoring.rs` **34 项**通过；`cargo xtask check --registry`
通过；`check-deps` 18 包；`cargo xtask check --profile portable` EXIT=0。详见 docs/adr/0035-mobile-monitoring-surface-boundary.md、
docs/reports/2026-10-10-w33-mobile-monitoring-surface.md 与 docs/mobile-monitoring.md。

W34 设备接入判定骨架（ADR 0036）已实现/实测：**O04**（安装与设备）三条验收（§1329「干净 Windows 安装且运行库可取得；音频设备变更不重复收音；
升级可回退」）需外部条件，O04 保持 `blocked`；**安装侧已由 `nai-upgrade-core` 的 `manifest`/`preflight` 覆盖、回退由 `plan`/`gates` 覆盖**，
本轮补的是此前**全库无落点**的**设备侧**（§1424 缺口清单原文「没有设备与硬件接入的抽象，也没有物理安全联锁」）。落
`crates/nai-kernel/src/device.rs`（只依赖 contracts，18 包与方向不变，零新增依赖），复用 `golden::Finding`。钉死的口径：**设备能力靠声明**
（§1469 动作枚举/取值范围/速率限制，缺项指名设备与动作）；**执行类必须安全联锁 + 本地手动覆盖**（§1469/§1497）而**纯 `Sense` 不适用**——
**否决**把「不适用」读成「不合规」（同 W33 预览未启用）；**物理执行类推导 `risk_3`、默认禁用**（§34.4）、需本地确认与独立开关（§1469），
`Sense` 标注物理执行为分类错误；**有效风险 = `max(声明, 推导)` 且声明低于推导即拒**（§35.3，声明方可主动提高但不可自我降级）；
**驱动与 SDK 不进内核进程**（§34.4）落成 `DeviceHostPlacement`，是独立声明故不受是否有设备门控；**热插拔/断线/重复/超时各有处置**（§1498）
且**写决策日志**（§1497），**未声明设备时不存在设备事件故条件化**；**「音频设备变更不重复收音」**（O04、§8.5「只允许一条有效收音路径」）
落成 `duplicate_event_opens_second_capture_path`——判**结果**而非「处置了」；**INV-23 实际权限不得超出声明与用户授权**落成使用侧审计
（宿主强制执行、不靠自觉），未声明只报一次、不重复报越权；`readiness` 三态 `Blocked`/`Defective`/`Ready` 且**先取审计**
（`Defective` 优先，`Blocked` 判据是审计为空且无声明）。`audit_host` 是描述符纯函数，**十六条**稳定标记逐条可构造。
**不新增 xtask 命令与证据目录**：§30.1 为升级/RAG/UI 指定 `tests/upgrade`/`tests/rag-golden`/`tests/ui`，未为设备接入指定目录；真值来自
受控 Windows 机器与真实设备而非仓库载荷。实测：`tests/device.rs` **31 项**通过；`cargo xtask check --registry` 通过；`check-deps` 18 包；
`cargo xtask check --profile portable` EXIT=0。详见 docs/adr/0036-device-admission-boundary.md、
docs/reports/2026-10-10-w34-device-admission.md 与 docs/device-admission.md。

W35 平台生命周期判定骨架（ADR 0037）已实现/实测：**P01**（平台生命周期，§1328「伴侣启动、退出、重连、切场次不产生旧房间播报；权限撤销停止相关功能」）
的真值需外部条件（官方 SDK 认证 + 生产 composition root 平台宿主接线 + 真实启动/退出/端口冲突与网关），P01 保持 `blocked`（其 notes 原仅记
「官方 SDK/生产宿主未实现（D13）」）。**D13 已交付的是平台事件准入**——`nai-kernel::platform` 的 `PlatformLink` 判「一个事件能不能进」；
本轮补的是此前**全库没有落点**的**桥接进程生命周期**（`grep bridge` 在 kernel 内为空）。落 `crates/nai-kernel/src/bridge.rs`（只依赖 contracts，
18 包与依赖方向不变，零新增依赖），复用 `golden::Finding`。**①** §27.1「桥接只做：SDK 初始化、官方事件与权限转换、认证 IPC、退出」落成**封闭四项**，
越界职责逐条指名；**②** §1328 四类转换（启动/退出/重连/切场次）各有处置、**各清播报授权**、各写决策日志，**未声明桥接职责时不存在转换故条件化**；
**③** 「不产生旧房间播报」判**两个面**——处置面 `transition_keeps_stale_broadcast` + 状态面 `stale_room_broadcast_permitted`（授权绑定**房间与连接代次**，
`may_broadcast` 为运行时判据），状态面**不受门控**；**④** §347 退出转待机与重连不恢复旧自动权限**都落在转换处置的声明上**（否决另设描述符级引擎状态）；
**⑤** §349 能力快照 `unknown` 取**最严格**（读作已授予即报出），「**只**关闭对应功能」判**双向**（欠关闭 `feature_active_without_capability` 与
因能力变更过关闭 `feature_stopped_despite_capability`，后者带 `stopped_by_capability_change` 前置以免误报用户主动关闭）；**⑥** §119 平台中断**三态**
（撤自动、保手工、保回放）条件化于 `bridge_lost`；**⑦** §347 **原文七类**情形各有处置与记录；**⑧** §27.1/§1075 官方 SDK 只经固定路径与
签名/nonce/令牌校验加载，无官方 SDK 时如实记 `UNVERIFIED_EXTERNAL_DEPENDENCY`、**不用 fake connector 冒充**（该错误码此前在 workspace 内从未被使用）；
**⑨** `readiness` 三态且**先取审计**，支点是三条**不受门控**的独立判据（陈旧房间授权、伪造替代、功能开关双向）。`audit_bridge` 是描述符纯函数，
**十九条**稳定标记逐条可构造。**不新增 xtask 命令与证据目录**：§30.1 为升级/RAG/UI 指定 `tests/upgrade`/`tests/rag-golden`/`tests/ui`，
未为平台生命周期指定；真值来自官方 SDK 与真实账号而非仓库载荷。实测：`tests/bridge.rs` **44 项**通过；`cargo xtask check --registry` 通过；
`check-deps` 18 包；`cargo xtask check --profile portable` EXIT=0。详见 docs/adr/0037-platform-lifecycle-boundary.md、
docs/reports/2026-10-10-w35-platform-lifecycle.md 与 docs/platform-lifecycle.md。

W36 真实 Provider 接入纪律与真实调用证据已实现/实测（ADR 0038）：真实凭据到位后风险从「机制不存在」变为「接入会污染仓库或把事实读错」，
本轮把纪律固化成**可执行、可检查**的形态而非写进文档靠自觉。落 **`xtask/src/secrets.rs`**（tool 层，无依赖限制）与 **`scripts/provider-smoke.mjs`**
（Node 22，零新增依赖），**`provider-core` 零改动**、18 包与依赖方向不变：**①** §30.1/§1469/S07 落成 `cargo xtask check --secrets`，扫描域取
**git 会发布的内容**（`git ls-files --cached --others --exclude-standard`）——被忽略的本地 `.env` **不在**域（否则本机无法持有凭据），
而**运行期新建、尚未暂存**的文件**在**域（否则检查只能在错误几乎铸成之后才发现），**两条测试各钉一个方向**；按**前缀＋最小长度表**匹配，
命中只报**文件/行号/前缀族**、**不回显匹配内容**；扫描器源码**不得自命中**（测试夹具运行期拼装）。**②** 接入**只是配置**：两家真实端点都是
OpenAI 兼容，`Protocol::OpenAiCompatible` 已同时 `serves` 文本与语音，故 `Protocol` 枚举、适配器集合与 `provider-core` **全部零改动**，只新增一份 `ProviderProfile`。
**③** 缺凭据 **= `CAPABILITY_NOT_READY`** 非零退出、打印缺失**变量名**、**不写出**任何证据或音频，**绝不回退** `RehearsalTextGenerator`/`RehearsalSpeechSynthesizer`
（§30.1 点名「用固定素材冒充」）。**④** **可用性是账户状态**：同端点逐模型记录，模型甲 `200`、模型乙 `403 insufficient_user_quota`（报文余量为负）、
模型丙 `503 model_not_found`（渠道缺失记作**渠道事实**、**不是**凭据无效），结论**只覆盖被测模型**。**⑤** 证据**只含事实、连凭据前缀都不含**：
主机名/路径/方法/协议名/凭据**方案名**/状态码/网关错误码/用量字段名/字节数/media type，**不记**请求头值、请求行原文与凭据片段。**⑥** 音频证据是**真实字节**，
字节数取自响应体、media type 取自响应头，空体/错误报文判为**未产出证据**。**⑦** 契约差异**以实测为准**（音色裸名被 `400`＋错误码拒，端点要求的
`model:voice` 完整形式返回 `audio/mpeg`），**不静默归一化、不重试掩饰**。**⑧** 主机差异是**环境事实**（本机文本端点报 `CRYPT_E_REVOCATION_OFFLINE`），
记为**主机**注记、**不**读作端点事实。**⑨** **冒烟不是验收**：连通成功**只**新增连通性事实，**不改** B01/O01 登记状态。实测：`check --secrets` 全部待发布文件**零命中**；
`cargo test -p nai-xtask --lib` **35 项**通过；两条真实链路经**本栈**（`ProviderProfile → ProviderClient → 适配器 → 预算网关`）走通；`check --registry` 通过；
`check-deps` 18 包；`cargo xtask check --profile portable` EXIT=0。详见 docs/adr/0038-live-provider-evidence-boundary.md、
docs/reports/2026-10-10-w36-live-provider-evidence.md 与 docs/provider-live.md。

W37 文本请求的轮次呈现已实现/实测（ADR 0039）：W36 用真实凭据跑两条冒烟时，**经本栈**的文本链路在 `agnes-2.5-flash` 上被
`400 No user query found in messages` 拒绝。根因在**我们自己的适配器**——`permitted_context` 这个字段，Anthropic 适配器作为
`role: "user"` 发出（有注释说明理由），**OpenAI 适配器却作为 `role: "system"` 发出**，**同一语义字段两个相反角色**。W19 把该差异
当作「实测到的供应商差异」用测试钉住并写进对照表，但**记录差异不等于判断对错**；真实网关给出裁决（同一网关同一请求体，
`agnes-3.0-flash` 200、`agnes-2.5-flash` 400）⇒ **system-only 不是普遍合法形状**。本轮把 `openai.rs` 的 `messages[0].role`
由 `system` 改为 `user`（与 Anthropic 一致），并**把性质写进规格**（`api-provider-transport` 新增需求「permitted context 以恰好一条
user 轮呈现」，2 场景）而不只是改一行。**否决**「system+user 两条」（适配器 MUST NOT 自拟提示文本 = 发明内容）、**否决**「角色可配置」
（角色是最小合法形状而非供应商差异）。**第二种问题**：修好角色后本栈冒烟返回 `chunks=1 chars=0`——链路通了却**没有可见输出**。
实测根因是**推理模型与 token 上限的交互**（同一端点：`max_tokens=32` ⇒ 可见 `content` 为空、`reasoning_content` 67 字符、
`finish_reason=length`；`max_tokens=256` ⇒ `"\n\n收到"`、542 字符、`stop`）：模型先流 `reasoning_content`（思考）再流可见 `content`，
上限只够思考时冒烟会在**每项检查都通过**时报出**空心成功**。故两处冒烟的 `max_tokens` 由 32 提到 **256**（附注释说明理由），
**不**把「可见输出为空」做成失败判据。**第三种修正**：`sse_delta_keys` 原取**首帧**，推理模型的 `reasoning_content` 帧完全不被记录
⇒ 改为记**所有增量帧键的并集**。契约与共享组件**不变**（`Protocol`/`ProviderProfile`/`ProviderClient`/SSE 解码/用量映射/预算接线），
零新增依赖，18 包与依赖方向不变。勘误：`docs/provider.md` 对照表与 W19/W36 报告已加注记。实测：
`cargo test -p nai-provider-core --offline` **36 项**通过；两处冒烟文本链路由 400 变为 **200 且有可见输出**；
`check --secrets` 680 文件零命中；`check --registry` 通过；`check-deps` 18 包；`cargo xtask check --profile portable` EXIT=0。
详见 docs/adr/0039-provider-message-turn.md 与 docs/reports/2026-10-10-w37-provider-message-turn.md。

W38 桌面审批面与 §30.1 的 UI 层已实现/实测：W09 交付了审批 **API** 与存储原子日志，但**没有界面**，且设计 §30.1 为 UI 层指定的
`tests/ui` 目录在仓库里**从未存在过**。本轮把两者都补上，并在摸清事实时撞到**两条硬约束**。**F1（未满足的验收）**：设计 §662（O02）与
§425（T-14）要求「审批按钮显示最终文本」，本栈做不到——`utterances` 表只保存 `text_hash`（`migrations/sqlite/0001-foundation.sql:10`），
审批列表接口也只返回哈希/元信息引用/期限/证据计数（`crates/nai-control-api/src/lib.rs:540`；`schemas/openapi/control.yaml:84-97`、
`ResourceView`/`ResourceItem` :279-299），载荷里**没有文本字段**。做法是界面**如实说**（`FINAL_TEXT_GAP` 写明「本载荷不含最终文本、
身份为候选哈希」并直接指向 §662），**不**把哈希当文本展示、**不**为此临时发明字段；补齐需存储侧保存可展示文本并同步契约，**不在本轮范围**，
已写进 O02 notes 的未满足项。**F2（时效）**：候选的有效性绑定 TTL 与会话代次（`crates/nai-storage/src/store.rs:731` 的 `list_pending_approvals`
在 SQL 里就按 `u.generation_epoch=?3` 与 `u.expires_at_ms>?5` 过滤），故「界面还显示着、但已经不该批」是正常会发生的事 ⇒ 动作前**复核新鲜度**
（`approvalFreshness` 比较面板载入时的快照与动作前的实时快照：会话换代/已不在最新队列/无有效期/已过期 ⇒ **不发请求**并给具名理由与下一步；
把已失效的候选呈现为可点本身就是错误的反馈）。决策 D1–D8：界面侧复核而非信任快照、批准二次确认而拒绝单步（批准是**高危方向**——放行将要
播报的内容）、看不见文本就如实说、UI 层放 §30.1 **指定**的 `tests/ui` 且**不引入 DOM 库**、未实现的断言登记并点名前置、不改服务端与契约
（Rust 侧**零改动**）、登记状态不变、零新增依赖。落 `apps/desktop/src/console.ts`（无 DOM 的可测逻辑：`approvalFreshness`/`nextApprovalStep`/
`describeCommandResult`/`runApprovalDecision`/`UI_ASSERTIONS`）与 `control.ts` 的 `approvals()`/`decideApproval()`（真实 `ApprovalCommand`：
`candidate_hash` 取候选哈希、`hash_algorithm: sha256-canonical-v1`、`seq` 单调、epoch/revision 取自真实视图、TTL 有界），界面为待审批面板
（候选哈希/状态/证据计数/有效期/200 上限提示）+ 批准二次确认 + 拒绝；页脚不再声称「不提供审批按钮」，改为如实说明缺口。`tests/ui` **15 项**通过
（`approval-surface.test.mjs` 11 项：失效不发请求、两步确认、命令字段逐字、假 ACK 不变成已批准且不重试、严格解析、控制 ACK 口径；
`ui-layer-declaration.test.mjs` 4 项：断言集合**恰为**四条、状态确定、未实现点名前置）。§30.1 四条里「模式切换条件」（缺
`POST /sessions/{id}/mode` 实现，契约见 `schemas/openapi/control.yaml:66`）与「升级与回滚提示」（缺 `nai-upgrade-core` 的 plan/journal
状态来源）登记为**未实现**并点名前置。18 包与依赖方向不变。实测：`check --secrets` 689 文件零命中；`check --registry` 通过；`check-deps` 18 包；
`check --profile portable` EXIT=0（675 passed / 0 failed）；两处 `tsc --noEmit` EXIT=0；`vite build` 30 模块；desktop 5 项 + `test:ui` 15 项通过。
见 docs/ui.md、docs/control.md 与 docs/reports/2026-10-10-w38-desktop-approval-surface.md。未做边界：**不测渲染**（不引入 DOM 库 ⇒ 不构成
浏览器或人工 UI 验收）；`local_control` 示例未装配审批 backend（手工点开界面仍返回 `CAPABILITY_NOT_READY`，`tests/ui` 用替身驱动）；
真实候选来源/证据详情页/手机审核/生产 root 接线仍未建；**O02 保持 `pending`**（界面出现不等于验收达标）、O05 未动。
（**计数勘误**：此前报告的「82 个测试目标（65 Running + 17 Doc-tests）」把 cargo 打印的 `Running ...nai-xtask.exe check --profile portable`
包装行多算了一个，它不是测试目标；按 `Running <test target>` 精确重数为 **81（64 Running + 17 Doc-tests）**，`675 passed / 0 failed` 不变。
W36/W37 报告已加注记、未改写正文。）

W39 攻击者可控输入面的随机输入测试已实现/实测（S11 的「模糊」这一半）：S11 验收名是「IPC **模糊**与路径穿越」，
状态 `tested`，而同一行 notes 写着「**模糊测试待建**」，全仓库 `grep -ri fuzz` **零命中**——验收名里的一半没实现却被标成 `tested`，
属**登记与事实不一致**，比缺功能更危险（读者会据此以为攻击者可控输入的健壮性已被验证）。审计发现四个攻击者可控入口中
**两个此前零测试**：`Envelope`/`CapabilityInvocation`/`CancelRequest` 解码（`host.rs:154,181,195`）与 `Handshake` 畸形 body
（`handshake.rs:33`）；路径面只有 1 项手写用例（`boundary.rs:119`）；且「**撤销/期限先于内容解释**」的顺序不变式此前只用
**一个特定输入**钉过。本轮落 `crates/nai-plugin-hosts/tests/untrusted_input.rs`（**8 项**，**实现一行未改**、**零新增依赖**，
自带 splitmix64 固定种子）：断言**全域性与有界性**——任意字节 ⇒ `Ok` 或**具名 `ErrorCode`**、不 panic、不按输入无界分配
（超长长度前缀**在分配前**拒）、取消/期限对**任意**输入都优先——外加随机 body 往返逐字节相同、**被拒的帧终止连接**
（其后的合法帧不被继续服务）。**非空洞性守卫是关键**：生成器只产合法输入会让全部断言通过而什么都没证明，故语料
**结构化**构造危险形状（`0xFFFFFFFF` 长度前缀、截断 body、合法帧后跟垃圾、**合法 Envelope 头 + 任意 `message_type`/payload**、
由 `{正常段,"..",".",空段,长段,unicode}` 组合的路径）并统计**实际命中**，计数为零即**测试失败**；守卫做过**反向自检**
（阈值调到不可能命中 ⇒ 帧流与路径两个测试**确实失败**并打印计数，随后恢复）。实测命中：帧流
`bodies=683/clean_eof=179/oversized=1982/truncated=1935`；结构化 Envelope
`invocation_decoded=1569/version_incompatible=601/invalid_argument=846/denied_or_absent=225/completed=185/cancel_acks=191`；
路径 `rejected=1927/accepted=121/traversal_rejected=651`。过程中 harness 逼出两处**真实语义**（都不是缺陷，但不知道就会读错）：
**①** ACL 拒绝与能力缺失是**result 级**错误而非连接级错误——`dispatch` 把 `CapabilityResult{end:StreamError, error:PermissionDenied}`
**写回对端**后返回 `Ok(())`（`host.rs:258-264`），故 `serve` 返回 `Ok` **不代表放行**，只看返回值会把 225 次拒绝**误计为成功**
（harness 因此改为解析宿主**写出的帧**，并顺带断言宿主只输出良构帧与已登记消息类型）；**②** 空 payload 是**合法**的 `CancelRequest`
（protobuf 默认值语义），「看起来是垃圾」的帧会走到取消路径并收到 `CancelAck`。**口径必须说准**：这不是覆盖率导向模糊测试
（无 cargo-fuzz/libFuzzer、无 nightly、无独立语料目录），一律称「随机输入测试」——名字骗人就是证据骗人；
**阴性结果不是安全证明**，本轮**未发现**缺陷，**不**据此宣称不存在缺陷或「系统安全」；不新增仓库级证据目录
（§30.1 未为模糊测试指定，per-crate `tests/` 是既有惯例，**不**臆造 `tests/fuzz`）。实测：`cargo test -p nai-plugin-hosts --offline`
**6+3+8 全通过**（约 0.5s，不构成门控瓶颈）；`check --secrets/--registry/check-deps/--profile portable` 全通过；
openspec 归档校验全通过。规格：`plugin-hosts-and-ipc` 新增 3 条需求 / 6 个场景。见 docs/hosts.md（新增一节）与
docs/reports/2026-10-11-w39-ipc-untrusted-input.md。未做边界：**不测传输层对端身份**——真实命名管道/Unix socket 的
`SO_PEERCRED` 在钉住的 rustc 1.96.0 上仍不稳定（E0658），Windows Named Pipe peer PID、AppContainer/Job Object 未实测（Blocked）；
fuzz 的是**解码与分发**而非传输；**随机语料计数不是分支覆盖证据**。

W40 黄金集评估执行体与参考答案判据已实现/实测（ADR 0040）：B01 的判定机制（W30）与载荷载体（W31）都已交付，但对照 §655
「100 题集**达到**第 6 章门槛」逐条核对后发现仍缺四段，**性质完全不同**——①100 题题集内容、④真实知识库内容属**数据**（必须由场景包
产出方人工投入，**代造即臆造真值**，ADR 0033 决策 6 已明禁）；②参考答案判据、③评估执行体属**仓库内可控**。②最容易被忽略：
题集 schema 原本**不含参考答案**，而 `Submission.answer: AnswerVerdict` 由调用方注入 ⇒ 即使 100 题到位，`answer_accuracy` 依然是
外部判定后填进去的，达标链条上有一段**没有可复现的执行体**。③是核对后确认的真缺口：全仓库没有任何代码把「题集 → 真实检索 →
真实生成 → 逐题判定 → 报告」串起来——**判决书（kernel）与卷子格式（xtask）都有，考试却没有组织者**；而两个接缝其实都已具备
（`nai-provider-core` 的 `TextGenerator`，W36 起真实模型已通；`nai-storage` 的 `retrieve_local`/`retrieve_session_knowledge`）。
ADR 0033 的后果段把「真实题集内容、真实模型评估与真实检索质量」一并划给外部——对①④正确，对③把**编排代码**也说成外部的，
代价是真实模型已经可用却没有任何一次运行能产生 B01 的结论。

本轮落 **题集修订 2**：每题**必填** `expected`（`{kind:"reference", must_include:[…], must_exclude:[…]}`，`must_include` **至少一条**——
没有任何要求的参考答案会让判定在「自动正确」与「永远未判定」之间二选一，两者都等于**把判定关掉**）。判定是 kernel **纯函数**
`judge_answer`（归一化 = Unicode 小写折叠 + 连续空白折叠；**否定式优先**——任一 `must_exclude` 命中即 `Incorrect`，否则
`must_include` 全中才 `Correct`；**空判据返 `Unknown` 而非 `Correct`**）；取**子串**而非相似度阈值，因为阈值需要一个数字，
**那个数字本身就成了判据**且不可复现。判定产物是**既有的** `AnswerVerdict`，故 `Submission`/`evaluate`/`QualityReport`
**一个字段都没改**，`Unknown` 从分母排除的语义原样继承；放 kernel 而非放执行体，是因为它是**口径**不是编排（放执行体就失去
portable 门控覆盖）。**修订 1 不做双读**（旧题集被拒而非读成「无参考答案要求」，静默改变用例含义——与 ADR 0033 拒绝未知字段同一理由），
夹具 `tests/fixtures/rag-golden/example-min.json` 同步升级。

新增**评估执行体** `xtask/examples/golden_eval.rs`，对照既有先例 `crates/nai-provider-core/examples/live_provider_smoke.rs`
（真实链路一律放 `examples/`，不进测试、不进工程门控）：它复用门禁**自己的**读取器（`read_case_set`/`case_set_files` 与审计共用
`parse_case_set`，故「这里读得进」与「门禁放行」**永远是同一件事**），经真实 `TextGenerator` 逐题生成并用
`GoldenCase::answer_verdict` 判定。**缺前置拒绝桩替代**：目录无 `*.json`/结构不合规/缺环境变量 ⇒ `CAPABILITY_NOT_READY` 且
**不发起任何调用**（凭据只报**变量名**，值从不打印、从不写进证据），真实调用失败退出 2（与「前置未就绪」区分）。
**它只驱动生成侧**：检索召回、检索 P95 与行为类判据（澄清/转义、无副作用、取消）具名打印为 **未驱动且不填零**，并明示
「**不是 B01 达标**」——**否决**「用答案文本猜 `outcome` 再喂 `evaluate`」：那会造出字段齐全但含义失真的报告，行为类 token
**假失败**（把「执行体没驱动」读成「系统差」），`MustNotAct` **假通过**（本路径没有副作用通道故恒为空），**两种误导都比缺字段严重**。

实测：夹具 8 例 → **8/8 judged / 0 correct**（占位问题与夹具参考答案不匹配，正确率**无意义**，只证链路通）；临时探针 1 例 →
**1/1 correct**（1000‰，证明判定**双向往返**可用，不是恒 `Incorrect`）；默认目录仍 `CAPABILITY_NOT_READY`（`files: 0`）。
`check-deps` **仍 18 包**方向不变（真实接线仅 `[dev-dependencies]`，example 不进 lib ⇒ portable 覆盖与耗时不变）；
kernel `tests/golden.rs` **31 项**、xtask lib **39 项**全绿；portable **EXIT=0**（**82 个测试目标**、**694 passed / 0 failed**，
较 W39 +11，目标数不变）；`check --secrets` **702 文件零命中**；`check --registry` 通过；两处 `tsc` EXIT=0；desktop 5/5、`test:ui` 15/15。**缺口从四段降到两段**（①100 题内容、④真实知识库内容，均外部投入），**但两段的充分性不同**：①到位即让门禁放行并出生成侧 `answer_accuracy`（零代码改动）；④到位**本身不产生**检索侧结论——未驱动项是**常量**（`golden_eval.rs` 的 `UNDRIVEN`，无条件打印），Recall@5／检索 P95／行为类判据要出数**仍需扩展执行体或另接采样器**；registry B01 **保持 `blocked`**（达标仍不可能达成，「执行体就位」不是达标）。
见 docs/golden-set.md、tests/rag-golden/README.md、docs/adr/0040-golden-eval-runner-boundary.md 与
docs/reports/2026-10-11-w40-golden-eval-runner.md。未做边界：**行为侧执行**（守门/副作用/取消的观测接入）与**真实检索接入**未做，
执行体目前只在生成侧产出结论。

W41 完整工程 profile（前端门控）已实现/实测（ADR 0041）：`engineering-foundation` 规格从 D01 起就要求
「workspace 编译、测试和**前端检查**可执行」，但任务工具里**没有这个入口**——`check --profile portable` 只做六件事
（`check-deps`/`check --registry`/`fmt`/`clippy`/全 workspace 测试/harness 库测试），前端一行都没有。前端检查**一直存在**
（`@nai/contracts` 与 `@nai/desktop` 各自的 `typecheck`/`build`、桌面包 `test`、根 `test:ui`），但**从未被门控覆盖**：
历次记录都把它们与 portable **并列**写成人工结果，并列本身就说明「有人记得才有一次」。故缺口的准确性质不是
「前端没有检查」，而是「前端检查**没有门控入口**」——portable 跑绿不能排除前端已破。本轮落
**`cargo xtask check --profile full` = portable 全部步骤 + 前端门控**（`portable` 语义与历史口径**不变**，它的名字承诺
「只要 Rust 工具链就能跑」）；`xtask/src/lib.rs` 把六步抽成 `engineering_checks` 供两种 profile 共用，
「full 包含 portable 全部步骤」因此是**代码结构事实**而非需同步的清单。`xtask/src/frontend.rs` 只决定**何时跑**
（`NODE` 覆盖、缺省 `node`）与**失败怎么报**；`scripts/frontend-gate.mjs` 是「前端怎么跑」的单一真源——命令取自各包
`package.json` 的 `scripts`，只把可执行名解析成真实入口（已知仅 `node`/`tsc`/`vite`），且一律经**当前 node** 运行其 JS 入口
（`typescript/lib/tsc.js`、`vite/bin/vite.js`），因为 npm 的 `.bin` 垫片是无扩展名的 POSIX 脚本、Windows 上不可执行；
不认识的可执行与 `|`/`>`/`||` 等未实现的 shell 语法一律**具名拒绝**，glob 由脚本自行展开且**空匹配即失败**
（「0 个测试文件被跑到」不得读成通过）。缺前置三条硬规则：无 `node` ⇒ `FRONTEND_TOOLCHAIN_MISSING`、
依赖不可解析 ⇒ `FRONTEND_DEPS_MISSING`、某步非零 ⇒ `FRONTEND_STEP_FAILED`（指名包与 script），**绝不静默跳过**。
实测：`check --profile full` **EXIT=0**（2m59s），前端六步——两包 typecheck、两包 build（`vite v7.1.9` 30 modules / 3.32s）、
desktop `1..5`、`test:ui` `1..15`；Rust 侧 **82 个测试目标**（65 `Running` + 17 `Doc-tests`）、**695 passed / 0 failed**
（W40 基线 694，+1 即 `frontend.rs` 新单测）。**三条非空洞自检**（均随后还原、`git diff` 无残留）：注入类型错误 ⇒
`FRONTEND_STEP_FAILED` 且后续步骤停止；注入 `eslint .` ⇒ `FRONTEND_SCRIPT_UNSUPPORTED`；注入 `tsc --noEmit | cat` ⇒
同标记并指名 token。**主机关联事实**（不是仓库内容）：本机 `apps/desktop/node_modules/{vite,typescript,react}` 是
Windows 不可解析的符号链接（`realpathSync` 报 `EACCES`），前端依赖因此退化到根层手工副本，而后者的
`@rollup/rollup-win32-x64-msvc` 与 `@esbuild/win32-x64` 缺失，已按既有做法从 `.pnpm` 补齐；重装后可能需重补。
registry **不改任何 `status`**（本轮是覆盖面扩展不是达标），D01 notes 只补记事实。见
docs/adr/0041-engineering-profile-frontend-boundary.md、docs/reports/2026-10-11-w41-frontend-gate-profile.md。
未做边界：**不测渲染**（仍不引入 DOM 库）、**不代跑 `install`**；
前端门控通过只说明工程基础可执行，MUST NOT 读成浏览器/人工 UI 或任何 `O` 系列验收达标。

W42 把完整工程 profile 接进 CI（上一段的「另行决定」，本轮落定）：W41 建的 `check --profile full` 在流水线里**从未被调用**——
`foundation.yml` 只有 `portable`（Linux）/`windows-local`（Windows）两条分支，外加两平台各一次的根 `pnpm typecheck`
（`pnpm --recursive typecheck`）与 `pnpm build`（`pnpm --recursive build`）；于是 `apps/desktop` 的 `test`（5 项）与根 `test:ui`
（15 项）**在 CI 中从不运行**，流水线跑绿不能排除这 20 项已回归（与 W41 的缺口同构：门控已建、流水线没接线）。本轮把两个
job **统一为 `check --profile full`**，并**删除**被其逐包完全覆盖的两步根命令（保留它们等于让「前端怎么检查」在 CI 里出现
**第二处定义**，与「命令取自各包 `scripts`」的既有规格相抵）；不新增 job、不改 matrix/timeout，8 个 `test --suite`、`schema --check`
与 openspec 校验步骤保留。`windows-local` profile 在**工具中保留**（本机与未来用途），CI 不再用它——在 Windows runner 上
`full` 是它的**严格超集**（同一套 `engineering_checks` + 前端步骤），平台要求由 `runs-on` 天然满足。规格新增两条需求：
CI SHALL 执行完整工程 profile；CI MUST NOT 在 profile 之外另定义前端命令（单一真源）。
验证：用 Node **解析 YAML** 做结构断言 **13/13 通过**（含 `full` 恰 1 步、不含 `portable`/`windows-local` 与两步根命令、
保留 8 个 suite、矩阵与 timeout 未变）；本机 `check --profile full` **EXIT=0**（82 目标 / 695 passed / 0 failed，与 W41 一致，
本轮**零源码改动**）；`check-deps` 18 包、`--registry` 通过、`--secrets` **714 文件零命中**、`validate --all --strict` **37 项**。
**观测（原写「未经观测」，同日复核订正）**：`F31/nai` 是 **public** 仓库，Actions 结果可**匿名**经 `api.github.com` 读取
（本机沙箱走 HTTP 代理）——**不需要 runner、不需要 `act`、不需要凭据，也不需要手动「创建执行」**：workflow 有 `on: push`，
每次 push 都已自动触发。实测本次 `21163d0` 的 run `38099556545` **两个 job 全部 `success`**，含第 10 步
`Complete engineering foundation (portable steps + frontend workspace)`（即 `check --profile full`，含前端门控）在
ubuntu-24.04 与 windows-latest 上均通过 ⇒ 本轮 CI 改动**已被流水线实证**（强于「结构断言 + 本机通过」）。
**但同一次核对暴露了历轮从未记录的仓库事实**：CI 在 ubuntu 上**长期间歇失败**——近 30 次运行中 15 次 `failure`，
其中 10-10 之后的失败**全部**是 ubuntu 的 `Portable foundation` 单点失败，而**同一 run 的 windows 侧通过**；
同一份代码在 `ba1c33b` 失败、其上仅改文档的 `6f3941e`/`21163d0` 成功 ⇒ **非确定性**。失败详情需 job 日志，而该端点匿名
返回 403 `Must have admin rights`，annotations 只给 `exit code 1`（无测试名）；本机非 Linux（`wsl.exe` 被安全策略禁止、无 Docker）
⇒ **定位需 admin 权限下的日志或另设观测通道**（如让 CI 失败时把摘要写成 annotation）。registry **不改任何 `status`**，
D01 notes 只补记事实。见 docs/reports/2026-10-11-w42-ci-complete-profile.md 的**勘误**段。

W43 让 CI 失败**在无凭据下可诊断**：W42 勘误段确立了「注释位可读、内容是空的」——
`GET /check-runs/{id}/annotations` 匿名可读却只有 `Process completed with exit code 1.`，而真正的日志端点
`GET /actions/jobs/{id}/logs` 需要 admin（403）⇒ 那 13 次 ubuntu `Portable foundation` 间歇失败只能定位到**步级**、
定位不到**测试级**。GitHub 会把 workflow command（`::error::…`）解析成 check-run annotation，而该 API 匿名可读
⇒ 把门控日志的失败摘要转成 annotation 即可补上这一段。门控步改 `shell: bash` + **`set -o pipefail`** +
`tee "$RUNNER_TEMP/nai-gate.log"`，其后加 `if: failure()` + `continue-on-error: true` 的诊断步跑
`scripts/ci-diagnose.mjs`（**失败信号行优先 + 尾部兜底**，逐行输出、剥离 ANSI、转义 `%`/`\r`/`\n`、
双层截断与省略提示；**日志缺失/为空一律退出 0 且不输出任何注释**）。
**`set -o pipefail` 是必需的而非风格问题**：`cmd | tee f` 的退出码恒为 `tee` 的 0——实测同一条命令有它时失败路径
`EXIT=1`、无它时 `EXIT=0`，**少了这一行 CI 会永远绿且看上去一切正常**。
端到端实测（**两个方向都做**）：正常路径 run `38108824691` 两平台 `Complete[success]` + `Publish[skipped]`
（成功时诊断步跳过，**不制造注释**）；失败路径在临时分支把命令换成 `check --profile bogus`，run `38108847930`
两平台 `Complete[failure]` + `Publish[success]`，**匿名**读到
`xtask: unsupported or not implemented task: check --profile bogus (use cargo xtask help)`
（Windows 上还多抓到 cargo 的 `error: process didn't exit successfully …`）⇒ **通道成立，且两平台都成立**；
两平台门控步都是 `failure` 也**反证** `pipefail` 在 Windows 的 Git Bash 下生效。probe 分支已删除。
**代价**：仓库永久留下一个失败 run 记录（GitHub 不支持删 run 历史，已在 proposal 事先声明）。
本轮只解决「查不到原因」，**未解决**那个间歇失败本身；变化是下次它发生时失败测试名会自动出现在匿名可读的注释里。
规格新增三条需求（摘要 MUST 匿名可读 / 采集 MUST NOT 改变判定 / 无内容 MUST NOT 制造注释）——
不写进规格，这条通道会被当冗余删掉。registry **不改任何 `status`**。见
docs/reports/2026-10-11-w43-ci-failure-annotation.md。

W44 让这条通道**第一次真实使用**，并修复了它暴露的三类信噪比缺陷。W43 的勘误提交 `81ce88b` 触发 ubuntu 失败
（run `38110014189`，第 10 步失败、第 11 步诊断步成功），**匿名**读到 check-run `114383450000` 的 12 条注解，
**首次拿到测试级结论**：`test halt_during_media_drill_prewarm_never_commits_a_frame ... FAILED`——即 W42 登记、
历轮只能定位到**步级**的那个间歇失败。但 11 条里只有 1 条是信号，三类成因各有明确来源：
①`Checking thiserror`/`Compiling thiserror-impl` 因**裸 `error` 子串**被判成强信号；
②一个**通过**的测试（`…cors_queries_unimplemented_routes… ... ok`）因名字含 `unimplemented` 被判成强信号，
**并把它的上下文拖进摘要**；③信号稀少时被无关行填满配额。
修法**不是收窄关键词**（那只会把一种漏判换成另一种），而是把判据从「这行含不含某个词」改为**「这行报告了失败吗」**：
新增噪音类 `test <name> ... ok|ignored`、`error` 改为 `\berror(\[|:)`、选行改为「信号 → 后置上下文与所属目标头 → 弱信号」，
且**一条信号都没有时**才退化到尾部窗口（**绝不**用无关行填充）。
另新增**截断自证**：本次采集到的日志**只有 11 行**，而该 job 跑了 **2 分 50 秒**，且**没有** `test result:` 汇总行
（判断依据：省略提示只在 `omitted>0` 时输出，而 `lines.length===11` 时不会出现）⇒ **一份被截断的日志正在被当成全部证据**。
现在「有逐测试行却无汇总行」会输出一条 `::warning::` 并给出总行数。**截断成因未定，如实登记、不猜**。
**收尾观测**（推送后该 run 在 ubuntu 上**再次失败**，run `38111087579`）：新脚本的**第一次真实运行**产出
**11 条注解全部是信号**（对比修复前的「1 真 + 10 噪」），**且没有截断警示**——本次日志确实含 `test result:` 汇总行
⇒ 同一个判据在两种真实形态上给出了相反且**各自正确**的结论。更重要的是**拿到了精确到行的根因**：
`thread '…' (19037) panicked at apps/engine/tests/engine.rs:446:5`，断言原文
`a halt before the worker commits must not log a committed frame: [DecisionEntryView { seq: 1, …, "event":"armed" }, …]`
⇒ 失败点是**第二个**断言（halt 后决策日志不得再出现 `media_drill_committed`），**不是** `halt_and_wait` 的 15 s 上限、
**不是 hang**；机制是 `Engine::start` 返回后测试才写 halt 文件，而 supervision 线程此时**可能已把 drill 跑完并提交了帧**
⇒ **halt 与 drill 的时序竞态**（CI 调度差异使其时快时慢）。**修法未做**（方向是给测试一个确定性的 prewarm 阻塞点，
而不是放宽断言——断言本身是对的）。
**同时撤回一条历轮沿用的旁证**：失败测试带 `#[cfg(target_os = "linux")]`（engine 的 6 个 `#[test]` 里 **4 个**如此，
Windows 上该 target 只跑 2 个测试）⇒「本机 Windows 跑 portable/full 从未复现」是**结构性必然**，**不是**非确定性证据
（W42/W43 报告的「非确定性」判定用的是「同一份代码时红时绿」，**不受影响**）。
**当前实测**（匿名 `GET /actions/runs?per_page=100` 后取最近 30 次）：**13 `failure` / 17 `success`**——
与 W42 时点的 15 不同，是**窗口滑动**所致（期间新增了 W43 为验证通道人为制造的那 1 次 probe 失败），
故 `AGENTS.md` 上文 W42 段的「15 次」按**当时时点**理解，不回改。
规格新增两条需求（摘要以信号为中心 / 采集不完整 MUST 明示），**workflow 未改**、registry **不改任何 `status`**。见
docs/reports/2026-10-11-w44-ci-diagnose-signal-to-noise.md。
