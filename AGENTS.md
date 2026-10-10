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
check --profile portable/windows-local、schema [--check]、test --suite contracts/storage/kernel/hosts/session/scheduler/provider/knowledge/upgrade
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
