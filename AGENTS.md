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
check --profile portable/windows-local、schema [--check]、test --suite contracts/storage/kernel/hosts/session/scheduler/provider/knowledge
及前端 typecheck/build 可执行。契约工具要求 protoc 3.21.12，完整 C++ suite 另需同版
libprotobuf；详见 docs/contracts/README.md、docs/hosts.md、docs/session.md、docs/scheduler.md、docs/provider.md、docs/knowledge.md 与 docs/kernel.md。
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
