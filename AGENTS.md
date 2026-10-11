# nai development instructions

- 项目英文名为 `nai`，仓库为 https://github.com/F31/nai。命名映射见
  `docs/adr/0001-project-name-and-design-baseline.md`。
- `docs/design.md` 指向唯一实现基线
  `docs/直播数字人 + 智能体系统技术方案 V3.9.md`（ADR 0048 升基线；ADR 0047、ADR 0042 与 ADR 0015 的裁决继续有效）。历史版本（V3.7/V3.6/V3.5 等）只供参考；
  不自行拼接旧条款覆盖 V3.9。规范变更记录 ADR。V3.9 的 dh-* 命名不采纳，工程标识一律 nai-*。
- 验收唯一来源为 `tests/registry.yaml`（V3.9 S/R/A/B/O/P/M/H/X + release_line（新增 capability_pack），D→新编号映射自建）。
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

D15 验收注册表与基线切换已实现/实测：ADR 0015 把 V3.6 升为唯一实现基线（勘误清单生效，dh-* 命名不采纳；V3.6 后由 ADR 0042 升为 V3.7、V3.7 后由 ADR 0047 升为 V3.8、V3.8 后由 ADR 0048 升为 V3.9），
tests/registry.yaml 为验收唯一来源（S/R/A/B/O/P/M/H/X + release_line，D01–D14 映射自建并回填）。
`cargo xtask check --registry` 与 `cargo xtask check --profile portable` 可执行；17 个主规格、15 项归档校验通过。
决策日志 decision_log（R07 前置）与值守租约（600ms 短期限）为下一轮；播放 ACK 段保持 Draft，不冒充完整 R07。
详见 docs/adr/0015-v36-baseline-and-admission-registry.md、docs/reports/2026-10-08-d15-admission-registry.md 与 tests/registry.yaml。

W02/W03 决策日志与值守租约已实现/实测（按 V3.9 §31 工作项推进，不再新增 D 编号）：
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
把 V3.9 §14.3 的九步升级流程落成可执行编排，registry R05/R09/R10 由 pending 升 implemented（**不标 tested**：
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
**不新增 B01a/B01b 编号**——registry 的 id 正则 `^(S|R|A|B|O|P|M|X)[0-9]{2}$` 要求恰好两位数字，拆分会直接使 `check --registry` 失败，且等于
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
**同日再跑**（`064d754`，**只改文档**）：结论为 **ubuntu `success` / windows `failure`** ⇒ 同一份 Rust 代码在 ubuntu 上
时红时绿（`ca47168` 红、`064d754` 绿，**两者只差文档**），非确定性再添一例；**windows 侧失败是 10-10 以来首次**，
注解同样全是信号，落在 `crates/nai-runtime` 的 4 个测试上（`StorageError { DeadlineExceeded, "operation stopped" }` /
`{ PermissionDenied, "scope permission missing" }`）⇒ 与 ubuntu 侧**不同测试、不同根因，但同属「并行 + 共享资源 + 慢主机」
下的时序脆弱**，非平台专有缺陷。三次运行的摘要里都**没有**「（另有 N 行未展示）」，故「采集到的日志只有约 10-11 行」
与「该条注解被服务端丢弃」**两种解释尚未区分**，如实登记、不武断。
**同时撤回一条历轮沿用的旁证**：失败测试带 `#[cfg(target_os = "linux")]`（engine 的 6 个 `#[test]` 里 **4 个**如此，
Windows 上该 target 只跑 2 个测试）⇒「本机 Windows 跑 portable/full 从未复现」是**结构性必然**，**不是**非确定性证据
（W42/W43 报告的「非确定性」判定用的是「同一份代码时红时绿」，**不受影响**）。
**当前实测**（匿名 `GET /actions/runs?per_page=100` 后取最近 30 次）：**13 `failure` / 17 `success`**——
与 W42 时点的 15 不同，是**窗口滑动**所致（期间新增了 W43 为验证通道人为制造的那 1 次 probe 失败），
故 `AGENTS.md` 上文 W42 段的「15 次」按**当时时点**理解，不回改。
规格新增两条需求（摘要以信号为中心 / 采集不完整 MUST 明示），**workflow 未改**、registry **不改任何 `status`**。见
docs/reports/2026-10-11-w44-ci-diagnose-signal-to-noise.md。

W46 把注册表补全到「全部验收家族在册」：基线 §16 与汇总索引 §39.5 把开放平台与技能市场的 **M01–M18** 列为
「唯一验收编号体系」的一部分（§37.5/§38.10 给出逐条用例；§1.4 明写 R1 含 `M01–M07、M13–M16、M18`、R2 含 `M08–M12、M17`），
而 `tests/registry.yaml` 里 **M 一条都没有**（`grep -cE '\{id: "M[0-9]'` = 0）。这不是「未开始」的表达而是**登记缺失**——
`pending` 才是在册未开始。缺口的后果是「以注册表为准」这条规则会把 V3.9 第三部分（§33–§38、W26–W40）从验收视野里抹掉；
它长期不可见是因为 **`check --registry` 只校验已登记条目的合法性，不校验家族是否齐全**。
本轮：登记 **M01–M18 共 18 条 `status: pending`**（title/release_line 取自两张表；`automation` 是**规划意图**并在 notes 写明，
`contract` 仅在 §37.4 命名了契约时填，M10/M12 留空）；**扩 id 家族白名单到 `M`**
（`scripts/contracts/registry.mjs` 的 `ID_RE` 与 `schemas/tests/registry.schema.json` 两处 pattern ⇒ `^(S|R|A|B|O|P|M|X)[0-9]{2}$`，
按 §39.5 顺序；`B01a` 这类拆分仍被拒）；`tests/registry.test.mjs` 加**家族守卫**（M01–M18 必须全部在册，条目下限 44→66）。
家族分布 **S12/R11/A11/B6/O5/P1/M18/X2 = 66**；既有 48 条 status **一字未动**（implemented 27 / tested 10 / blocked 5 / not_applicable 3），
`pending` 由 3 升 21。**守卫做过两步自检**：删 M18 时测试失败但落在计数断言（65<66，被短路）；把 M18 改名 M19（计数仍 66）
才真触发家族守卫 `baseline acceptance M18 is not registered` ⇒ **一次自检不够**，第二次才是有效证据。
顺带订正活文档沿用的家族简写——`AGENTS.md`（含正则那处）、`docs/design.md`、`docs/development-plan.md`、`openspec/config.yaml`、
`README.md` 都写「S/A/R/O/X」，而 **B 与 P 早已有条目**（B01–B06、P01），简写本身就不准 ⇒ 统一改为 `S/R/A/B/O/P/M/X`。
规格（admission-registry）新增一条需求：注册表 SHALL 覆盖基线定义的全部验收家族，未开始 MUST 以 `pending` 在册、不得省略。
**不做**：不实现 M01–M18 任何一条（W26–W40 未动工）；不把 M 并入 `check --profile portable`（未实现不应让工程门控变红）；
不写通用的设计文档解析器（只对触发缺口的 M 家族落确定守卫）。见 docs/reports/2026-10-11-w46-register-m-series.md。

W47 消除「SBOM」这个**无定义空位**。基线三处要求它（§11.6「每次发布附 SBOM」、§29.5 发布目录含「SBOM 与许可」、
§11.3 T-08 列为供应链控制），仓库实现为零，S08 notes 自记「SBOM 与真实签名链仍待建」。真正的问题不是缺文件而是
**没有定义**：不说谁产、什么格式、覆盖什么 ⇒「每次发布附 SBOM」不可判真伪，任何清单都能自称 SBOM。
先立规格（新能力 `supply-chain-sbom`，6 条需求）再实现，裁决见 **ADR 0043**。
**工具链选型以实测为据**：`cargo metadata --locked` 已提供 299 包的 name/version/license/source/repository
（299/299 带 license），只有 `checksum` 拿不到（当前 cargo 的 metadata 输出与 `cargo_metadata 0.23.1` 的 `Package`
都没有该字段）；`Cargo.lock` 有 281 条 `checksum`，**恰等于 299 − 18 个 workspace 成员**。而 `xtask` 本来就依赖
`cargo_metadata`/`serde_json`（`check-deps` 正用前者）⇒ **自建生成器零新增依赖**。故**不装**任何新工具：
`cargo-cyclonedx`/`cargo-sbom` 需联网 `cargo install`（为审计供应链先引入一个自己进入供应链的工具），
`syft` 是需下载的非 Rust 二进制（违反 ADR 0041 的「portable 只需 Rust 工具链」）。
交付 `cargo xtask sbom [--out <path>]`（`xtask/src/sbom.rs`，11 项单测）：输出 **CycloneDX 1.5 JSON**，
实测 299 组件、175,111 字节、`id 95 / expression 194 / name 10 / no-license 0`、281 带 SHA-256。
**三条取舍**：①**确定性优先于时间戳**——省略可选的 `timestamp`/`serialNumber` 使「同一锁文件 ⇒ 逐字节相同」成立
（两份 SBOM 的 diff 即等价于依赖闭包真变了），发布期元数据注入不在本轮范围；②**许可三态、绝不静默改写**——
工作区 4 种旧式斜杠写法（`Apache-2.0 / MIT` 等，共 10 个包）**不是合法 SPDX 表达式**（`/` 不是运算符），
塞进 `expression` 会产出「格式不合法却看似正常」的数据、改写成 `OR` 则替上游做许可解释，故原样记入 `license.name`
（CycloneDX 里 `id`/`name` 是两个字段，「是不是 SPDX 标识」天然可见）；「已知 id」是**显式表而非形状匹配**
（`BSD-2-Clause-FreeBSD` 已被 SPDX 废弃 ⇒ 断成 `id` 可能说错，记成 `name` 只是少说），代价是表外合法 id 会降级为 name
——所以生成时把四类计数打到 stderr 让偏移**可见**；③**范围声明写进产物自身**（`metadata.properties`
的 `nai:sbom:scope|excluded|not-evidence-for`），因为用仓库内文档约束一份会外发的产物等于没有约束。
**验证**：与两个独立来源逐项交叉核对——组件 299 = 锁文件 `[[package]]` 299；哈希**逐个**比对 **281/281 一致、0 不符**；
许可**逐条**比对 **299/299 原样、0 改写**；斜杠包数 10 = verbatim-name 计数；`purl` 唯一。上述两项是用**独立的 Python 实现**
重新解析比对的，不是复用被测代码的结论。单测 40 → **51 passed**；反向自检：缺 `checksum` 的 registry 包与畸形校验和
都被**具名拒绝**。**解析器有一个 bug 是靠测试失败才发现的**——最初在匹配键**之前**就解引号，于是 `dependencies = [`
被当作「不是带引号的字符串」而报错；修法是**先匹配键、再解引号**。
**不做**：不改任何验收 `status`（S08 仍 `implemented`——真实签名链仍缺、漏洞与许可**政策**审计仍缺，那需要 advisory
数据源与允许/拒绝清单，是**政策**而非**记录**）；不并入 `engineering_checks`（生成是产出命令会写文件，非只读检查，
portable 语义按 ADR 0041 不变；但**其单测在门控内**，故不是幽灵工具）；不覆盖前端 npm 依赖/原生二进制/模型素材/插件包 BOM；
不做发布期元数据注入；不做外部 Schema 校验（**只声称是 CycloneDX 1.5 文档，不声称通过完整 schema 校验**）。
见 docs/reports/2026-10-11-w47-sbom-supply-chain.md、docs/adr/0043-sbom-generation-and-scope.md。

W48 修掉 W44 定位到断言行的那条 **ubuntu 间歇失败**（`halt_during_media_drill_prewarm_never_commits_a_frame`）。
**先把定性说清：engine 的拆除路径本来就是完整的**（`DrillGuard::tick` 在每个轮询点检查 graceful halt 与
epoch 撤销，且所有 tick/wait 都在提交之前），所以问题**不在**「engine 不会中止」，而在**测试没能保证 halt 真的先到**——
`Engine::start` 在 engine 打印控制地址后即返回、测试**随后才写 halt 文件**，而 supervision 线程此时**可能已把
整个 media drill 跑完并提交了帧** ⇒ 那条「halt 先于提交 ⇒ 不得记录提交」里的**先后顺序是靠墙钟赌出来的**，
CI 调度差异使它时红时绿（同一份 Rust 代码 `ca47168` 红、只改文档的 `064d754` 绿）。
故 **不放宽断言**（断言与决策链校验一字不改），而是**让前提成立**：新增 Linux 专用夹具 `park_asset_on_fifo`
（把**首个 avatar 素材**换成**无写端命名管道**——render-worker 在 prewarm 按序读素材、`File::open` 即阻塞，
于是它**不可能**走到提交），并在写 halt 前 `wait_for_worker_pid` 确认 **worker 确已进入 prewarm**。
**为何必须等 worker 出现**：否则 halt 可能落在更早的 `await_grant` keeper 握手里，drill 根本到不了 prewarm，
worker 不被派生、提交无从发生——测试会**假通过**，不再在测它名字里的那件事。
**为何阻塞点不是恒过**：同夹具（不带管道）在 `media_drill_produces_a_committed_frame_through_the_real_worker`
里**确实提交过帧**，它移除的是时序赌博而非提交能力；且若 halt 路径失效，drill 会卡在 prewarm 直到**自身 15s 期限**，
`halt_and_wait`（同样 15s）便无法干净通过。**记下一个边界**：`--pcm` **不能**当阻塞点——engine 的
`pcm_samples` 取 `metadata().len()`，管道大小为 0，会在派生 worker **之前**判 `PCM must hold whole f32 samples`，
那是另一条路径而非 prewarm；素材则可以，因为 engine 侧**不读素材内容**（只作为 `--asset` 转发给 worker）。
不引入 `libc`：`mkfifo` 走 `Command::new("mkfifo")`，与同文件既有的 `Command::new("kill")` 同构。
**验证与边界（如实披露）**：`check-deps` 18 包、`check --registry`、`check --profile portable`
（**82 目标 = 65 `Running` + 17 `Doc-tests`、706 passed / 0 failed**；706 较 W42 的 695 **恰 +11 = W47 的 sbom 单测**，
**本轮零新增测试**）、`validate --all --strict` 38/38 均通过——但**被修测试与夹具都是 `#[cfg(target_os = "linux")]`，
Windows 上不编译**，本机 `tests\engine.rs` 目标只跑 2 个跨平台测试，故**本机门控不能证明修法成立**，
**以 CI 的 ubuntu job 结论为准**（匿名读 `/actions/runs` 与 `/check-runs/{id}/annotations`）。
新增规格需求「断言因果顺序的演练 SHALL 由构造保证该顺序、MUST NOT 依赖墙钟时序」。
**不做**：不改 `src/`/`crates/`、不改监督语义与 `AVATAR_TIMEOUT`、不改任何验收 `status`、
不修 Windows 侧 `nai-runtime` 的时序脆弱（另属一类）、截断成因仍未定。
见 docs/reports/2026-10-11-w48-deterministic-prewarm-halt-drill.md。

W49 收尾 O01 与 O05 中「外部依赖最少」的一半，**两者 `status` 均保持 `pending`**（W38 的 O02 先例：
逻辑/界面出现**不等于**验收达标）。**O01** 的缺项原是「演示包未建」（`preset`/`演示包` 两词在 `crates/`、`apps/`、
`schemas/` 里此前一次都没出现过）：新增 `crates/nai-kernel/src/demo.rs`（与 O03 的 `monitoring.rs`、O04 的
`device.rs` 同层同形）把 §615 演示包五要素与 §612 两条时间目标落成 `audit_demo_package`（**十六条**稳定标记）
+ `DemoReadiness` 三态；四条口径是本轮要定死的：**未声明即条件化**（空描述符审计为空、读作 `Blocked`——
尚未交付不是违规；声明了任一部分则其余成为**欠项**）、**「授权的」是要求而非形容词**、
**连通不等于达标**（常量桩与把冒烟当验收各报一条）、**时间戳早于安装就绪是不自洽**而非「跑得快」。
`tests/demo.rs` **19 项**，含「一组构造描述的审计并集**恰为**这十六条」断言（用一组而非一份：`missing` 与
`defective` 对同一部分互斥）。**O05** 的缺项原是「成本显示未建」：新增 `apps/desktop/src/upgrade.ts`
（零 DOM，与 W38 的 `console.ts` 同形）——未知分项呈现为**原因**（绝不呈现为 0）、部分和呈现为**下界+未知计数**
并明说不是总额、载荷严格解析（十条具名拒绝）；`tests/ui/upgrade-cost.test.mjs` **19 项**。
**验证**：`check-deps` **18 包**、`check --registry`、`check --profile portable`（**83 目标 = 66 `Running` + 17
`Doc-tests`、725 passed / 0 failed**；706 → 725 **恰 +19 = 本轮 demo 单测**，新增 `tests/demo.rs` 目标使 `Running`
65 → 66）、`pnpm test:ui` **34/34**、`frontend-gate.mjs` 六步全通过、`validate --all --strict` 38/38。
**与 W48 相反的平台边界**：本轮的 kernel 判定与前端呈现层**没有任何 `#[cfg(target_os)]`**，portable 全程可测，
**本机门控即可证其成立**。新增规格能力 **`demo-package`（9 条需求）**；`local-control-surface` +1 条需求；
**ADR 0044**。**不做**：不建真实演示包内容、不跑人工演练、不接控制面升级状态来源（**O05 的呈现层因此未接入
任何界面**——与 §30.1 第 4 条「升级与回滚提示」**同一缺失前置**）、不加 xtask 命令与约定外证据目录、
不改任何既有验收 `status`。见 docs/reports/2026-10-11-w49-o01-o05-in-repo-closeout.md、
docs/demo-package.md、docs/adr/0044-demo-package-readiness-boundary.md。

W50 把使用者提供的外部语料 **LiveChat**（ACL 2023／**MIT**，实测 **2,411,678** 条真实中文直播对话、346 主播，
结构 `[streamer_id, audience_comment, streamer_response]`）接入为 B01「贴近真实直播的问题」（§284）的**取材素材**。
起点是一个**口径缺口**：设计规格 §5.9 只列举三条**题集**来源（按场景编写／随场景包发布／坏案例经具名确认回流），
**未覆盖「外部语料取材」**——故该语料不作第四条来源，而被定义为**素材层**：供问题**措辞与语境**，
**不**供产品知识库与每题的 `evidence`/`expected`，因而**不使 B01 达标**（**保持 `blocked`**）。
**安全摄取**（`.pk` 是 pickle，反序列化即任意代码执行）：核验**不先 `pickle.load`**，先用 `pickletools.genops`
枚举 opcode 并对**数据白名单**放行（`GLOBAL`/`STACK_GLOBAL`/`REDUCE`/`NEWOBJ`/`BUILD`/`EXT*`/`PERSID` 一律拒绝），
**8 个 `.pk` 全通过**（自检非空洞：含 `os.system` 的 pickle 被具名拒绝）；解压**先校验成员路径**防 zip-slip。
**候选提取**：从 1,440,191 条对话提取 **154,812 条**提问候选（去重去噪、带溯源与主播真实回应），候选**不进仓库**、
仍须**具名人工确认**才入 `tests/rag-golden`（复用既有 `promoted`+`confirmer` 机制）。**一次被实测推翻的尝试**：
初版按词映射 §5.9 类别（`连麦/上麦/管理`→越权、`给你`→注入），抽样显示 `injection` 档几乎全是打赏互动、
`privilege_escalation` 96 条里真正求权限的不到 10 条 ⇒ **整段移除**，工具只输出**原始信号词**；结论是该语料
**可靠覆盖的只有「问题措辞」**，对非 `normal_answer` 类别**基本不适用**。抽样验真 60 条 ≈ **78%**（如实披露）。
**可达性**：Google 全不可达（代理 502／直连 000）、HF/ModelScope 无镜像、上游无 tag/release ⇒ 全量只能人下载。
**零仓库代码改动**（数据在仓库外、读 pickle 需 Python）、零新依赖、零新命令与证据目录；语料与候选池**不入库**。
新增证据 `docs/reports/evidence/livechat-corpus-provenance.md`；规格 `knowledge-quality-gate` **+1 条需求**
（外部语料只作素材／候选须具名确认／未使门槛满足／先验后读／来源可追溯）；`docs/golden-set.md`、
`tests/rag-golden/README.md`、B01 notes 同步。遗留：§④ 真实知识库与 100 题内容仍是外部投入。
见 docs/reports/2026-10-11-w50-livechat-corpus-sourcing.md。

W51 补齐 **O02** 的「审批按钮显示最终文本」（§662／§425）在**逻辑面**的缺口。W09 交付审批 API、W38 交付桌面
审批面，但这条例验**一直未满足**——`utterances` 表只保存 `text_hash`（`migrations/sqlite/0001-foundation.sql:10`），
审批列表只返回哈希/元信息/期限/证据计数。**存储侧承载**：迁移 `0006-utterance-display-text.sql` 新增
`utterances.display_text`（`DEFAULT ''`；**只增不改**——`verify()` 逐文件 SHA-256，改 `0001` 会让既有库校验和漂移；
`VERSION` 5→6、`SQL` 5→6 项、`backup.schema.json` 的 `db_schema_version` 5→6），`record_candidate` 具名列写入
候选自身 `display_text`，`list_pending_approvals`/`read_approval` 返回该列。**契约**：`control.yaml` 的 `ResourceItem`
增**可选** `display_text`（**description 含逗号必须加引号**，否则 YAML 流式映射解析成多余键触发校验失败——
本轮真实踩到；选可选而非必填，因必填会让早于 `0006` 的候选无法表示「文本缺失」），三份生成物（`control_http.rs` 1 行、
`http.ts` +2、`backup.ts` 5→6）随之重建。**控制面/桌面**：`PendingApprovalView`+`display_text`；
`console.ts` 的 `candidateText`/`approveConfirmPrompt` 显示文本并在二次确认文案里**点名身份为候选哈希**；
`control.ts` 严格解析拒绝非字符串 `display_text`。**三条口径（ADR 0045）**：**文本是只读呈现、身份始终是候选哈希**
（命令也以哈希为准）；**缺文本具名缺口**（`FINAL_TEXT_GAP`）而非用空串/哈希冒充；`display_text` 在哈希覆盖范围内
（`candidate_hash` 对整个 cue 含该列做 SHA-256）但存储只留哈希与文本两列、**无法**由文本重算哈希本地核对
⇒ **不声称「文本已被哈希校验」**，文本被篡改不会让审批绑定到另一候选、但**可能误导操作员**（**剩余风险**）。
**本机边界**：Node 同步子进程创建被阻断（`spawnSync`/`execSync` 一律 `EBUSY`，managed/system node 均如此，
`sandbox:false` 亦然；**异步 `spawn` 正常**）⇒ `scripts/contracts/generate.mjs`（三处 `spawnSync`）本机必败；
处理是**临时**换异步 shim 跑一次生成、**随后还原脚本**（`git diff` 为空），**不**为主机限制永久改源码。
实测：`check-deps` **18 包**、`check --registry`、`cargo xtask schema` 生成成功；`cargo check --workspace` EXIT=0；
**`check --profile portable` EXIT=0（83 目标 = 66 `Running` + 17 `Doc-tests`、726 passed/0 failed**；W49 为 725，
**+1 本轮新增存储测试**）；`tests/ui` **37/37**（W49 后 34，**+3 本轮**）；两包 `tsc --noEmit`、`@nai/contracts` build、
`vite build` 均 EXIT=0（`index-9TygVhoX.js 206.90 kB`）；`validate --all --strict` **39/39**。
（**本机 `LNK1104` 三次复现**为**主机事实**：portable 两次因链接被瞬时占用失败——修法是把 **workspace 与
`nai-xtask --lib` 两个目标都预热**，一次成功后再跑门控即 LNK=0。）**O02 保持 `pending`**
（真实候选来源/桌面渲染/生产 root 接线/手机审核未接；按 admission-registry 诚实回填**不得**自动升级），
notes 追加本轮成果。新增 ADR **0045**；**不改任何既有验收 `status`**。见
docs/reports/2026-10-11-w51-approval-final-text.md、docs/adr/0045-approval-display-text-boundary.md、
docs/ui.md、docs/control.md。

W52 补上「开放兼容」的地基并闭掉一条**门控盲区**，同时**更正 W51 的 CI 结论**。**更正**：W51 的 CI 实为
`failure`——`347d74e`（run `38133956560`）与 `0e1c233`（run `38134889196`）**两平台同红在步骤 19**
「Validate schemas and API contract」；根因是 W51 改了 `backup.schema.json` 的 `db_schema_version` 常量（5→6）
却**没同步** `tests/contracts/schema.test.mjs` 的样本，**而该测试不在 `engineering_checks` 内** ⇒ 本机
portable/full 绿、CI 红。上一轮「两 job 全绿」的观测来自**被 HTTP 代理缓存的过期 API 响应**（同一次响应
`total_count: 42` 却只列 2 条，即为征兆），已订正并写入 W51 报告**勘误段**（原文不改写）。**目录**：
`catalog.schema.json` 条目改 `oneOf` 两支（`frozen` 须有 `request`/`response`/`stream`；`draft` **禁有**这四个字段）、
`required` 增 `stage`、`minItems` 27→33；`capabilities.json` 现有 **27 条标 `frozen`**、新增 **6 条 `draft`**
（§37.4 的 `data.query@1`、`data.subscribe@1`、`device.sense@1`、`device.control@1`、`asr.transcribe@1`、
`market.catalog@1`）。**生成器**（`xtask/src/schema.rs`）**只为 frozen 生成消费者代码**——
`MESSAGE_TYPES`/`SESSION_MESSAGE_TYPES`/协商用的 `CAPABILITY_IDS` 全取 frozen，draft 另列 `DRAFT_CAPABILITY_IDS`
（`catalog.rs` +9 行）；draft 声明消费者字段在 **JSON Schema 与 Rust 两侧都报错**。**门控**：`registry.mjs`
新增读目录 id 与逐条校验——`contract` 必须命中目录（**draft 也算已声明**）或命中**极小且就地附理由**的白名单
（当前仅 `plugin-hosts-and-ipc`），**不传目录本身即错**（防止「忘了传参数」绕过）；`validation.mjs` 收紧为
「adapter 目标必须是 frozen」并复述 draft 禁令；`tests/registry.test.mjs` 补悬空引用（`market.catalog@9`）负样本
与「draft 引用**不使 M04 升级**」正样本。**门控盲区闭法**：按「是否需 protoc 产物」拆分——
`tests/contracts/schema.test.mjs`（Ajv/Swagger 静态，**无 protoc**）并入 `engineering_checks`，
`tests/contracts/descriptor.test.mjs`（需 `target/contracts/descriptor.bin`）留在 contracts suite；
CI 步骤 19 与 contracts suite 均改跑两个文件；同时订正 `lib.rs` 里**不准确**的注释（portable 从来不是
「只需 Rust 工具链」，真实前置是 Rust + node + **仓库根**依赖）。**如实登记**：①`frozen` 判据本轮是
「有正式消息绑定并生成消费者代码」，设计第 63 行的**更强判据**（须被切片验收覆盖）**未改判**，列为**待裁决项**；
②目录完整模型（类别/必需性/首次切片/降级行为）**未落地**，`stage` 是本轮唯一新增字段；③`contract_catalog_sha256`
是文件**整体哈希** ⇒ 加 draft 会让**早于本轮的备份报 VersionIncompatible**（无生产数据，可接受；收窄哈希是
独立决定，观察项）；④`catalog.ts` 因 `minItems`+`items` 生成 **33 元组**（原 27 元组，**既有**行为），
213→561 行，属生成器可读性债；⑤六个 draft **无消息、无实现、无降级行为**，不产生任何可用于真实直播的能力；
**不改任何 `status`**。实测：`check --profile full`（**与 CI 同命令**）**EXIT=0**；83 目标
（66 `Running` + 17 `Doc-tests`）、**726 passed/0 failed**；前端 5 步全过、`tests/ui` **37/37**；
`tests/registry.test.mjs` **2/2**、`tests/contracts/schema.test.mjs` **11/11**、`descriptor.test.mjs` **2/2**；
`cargo xtask schema`（临时异步 shim，**已还原**）`33 capabilities (27 frozen, 6 draft)`；
`tsc -p packages/contracts-ts` EXIT=0；`validate --all --strict` 与 `--archived` 全绿。见
docs/reports/2026-10-11-w52-open-contract-catalog.md、docs/contracts/README.md。

W53 补上 **A03「新增连接器不改内核」的实物落点**，并顺手把两处「说了但没主语」的性质变成**可判定的结构事实**。
**缺口性质（本轮的核心判断）**：缺的**不是判定**而是**被判定的实物**——§27.1 的
`fake_connector_claimed_as_real_capability` 早在 `bridge.rs` 里，`deps.rs` 的单测夹具甚至已经叫
`plugins/platform-manual`（结构照着「有一个平台连接器」设计），但 `plugins/` 下**从没有任何平台连接器**，
A03 的命题因此**无主语**。**接缝**落 `crates/nai-plugin-sdk/src/connector.rs`（层 `sdk`，只依赖 contracts；
内核不得依赖插件，故接缝必须在内核之外）：`ConnectorAvailability` 只有两态——`Verified { interface_revision }`
（**空值即拒**：「已核实」是有依据的主张）与 `Unavailable { reason }`（`reason` 取自**封闭五值**），
`ConnectorIdentity`（连接器 id + 平台 id + 标签，三者非空）、`AdmissionNotes`（§13.1 **五项** +
§9.3 **四项**，`declared` 对缺项**按名拒绝**）。**§27.1 的四项职责是「提供的方法」**：每个先过
`require_available()` 再交 `*_impl` 钩子（钩子默认返回具名 `NOT_IMPLEMENTED` ⇒ 失败关闭）
⇒ 不可用连接器**不可能**从任何职责返回成功，**「返回 Ok 但什么都没做」在接缝里没有表示**；失败统一是
`CAPABILITY_NOT_READY` + `details["reason"] = "UNVERIFIED_EXTERNAL_DEPENDENCY"`（§27.1 **原文术语**，
与 `bridge.rs::SdkAvailability::UnverifiedExternalDependency` 同名）+ `retryable`。
**连接器**：新建 `plugins/platform-unavailable`（层 `plugin`，包名 `nai-platform-unavailable`，
workspace 成员 **18 → 19**），登记**快手**与**视频号**两个显式 Unavailable 实例（§9.4 同一平台插件目录、
`platform_id` 互异），二者**只声明身份/可用性/准入说明，不实现任何 `*_impl`**——这正是「显式 Unavailable 实现」
的形状。**「不放伪造 SDK」是结构事实而非承诺**（§17.2）：该包只依赖 `nai-contracts` 与 `nai-plugin-sdk`，
且 `tests/unavailable.rs` 用 `include_str!("../Cargo.toml")` **读该包自身的清单**做依赖白名单与 SDK 形态断言，
并**自检该断言非空洞**（把 `douyin-sdk` 注入清单必须被抓到、把 `platform-bindings` 注入同样被抓到）。
**A03 的两个方向现在都由门控强制**：`deps.rs` 既有规则禁「内核依赖连接器」，**W53 新增**禁
「**平台连接器依赖内核**」（`plugins/platform-*` → kernel；对普通插件不适用，测试钉住这一点）。
**非空洞守卫**：测试内有一个 `Verified` 替身走通四项职责的成功路径（§17.5/§721 允许 mock 仅存在于测试），
否则「全部失败」会被一个恒失败的接缝同样满足。实测：`cargo test -p nai-platform-unavailable` **9 项**通过；
`nai-plugin-sdk` lib **14 → 18 项**；`nai-xtask` lib **51 → 52 项**（新增 deps 规则单测）；
`check-deps` **19 包**、依赖方向不变、**外部依赖零新增**。**未升级**：真实官方接口、调试版伴侣 PoC 与
平台准入决定（§9.3）仍是外部真值，两个平台一律如实记 `Unavailable { NoOfficialInterface }`、不宣称可上线，
**A03 保持 `implemented`、P01 保持 `blocked`**，`registry.yaml` 只补 A03 notes。见
docs/reports/2026-10-11-w53-explicit-unavailable-connector.md、docs/platform.md、
docs/platform-lifecycle.md、docs/engineering.md。

W54 裁决 **桌面壳形态与 Tauri 边界**（用户五项决策的第 1 项），并把它落成**可判定的边界**。
**缺口性质（本轮的核心判断）**：形状**早已定了**（§1248 明文 Tauri 2 + React + TS + Vite；§1260 产物含
`desktop-shell`），缺的**不是选型**，而是**一条裁决**与**可查的落点**——`docs/adr/` 的 44 篇里**没有**桌面壳形态
这一条；`apps/desktop` 是**纯 Web**（deps 只有 react/react-dom/vite，无 `src-tauri/`）；而 §1151（capabilities/CSP
最小权限）、§1259（sidecar 三元组与 UI 父子生命周期）、§900（数据根）、§1101（同机优先受保护 IPC）四组条款在
`crates/`/`apps/`/`schemas/` **零命中**；`tests/registry.yaml` 的 66 条里**没有**桌面壳条目。
**注意一条「不是缺口」的事实**：INV-18「内核不依赖 Tauri」**已由 `xtask/src/deps.rs` 强制**
（`starts_with("tauri")` + 单测夹具 `&["windows-sys","tauri"]`），本轮**不重复实现**，ADR 只引用它。
**ADR 0046** 裁决：形态 = Tauri 2；**壳只做四件事**（窗口与托盘生命周期／受保护 IPC 宿主侧／数据根解析／
sidecar 拉起与监督），业务语义（会话·守门·审批·知识·媒体编排）MUST NOT 进壳（§17.5、§181）；
**最小权限面「逐一相等」**——§1151 的「只开放必要」是**上界**断言，而「包含」只证下界（含 `shell:allow-execute`
的集合显然「包含」必要权限），且**禁止 `core:default` 一类聚合权限集**（一条标识符展开成一组权限，使「最小」
不可逐条检查），故权限标识符须匹配 `^core:[a-z]+:allow-[a-z0-9-]+$`；CSP 无 `unsafe-eval`、任一指令无通配源
（`*`／`http:`／`https:`／`ws:`／`wss:`）、`connect-src` 必须含 `ipc:`（`data:` 与 `http://ipc.localhost`
**不在**通配列）；**受保护 IPC 是生产路径、loopback HTTP 是测试适配**（§1101：既有 `ControlClient` 不变，
只在声明里定性）；后台生命周期**不绑界面**、**不依赖 WebView 保活**（§1259）；sidecar 声明写**不带三元组**的
**逻辑名**且必须对应 `apps/` 下**真实存在**的二进制包，命名规则 `包名-目标三元组(+.exe)`；**三形态共用边界**
（桌面／macOS／移动端路线 B，§10.2/§10.3），差异只允许落在**目标与分发**上。
**本轮最关键的一取舍：不引入真实 Tauri 依赖**。`tauri`/`tauri-build` 在 Windows 上需要 WebView2 的导入库、
构建链会访问网络 ⇒ 拉进 workspace 会让 `check --profile portable|full` 的结论取决于**本机是否装了 WebView2、
网络是否可达**——那是**主机事实**，正是 ADR 0041 已裁过的禁忌。故本轮交付**真实格式的声明**与**自建的形状校验**
而不是构建产物；**workspace 仍 19 包、零新增依赖、Rust 侧零改动**。**落点**：
`apps/desktop/src-tauri/{tauri.conf.json, capabilities/default.json, shell-policy.json}`
（`shell-policy.json` 是本仓库自有的策略声明，把 Tauri schema 表达不了的四条边界——范围／生命周期／IPC 角色／
形态共用——变成可判定数据）＋ `tests/ui/desktop-shell.test.mjs`（**15 项**，经既有 `test:ui` 自动进入 `full` 门控，
**不改门控代码**——`scripts/frontend-gate.mjs` 按 `package.json` 的 `scripts` 驱动）。**非空洞守卫**：注入
`shell:allow-execute`、`core:default`、`unsafe-eval`、通配源、`@tauri-apps/plugin-fs`、未登记 sidecar、
`backendsOutliveUi=false` 等样本，各检查**必须分别抓到**——否则「配置通过」与「检查恒真」无法区分。
**未升级**：`tests/registry.yaml` 新增 **A12「桌面壳宿主边界」**、`status: pending`；§1248/§1260 的真值是
**可运行、可打包、可签名**的壳体，本轮无 `tauri` crate、无 Rust 主体、未打包未签名。**已知后果（如实登记，非缺陷）**：
形状校验**不能**证明配置被某个 Tauri 版本接受；Tauri 大版本改动配置字段时本仓检查**不会自动发现**；
`tauri.conf.json` 的 `$schema` 指向官方 URL、**离线不可校验**（故校验自建、不依赖网络）。
见 docs/adr/0046-desktop-shell-form-and-tauri-boundary.md、docs/reports/2026-10-11-w54-desktop-shell-boundary.md、
docs/control.md、docs/ui.md。

W55 把**平台**变成仓库的一等公民（用户五项决策的第 4 项：macOS 覆盖 Standard 与 Pro）。
**缺口性质（本轮的核心判断，与前两项不同）**：W53（第 3 项）与 W54（第 1 项）都是「**基线有明文、仓库零落点**」，
而这一项是**全新需求**——基线 V3.9 全文对 `macos`／`darwin`／`apple` 的命中数为 **0**（随基线订正重新实测，V3.9 亦为 0；只有 Windows 首发、
Linux 轻云与 WSL、iOS／Android 实验），`tests/registry.yaml` 亦无任何平台维度条目（全部验收按业务分系列，
`release_line` 只表达 Slice-0／R1／R2／R3／实验）⇒ **无基线判据、无验收标准、无目标三元组、无签名与分发策略**。
故按仓规「验收唯一来源为 `tests/registry.yaml`、新需求先立验收」先做**可判定的一步**，而不是凭想象写 macOS 实现。
**侦察到的三处已存在却从未被登记的事实**（本轮最有价值的部分）：①全仓含平台条件编译的文件有 **23 个、116 处**
（此前没人统计过，窄口径只看 `cfg(windows)` 会漏掉大半）；②`shared_ram.rs` 的文件头与 **ADR 0017** 都明文写着
帧槽是 **Linux-only 的 tmpfs-file 工程演练后端**，且「**Windows/非 Linux 后端明确 `CapabilityNotReady`**，
不改变 Windows 正式共享内存目标」——即首发平台 Windows 自身在这条能力上是**已裁决**的不可用，**不是缺陷**；
③`data-root` 的 0700 权限、`path-containment` 的链接拒绝、`process-supervision` 的退出状态走 `#[cfg(unix)]`，
而 **`unix` 同时覆盖 macOS** ⇒ macOS 在这些能力上**已有可复用落点**，并非空白。
**核心判断**：缺口不是「缺判定」，而是「**缺登记**」——这 116 处没有一处需要改代码。所以本轮补的是登记与约束。
**交付**：`schemas/platform-matrix/platforms.json` ＋ `platforms.schema.json`（形态）＋
`scripts/platform/matrix.mjs`（校验逻辑）＋ `tests/platform/platform-matrix.test.mjs`（**22 项**，
经 `engineering_checks` 自动进入 `portable` 与 `full`）＋ `docs/platform-matrix.md`。
**三个设计要点**：①**理由码从源码派生**——矩阵的 `explicit-unavailable` 必带理由码，而合法集合从
`crates/nai-plugin-sdk/src/connector.rs` 的 `UnavailableReason::label()` 的 match 臂解析得到，**不在校验里另立
常量表**（否则改一侧忘另一侧时校验变成假的保证；Rust 新增变体无需改脚本，用了不存在的码立即失败），派生前先断言
「该函数存在且解析非空」，形状变化时**具名失败**而非静默通过；②**核心门控是「文件子集」**——全仓含平台条件编译的
文件 MUST 是矩阵登记路径的**子集**，新增一处 `cfg(target_os = ...)` 而不同步矩阵即失败并指名文件，这把「平台假设
只增不减」变成可判定事实（同 W52 的「验收 `contract` 必须可解析」、A03/A04 的依赖方向检查是同一类断言：
**让没被写下来的事实变成构建失败**）；③**`bound_to` 一般化判据**——声明某原生平台依赖的能力在其它平台上
MUST NOT 是 `supported`，于是「抖音 SDK 是 Windows 原生目标」不必在校验里硬编码能力 id。
**macOS 的两处硬约束（如实登记）**：①§14.1 把「抖音连接器与事件治理」列为 **Standard 的核心构件**，而 §9.2 抖音
与 §687「Windows 原生是 SDK、音频与写实的正式运行目标」说明抖音 SDK 与 Windows 音频是 **Windows 原生目标**
⇒ 平台 SDK 在 macOS 上登记为 `explicit-unavailable(no_official_interface)`，**不假定也不臆造 macOS 侧等价接入路径**；
②§300 写实加速为「Windows NVIDIA CUDA 首发」、TensorRT 与 **DirectML（Windows 专属）** 单独认证 ⇒ macOS 侧
需另定路径、**尚未裁决**。⇒ macOS 上的 Standard 与 Pro 内容 MUST 与 Windows 档位表**分别定义**，不得照搬。
**用户拍板落为数据断言**：macOS 的 `release_lines` 必须同时含 `r2` 与 `r3`（文档会漂移，数据会被校验）。
**非空洞守卫**：注入未声明平台键、缺平台、坏理由码、PascalCase 变体名、unavailable 无理由、supported 带理由、
悬空 `source_files`、未登记的平台分支文件、macOS 丢 `r3`、Windows 绑定能力在 macOS 标 `supported`、`source` 类
无证据、`baseline` 类登记文件、重复平台 id、非法发布线、坏三元组，各检查**分别抓到**；另有一条
**「扫描非空且命中已知分支」守卫**——若扫描恒为空，「文件子集」这条核心门控会**恒真**，必须先把扫描本身钉住。
**未升级**：新增 **A13「平台能力矩阵与源码一致」**（`r1`／`unit`／`implemented`）与 **A14「macOS 覆盖
Standard 与 Pro」**（`r2`／`manual`／`pending`）；**零 Rust 业务改动**（仅在 `engineering_checks` 增一行命令）、
**零新增依赖**、**workspace 仍 19 包**。**如实登记的缺口**：基线 §29.4 要求的
`profiles/{editions, deployments, resources}` 目录**至今不存在**，本轮**不**创建（与本矩阵无重叠：
本矩阵登记的是**操作系统平台**的能力，不是**档位**或**部署配置**）。
见 docs/platform-matrix.md、docs/reports/2026-10-11-w55-platform-boundary-matrix.md、schemas/platform-matrix/。

W56 把「智能体 Harness 的工程化能力 + 开放互操作（MCP、A2A）」写进设计基线，产出
`docs/直播数字人 + 智能体系统技术方案 V3.8.md`，并由 **ADR 0047** 升为唯一实现基线（V3.7 降为历史参考；**V3.8 后由 ADR 0048 升为 V3.9**）。
**方法沿用 V3.6 → V3.7 的形状**：第四部分**追加**在文档末尾（§40–§48，**491 行**），§1–§39 **一字不动**——
实测仅 **8 行**改动（4 个 diff hunk，全部 1:1 行替换：标题、副标题、§0.2、§1.4 两行、§16 前导、§17.3 两行），
**1875 → 2366 行、行号一一对应**，故全部既有 `§NNN` 引用在本版解析结果一致。
**缺口是实测出来的，不是印象**（对 V3.7 全文命中数）：`Harness` **0**、`A2A` **0**、`意图识别` **0**
（且 §5.2 明写「不先做独立分类」——该取舍只对**实时路径**成立，却被写成全局结论）、`工具调用` **1**
（§987，是 SSE 解析措辞「工具调用增量」而**不是能力定义**）、`语义缓存` **3**、`MCP` **4**
⇒ V3.7 有智能体的**零件**（决策循环、分级路由、守门、技能、知识），但**没有 harness**。
新增 **INV-27–INV-32**：装配是**声明且可重建**的；记忆写入有门槛、读取有作用域；意图/置信度/分类**只作建议，
绝不构成授权**；缓存命中**不复制授权**且价格/库存/优惠/承诺/服务政策禁入；外部工具、MCP 与对端返回一律按
**不可信数据**处理，其自称「只读/已批准」**无任何效力**；任何一层失败 MUST 给出**具名错误码**并按 §12.2 降级。
**本轮最有价值的一步是直接应用 ADR 0042 决策 4 留下的教训**：H01–H18 **就地登记**为 `pending`（注册表 69 → **87**），
并在 `scripts/contracts/registry.mjs` **实现**规格早已要求、却从未实现的**家族在册断言**
（M/H 各 **18** 条**逐号**比对，缺号即失败并**指名缺哪个**）——此前 `check --registry` 只校验**已登记条目的合法性**、
**不校验家族是否齐全**，这正是 W46 那类缺口能长期不可见的**机制**原因。断言做成**非空洞**：
删 `H05`（家族内**洞**）、整族删除、加 `H19`（**越界**）、删 `M18`（证 M/H **分别**计数）四种注入**分别被抓到**。
**本轮不新增契约名、不改能力目录**（故生成物**零漂移**，`schema --check` 无关）：MCP 的 `tools` 映射为
`skill.invoke@1`、`resources` 走 `data.query@1`；A2A 的**协议版本与认证方式待验证**，为未验证协议占契约名只会产生
**半声明 draft**（§4.1 与 W52 起的能力目录纪律禁止），故 H13/H14 的 `contract` **留空**（沿用 M10/M12 先例）；
并**如实登记** §20.6 与能力目录对 `memory.access@1` 阶段的不一致（以目录为准，**不改 §20.6 原文**）。
**明确不做**：**零 harness 实现**（W56–W64 未开始，H01–H18 一律 `pending`）、**本轮不新增静态门控**
（无实现可门控，写了只能是**空洞门控**）、不改任何既有 acceptance 的 `status`、不重排 W 编号
（V3.7 计划止于 W40，**W41–W44、W46–W55 已被实际执行轮次占用**——**W45 无任何记录**，V3.9 §56-C6 订正 ⇒ 第四部分从 **W56** 起）、不扩张观众长期记忆字段集、
不臆造 MCP/A2A 协议版本。**三处阈值**（意图置信度、语义缓存相似度、上下文利用率）一律「**待本场景回放实测**」，
MUST NOT 直接采用外部观察值（外部「40% 利用率」之类的数字**不可外推**）。
**校验口径的一处修正（本轮发现，容易漏）**：`openspec validate --all --strict --archived` **只校验已归档变更**
（本轮实测 61 条全是 `change/…`），**不校验 `openspec/specs/`**；规格须另跑不带 `--archived` 的那条
（`--all --strict` → 42 个 `spec/…`）。**两者覆盖不同对象、都要绿**。本轮就是靠后者才发现归档合并后
`admission-registry` 的需求正文 **541 字符**（> 500，`--strict` 失败）——而变更层的同一段文本
**通过**（> 500 规则只作用于 `specs/`、不作用于差量），故归档前**看不出来**。修法：细节移进场景
（541 → **336**）。
见 docs/adr/0047-design-baseline-v38.md、docs/直播数字人 + 智能体系统技术方案 V3.8.md、
openspec/changes/archive/2026-10-11-w56-agent-harness-and-interop/、docs/reports/2026-10-11-w56-agent-harness-and-interop.md。

W57 在 W56 开出的轨道上落下**第一个判定面**（记忆的**读写与遗忘**：`crates/nai-kernel/src/evaluate/memory.rs` 的
**只增不改**扩展 + `crates/nai-kernel/tests/memory.rs` **21 项**，H03/H04 机制已交付、**状态仍 `pending`**）。
**本条此前在 AGENTS.md 中缺记**（本文件的轮次记录止于 W56）——本轮补记指针：
见 docs/kernel.md、docs/reports/2026-10-11-w57-memory-read-write-surface.md。

W65 做的是**范围重排与升基线**，不是加能力。产出 `docs/直播数字人 + 智能体系统技术方案 V3.9.md`（新增**第五部分
§49–§57**），并由 **ADR 0048** 升为唯一实现基线（V3.8 降为历史参考；ADR 0047 **决策 1 被替代**，其决策 2–5 继续有效）。
触发是一次外部审视（对 V3.8 的五维批评）与用户对它的反批评，**两份主张都被逐条取证核实**：产品向批评成立
（首版范围膨胀——收缩前 `r1` 56 + `slice_0` 10 = **66** 条；版本只做加法，无「砍掉/延后/降级」栏），
多项技术判断不成立（`181.9` 全文 **0 命中**；§23.2 实测 **32** 项而非 44）。**双方都没发现的 7 处问题**里有一条
严重项：`openspec/specs/spec-driven-delivery/spec.md` 的基线指针**自 V3.6 起就停在 V3.5**，
而 ADR 0042、ADR 0047 **两轮**都把它记为遗留而**不**订正。
**本轮第一次不再以「纯追加」处理升基线**：前两轮纯追加的前提是「无矛盾可订正」；本轮查出 7 处矛盾，
继续追加只会把矛盾**登记第二遍**。故 V3.9 §56.2 确立一般规则——**活文档就地订正 / 历史记录保留原文 + 追加勘误段**，
并给就地订正加硬约束：**MUST 是 1:1 行替换（不增删行）**。实测前 2366 行 **24 个 hunk / 42 行 / 全 1:1 / 零增删**
（`a`、`d` 型 hunk 为 0），**2366 → 2647 行** ⇒ 全部既有 `§NNN` 引用继续有效。规范订正 **C1–C9**：
**C6 的教训**——同一条不实陈述（「W41–W55」）实测**散落在四个活文档**（§47.6 编号说明、§48.6、`AGENTS.md`、
`docs/development-plan.md`），只订正主规范一处不够；**C8/C9 的教训**——**易漂移的计数与版本号 MUST NOT 写进活文档**。
**核心交付是机制**：新增**第六条发布线 `capability_pack`**（§51.1）——**「已设计」与「已承诺」是两个不同的陈述**，
此前只有前者有地方可写，于是「降级」只能表现为删除。**29** 条（M01–M18、H05–H14、H16）自 `r1`/`r2` 降为能力包，
收缩后 `slice_0` **10** + `r1` **36** + `r2` **9** + `capability_pack` **29** + `r3` **1** + `experimental` **2** = **87**
（**只改归属、不加条目、不改任何 `status`**）。**INV-33**（默认组合不依赖能力包：R1 内每条验收 MUST 在
**未安装任何能力包**时可判定）与 **INV-34**（降级不删边界，MUST NOT 使既有 `§NNN` 引用失效）是**用户三条约束的
可判定形式**（轻量由**默认组合**定义而非能力上限；降级是**排期**动作而非**设计**动作；§51.4 保证加装沿用
**同一条**升级与迁移图，**MUST NOT** 有第二条安装路径）。**分界判据**：*「一个确定性的上下文装配函数能不能满足它？」*
能 ⇒ 留在 R1；只有建独立子系统才能满足 ⇒ 能力包。**门控增量**（本版唯一机器可判定处）：`tests/registry` 的**三向**
能力包集合断言（集合恰为 29 条 / 无归属外 id 混入 / **无能力包 id 落在 `slice_0`/`r1`/`r2`**），
失败时**同时给出 id 与当前所在线**（形如 `M01=r1`），并以**三种注入**证其非空洞（**4/4 通过**）；
`schemas/tests/registry.schema.json` 的**两处**硬编码（`release_lines` 与 `acceptance.release_line.enum`）同改。
另引入**版本减法纪律**：每个版本的变化表 MUST 含**新增/砍掉/延后/降级为能力包**四栏且**同权重**评审，
只有「新增」栏的版本 SHALL 被读作**未做范围评审**——V3.9 是第一条适用记录，**本版砍掉的：无**（范围重排，不是能力删除）。
**明确不做**：不删任何能力、不整体移出第四部分、不删 workspace 作用域、不删决策日志、不停止内核开发、
**不新增契约名**（能力目录零漂移）、不改任何 `status`、不重排 W 编号（第五部分从 **W65** 起）。
**验证**：`cargo xtask check --profile full` **EXIT=0**，测试目标 **87（69 `Running` + 18 `Doc-tests`）**、
**761 passed / 0 failed**（**与 W57 完全相同**——本轮零 Rust 测试改动，数字不变即「未误改代码」的旁证）；
`check-deps` **19 包**；注册表输出串已为 **V3.9**。**本机限制（如实登记）**：`cargo xtask schema --check` 在本机
**跑不了**——`generate.mjs` 内部的 `spawnSync(process.execPath, …)` 报 **`EBUSY`**（managed 与 system node 均复现，
属**主机/沙箱**对「node 自旋 node」的限制，非仓库缺陷）；契约零漂移改以**结构性理由**取证
（生成器输入仅 `schemas/catalog/capabilities.json` 与 `schemas/{config,plugin}/*.schema.json`，实测均未改动，
`crates/`、`packages/` 亦无改动），并由 **CI 的独立步骤**兜底。
见 docs/adr/0048-design-baseline-v39.md、docs/直播数字人 + 智能体系统技术方案 V3.9.md、
docs/reports/2026-10-11-w65-v39-scope-and-baseline.md、docs/reports/2026-10-11-v38-review-adjudication.md、
openspec/changes/archive/2026-10-11-w65-v39-scope-and-baseline/。
