# nai development instructions

- 项目英文名为 `nai`，仓库为 https://github.com/F31/nai。命名映射见
  `docs/adr/0001-project-name-and-design-baseline.md`。
- `docs/design.md` 指向唯一实现基线
  `docs/直播数字人 + 智能体系统技术方案 V3.11.md`（ADR 0065 升基线；ADR 0049、ADR 0048、ADR 0047、ADR 0042 与 ADR 0015 的裁决继续有效）。历史版本（V3.10/V3.9/V3.8/V3.7/V3.6/V3.5 等）只供参考；
  不自行拼接旧条款覆盖 V3.11。规范变更记录 ADR。V3.11 的 dh-* 命名不采纳，工程标识一律 nai-*。
- 验收唯一来源为 `tests/registry.yaml`（V3.11 S/R/A/B/O/P/M/H/X + release_line（新增 capability_pack），D→新编号映射自建）。
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

D15 验收注册表与基线切换已实现/实测：ADR 0015 把 V3.6 升为唯一实现基线（勘误清单生效，dh-* 命名不采纳；V3.6 后由 ADR 0042 升为 V3.7、V3.7 后由 ADR 0047 升为 V3.8、V3.8 后由 ADR 0048 升为 V3.9、**V3.9 后由 ADR 0049 升为 V3.10、V3.10 后由 ADR 0065 升为 V3.11**），
tests/registry.yaml 为验收唯一来源（S/R/A/B/O/P/M/H/X + release_line，D01–D14 映射自建并回填）。
`cargo xtask check --registry` 与 `cargo xtask check --profile portable` 可执行；17 个主规格、15 项归档校验通过。
决策日志 decision_log（R07 前置）与值守租约（600ms 短期限）为下一轮；播放 ACK 段保持 Draft，不冒充完整 R07。
详见 docs/adr/0015-v36-baseline-and-admission-registry.md、docs/reports/2026-10-08-d15-admission-registry.md 与 tests/registry.yaml。

W02/W03 决策日志与值守租约已实现/实测（按 V3.11 §31 工作项推进，不再新增 D 编号）：
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
把 V3.11 §14.3 的九步升级流程落成可执行编排，registry R05/R09/R10 由 pending 升 implemented（**不标 tested**：
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
`pending` 才是在册未开始。缺口的后果是「以注册表为准」这条规则会把 V3.11 第三部分（§33–§38、W26–W40）从验收视野里抹掉；
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
而这一项是**全新需求**——基线 V3.11 全文对 `macos`／`darwin`／`apple` 的命中数为 **0**（随基线订正重新实测，V3.9 亦为 0；只有 Windows 首发、
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
`docs/直播数字人 + 智能体系统技术方案 V3.8.md`，并由 **ADR 0047** 升为唯一实现基线（V3.7 降为历史参考；**V3.8 后由 ADR 0048 升为 V3.9、V3.9 后由 ADR 0049 升为 V3.10、V3.10 后由 ADR 0065 升为 V3.11**）。
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
属**主机/沙箱**对「**node 派生任何子进程**」的限制（W74 实测 `git`/`cmd.exe`/`sh` 同样 `EBUSY`），非仓库缺陷）；契约零漂移改以**结构性理由**取证
（生成器输入仅 `schemas/catalog/capabilities.json` 与 `schemas/{config,plugin}/*.schema.json`，实测均未改动，
`crates/`、`packages/` 亦无改动），并由 **CI 的独立步骤**兜底。
见 docs/adr/0048-design-baseline-v39.md、docs/直播数字人 + 智能体系统技术方案 V3.9.md、
docs/reports/2026-10-11-w65-v39-scope-and-baseline.md、docs/reports/2026-10-11-v38-review-adjudication.md、
openspec/changes/archive/2026-10-11-w65-v39-scope-and-baseline/。

### W66 harness 观测与逐层具名降级（V3.11 §47.1、§47.4；验收 H15/H17）

W66 落 **W63**——V3.9 收缩后 R1 harness 轨道上**唯一完全未开始**的工作项。触发是实测盘点：
`grep -rn 'harness' crates/` 在判定逻辑上**零命中**，即「有没有静默降级」只能靠人读代码回答。

**新增** `crates/nai-kernel/src/evaluate/harness.rs`（**不新建顶层模块**：`evaluate/` 的契约就是纯逻辑判定，
且 `memory.rs` 头注释已引用 INV-32，放同处才不会出现两个都声称实现 INV-32 的模块。
**不复用** `bridge::Degradation`——那个只服务 §119 平台中断，合并会让「降级属于哪一类」重新不可判定）：

- **观测（H15）**：五类各带 `required_fields()`（逐项照抄 §47.1）；判「能否进日志、以何形态进」——
  必需项齐备、值内无凭据字面量（INV-11，**扫值不扫字段名**）、观众原文已脱敏且在保留期内。
- **降级（H17）**：六类失败各带互异 `code()` 与 `preferred_level()`，从首选级起沿 §12.2 四级取第一个可用级，
  静音待机恒可用；装配层 `refuses_call` **不因落点变化而解除**；`visible` 恒真（**不存在不可见的降级**）。
- **INV-32 后半段**：`assert_no_free_form` 判「已降级且失去策略/人设约束」为具名违规——只给错误码仍生成，同样判违规。

`tests/harness.rs` **24 项**，四组**能区分两种实现**的守卫**每组都用注入验证过确实会失败**：
字段名黑名单（3 项抓住）／所有失败压到第一级（1 项）／「命名了就继续生成」（1 项）／悄悄删 `crop_reason`（2 项）。
最后一条是本轮**补出来的缺口**：在此之前 `required_fields()` 没有任何测试钉住 §47.1，
删一项不会有任何测试失败 ⇒ 新增一条把五类字段表**逐项写死**的断言。
**教训：补「完整性守卫」时要问一句——它钉住的是「条数」还是「内容」？只钉条数，删内容照样绿。**

**明确不做**：不把记录追加进 `nai-storage` 的哈希链（runtime 接线）、不渲染进 `CapabilityView`（呈现层）、
**不新增契约名**（目录零漂移）、**不新增第二个门控入口**、不改任何 `status`、不实现 H05–H14/H16（能力包）。
**边界（如实）**：H15/H17 **保持 `pending`**（沿用 W57 对 H03/H04 的口径：判定面 ≠ 真实路径已接线），
故报告 MUST NOT 出现「已进决策日志」。
见 docs/kernel.md（W66 段）、docs/reports/2026-10-12-w66-harness-observability-and-degradation.md、
openspec/changes/archive/2026-10-12-w66-harness-observability-and-degradation/。

### W67 harness 爬坡门控（V3.11 §5.9、§47.2、§47.3；验收 H18）

W67 落 **W64 的 R1 那一半**。V3.9 §51.3 把 R1 保留的 harness 基础纪律定为 H01–H04、H15、H17、H18；
前六条已各有判定面（W56/W57/W66），**H18 是最后一条完全未开始的**。新增
`crates/nai-kernel/src/evaluate/ramp.rs`（**不**并入 `evaluate::harness`：§47.1/§47.4 判一次调用、
§47.3 判一个机制×一个阶段，维度不同；也不复用 `session::change_mode`——那是会话维度的值守租约）：

- 四段爬坡中**受监督自动是唯一被门控的阶段**（`DEFAULT_ENABLED_STAGES` 照抄 §47.3）；
- 晋升须过两条：**声明受影响类别且全部通过、并绑定当前 harness 修订**（§47.2 的「才生效」⇒ 修订变化判 `regression_stale`），
  以及启用记录**指名同一机制与目标阶段、值内无凭据**。
- `touched` 为空判 `regression_scope_undeclared`——否则一段空声明即可绕过门控。

**照实登记的发现**：§5.9／§47.3 是**四段**，但契约 `SessionMode` 只有**三个**取值、**无 `Shadow`**，
而 §1.4／§17.3 把「影子模式」列在 **R2** ⇒ **H18 的默认启用集合里有一个阶段在 R1 内没有落点**。
`RampStage::session_mode()` 对影子返回 `None`（不伪造映射），并由
`every_stage_but_shadow_has_a_session_mode` 钉死。**本轮不改基线文本**（改基线是升基线轮次的事，
且 MUST 是 1:1 行替换），作为待确认项交下一轮裁定。

**明确不做**：不按阶段真正启用机制（runtime 接线）、不把启用记录追加进 `nai-storage` 的哈希链、
**不实现六类回归的执行体**（那是能力包 H16）、**不新增契约名**、不改任何 `status`、不实现 H05–H14/H16。
**边界（如实）**：H18 **保持 `pending`**；本轮只判「回归是否已记录为通过」，**不**跑任何黄金集用例。
见 docs/kernel.md（W67 段）、docs/reports/2026-10-12-w67-harness-ramp-gating.md、
openspec/changes/archive/2026-10-12-w67-harness-ramp-gating/。

### W68 基础讲品循环（V3.11 §53、§5.3、§5.7、§5.4、§45.3；新增验收 B07）

W68 落 **§53**——V3.9 **自称「本版最重要的一条」**的那一项。触发是一次实测：§53 把它写进 R1 的**定义句**
（§14.1 档位矩阵、§56 的「R1 = Lite」行、§2324 的定义段），但 §48.5 的家族索引写「B | B01–B06」、
§57.3 的 R1 清单里**没有以它为主题的条目**，registry 里「讲品／段落编排／插问／节目／讲解／商品／卖点／售后」
的命中数**均为 0** ⇒ **门槛定义里有它、验收清单里没有它**（与 W54 之前的桌面壳、W55 之前的平台维度同类）。
新增 `crates/nai-kernel/src/evaluate/selling.rs`（**不**并入 `evaluate::show`——那里判**单个 ShowPlan 自身**是否合法，
本模块判**跨段落**的编排规则；也**不**并入调度面——那里的「冷却」是观众/任务维度的，这里的冷却是段落维度的）：

- **段落编排**：四段与顺序（卖点→规格→适用场景→售后说明）逐项照抄 §53；未定义段名、缺段（**指名**缺哪些）、
  乱序各自具名；判**序列**不判集合——排序后比较集合的实现会放行错序；「缺段」与「乱序」同时成立时**两条都报**。
- **插问后归位**：检查点只允许「所指段落＋已完成句引用」；`carried_text` 非空即拒（该字段是为使「带了未播出文本」
  **可构造**而设——缺了它该违规在类型上无法表达，守卫会恒真）；恢复位置必须**落在检查点所指段落**。
- **段落级冷却与最小切换间隔**：两条**分开**判定；阈值由调用方给定、模块**不内建**（§48.7：三项阈值待本场景回放实测，
  MUST NOT 采外部观察值；§5.3 的数字明确是「场景策略初值，不是固定标准」）。进入时刻序列须非降序，
  前置条件失败**具名**且区间规则不再评估（间隔在递减序列上无定义，直接相减会下溢）。
- **事实约束的行动提示**：§45.3 禁用域五类（价格／库存／优惠与活动条件／承诺与售后政策／个体化承诺）逐项写死；
  「无绑定」与「引用非**当前有效版本**」**分开**具名——只校验「引用非空」的实现会让三天前的价格通过。

`PitchRejection` 收录 **10** 个具名拒绝；`tests/selling.rs` **19 项**含五组非空洞守卫，每组都用注入验证过确实会失败
（集合比较 / 只报首个缺失段 / 忽略未知段名 / 只问「引用是否存在」 / 区间规则无前置门控）。
其中「只问引用是否存在」的守卫样本刻意取**同 object、不同 revision**——改用不同 object 的样本会让坏实现照样通过
（W67 那条「样本必须先落在判据上」的教训，本轮再次适用）。

**基线偏差（如实登记）**：新增 **B07** 后，§48.5 的「B | B01–B06」与 §57.3 的计数行
「= 87（本版只改归属，不加条目）」不再等于实际值。本轮**不改基线文本**（改基线是升基线轮次的事，
且 MUST 是 1:1 行替换），偏差在报告与活文档登记，留待下一轮升基线一并收录——与 W54 新增 A12、
W55 新增 A13 的处置一致。**该偏差已在 W71 收录**：V3.10 §48.5 的家族索引为「B | B01–B07 | 第 16 章、53」、§57.3 的计数行为 `r1` **37** / 合计 **88**（§58.2 的 E1–E3）。

**明确不做**：不生产段落内容与话术、不做 TTS 时长与回放采样、不接运行时、**不新增契约名**、
不改任何既有 `status`、不改基线文本、不实现 §53 之外的能力包（§53 明说四项都是确定性规则，不引入任何能力包）。
**边界（如实）**：B07 **保持 `pending`**——交付的是**判定面**；§5.7 的「恢复前重新校验产品、价格、活动、
素材与授权」由守门与知识层执行 ⇒ 报告 MUST NOT 称「讲品循环已可运行／已可恢复」。
见 docs/kernel.md（W68 段）、docs/reports/2026-10-12-w68-basic-selling-loop.md、
openspec/changes/archive/2026-10-12-w68-basic-selling-loop/。

### W69 上下文装配（V3.11 §41.1–§41.5、§47.1、§47.4；验收 H01/H02）

W69 落 **H01/H02**——W56 立 harness 轨道时的**原定目标**（§47.6 的 W56 行写的正是「上下文装配器」）。
此后 W57／W66／W67 分别落下记忆读写面、harness 观测与逐层降级、harness 爬坡门控，**H01/H02 一直停在
「有名称、有 registry 条目、无实现」**。本轮触发点是两次核实：

1. **`harness.rs` 里 `Assembly` 出现十余次，但没有一处判定装配是否合法**——每处都是**层名或字段名**
   （`ObservationLayer::Assembly`、`FailureLayer::Assembly`、必需字段 `segment_budget`／`cropped_items`／`crop_reason`）。
   ⇒ 我此前「R1 七条已全部有判定面」的说法**不成立**，本轮一并订正（教训：**「已交付」要按「有没有判定函数」核，
   不能按「名字出现过」核**）。
2. **`agent-harness` 能力里已有这两条需求**（W56 建立）⇒ 本轮走 `MODIFIED`，**不是** `ADDED`、**不是**新建能力。

落在 `evaluate::assembly`（**不**并入 `harness`——那里判「记录能否进日志、一次**失败**落到 §12.2 哪一级」；
也**不**并入 `selling`——那里的「四段」是 §53 的讲品段落，是**另一条轴**；与 W66 不复用 `bridge::Degradation`、
W67 不复用 `session::change_mode` 同一条理由）：

- **九段与顺序（§41.1）**：系统策略→人设→输出契约→事件摘要→证据→记忆→技能→最近播完句→工具结果，逐项照抄；
  顺序存在的理由是**约束不被挤掉**。判**序列**不判集合；未定义段名与乱序各自具名；**不短路**（缺段与乱序可同时报出）。
- **不可裁段 1–3**：缺失即拒，且**按名列出**缺了哪几段（不只报数量）；序号由 `UNCROPPABLE_COUNT = 3` 派生，
  **不**另写一张硬编码清单。
- **裁剪只从后段**：被裁的可裁段必须构成**后缀**——「证据被裁、而更靠后的技能与播完句完好」这种形状
  不可能由倒序行走产生，所以只判「有裁剪记录、有原因」不够；裁剪须**同时**记录被裁项与原因。
- **分段预算（§41.2）**：token／字符／条数**三维**逐段判定，**互不替代**，缺一维即拒，超限**报出是哪一维**；
  不可裁段须有 floor。阈值**由调用方给**、模块不内建（§41.2 初值须经 §47.2 回放调优并记入 ADR；§48.7 禁采外部观察值）。
- **片段标注（§41.3）**：七类来源逐项照抄；「观众文本／检索片段／工具返回／外部智能体返回**一律标为数据**」
  由 `must_be_data()` 表达，另三类 §41.3 无此约束故**不判**；作用域**必填**、时效「如适用」故**不判**；
  `Trust` **不是** `Option`——使「缺标注」在类型上不可表达，于是只有**标错**才需要拒绝。
- **可重建（§41.5／H02）**：`rebuild_assembly` 只消费**决策日志**，按**日志自身顺序**排出被引用片段再与记录比较——
  按记录顺序排序后比较会让「日志顺序与记录不同」**恒真**（注入已验证）；`FragmentNotLogged`（根本没记）与
  `FragmentContentNotInLog`（记了但正文没落盘、只存了指向当时内存的指针）**分开具名**；明文凭据复用
  `harness::CREDENTIAL_MARKERS`（INV-11，不建第二张表）。

`AssemblyRejection` **14** 条、`RebuildRejection` **6** 条；`tests/assembly.rs` **36 项**含两条「恰为 `ALL`」总账、
**逐项写死**的基线表守卫（九段／七类来源／不可裁序号／四类一律为数据）、**跨模块**守卫
（装配记录字段 == harness 的 L1 必需字段表）与七组非空洞注入（集合比较／缺段只报首个／忽略未定义段名／
去掉后缀检查／合成六条重建拒绝／按记录顺序排序／改字段名，各自被抓住）。

**复核方法上的一处坑（本轮实测，跨项目可复用）**：注入脚本用 `fs.copyFileSync` 复原源码时，
**Windows 会连同 mtime 一起复制** ⇒ 复原后的源码 mtime 比注入构建更旧，cargo 判定「未变更」而
**复用上一次注入的二进制**，于是「基线」跑出的是上一组注入的结论（本轮曾两次把这类陈旧结论误读为真缺陷）。
改用 `fs.writeFileSync(内容)` 并核对**内容哈希**后，前后两次基线均为 **36 通过 / 0 失败**。
⇒ 注入验证的每一步都要问：**这次跑的二进制，真的是我刚写下的那份源码编出来的吗？**

**明确不做**：不把预算初值回放调优成定值、不写 ADR 数值、不接 runtime、不把记录真正追加进 `nai-storage` 哈希链、
不执行 §47.2 六类回归（那是能力包 **H16**）、**不新增契约名**、**不新增 registry 条目**、不改任何既有 `status`、
不改基线文本。**边界（如实）**：H01/H02 **保持 `pending`**——交付的是**判定面**；
故报告 MUST NOT 出现「装配器已接入」「已不依赖当时内存」这类运行期断言。
**本轮不新增条目 ⇒ 不产生新的基线偏差**（W68 登记的 B07 偏差仍未收录——**已在 W71 收录**，见 V3.10 §58.2 的 E1–E3）。
见 docs/kernel.md（W69 段）、docs/reports/2026-10-12-w69-context-assembly.md、
openspec/changes/archive/2026-10-12-w69-context-assembly/。

### W70 瞬态宿主条件（V3.11 §5.9、§12.2、§20.6、§46.3；runtime 装配根与会话共享）

W70 **不是**一次「清 flaky CI」的运维动作。触发是 W69 收尾时登记的两族 CI 红
（run `38200252090` @ `a9f9aea` 的 ubuntu 失败落在 `engine.rs::killed_keeper_is_halted_by_the_engine_and_the_chain_survives`；
run `38201171920` @ `87dce31`——**只改一个 `.md`**——的 windows 失败落在 `pipeline.rs` 4 个用例的
`provisioning store unavailable: storage: DEADLINE_EXCEEDED`）。核实后它们**不是同一类模糊的「不稳」，
而是两个可机械证明的缺陷，其中一个**是安全缺陷**。W48 报告 §5 第 2 条早已把这件事留给单独一轮
（原句：**「Windows 侧 `nai-runtime` 的时序脆弱未修…需单独一轮。」**），本轮就是那一轮。

治理判据（写进规格，可判定）：**宿主的瞬态条件（锁竞争、慢盘）MUST NOT 改变被测系统的结论。**

**缺口一（安全）**——共享 `Arc<Mutex<Session>>` 上的 `try_lock` 被当成故障：

1. `ControlApi` 与装配根共享**同一个** `Arc<Mutex<Session>>`，两侧取锁都是 **fail-fast 的 `try_lock`**。
2. `DrillGuard::tick` **每 5 ms** 调一次 `live_epoch()`（`DRILL_POLL`）。
3. 旧 `live_epoch()` 以 **`?`** 上抛取锁错误 ⇒ 一次**微秒级、会自清除**的竞争被当成**故障**。
4. `tick` 视其为致命 ⇒ 闭包返回 `Err` ⇒ **`supervise` 结束监督循环** ⇒ 此后 keeper 被杀
   **再也不被检测、不被撤权**。
5. `Session::revoke` 自身**只剩 `actor_allowed` 一个前置**（本场景恒 `true`）⇒「撤权被跳过」**只有取锁一条机械通道**
   ⇒ 上述链条是**唯一可能的原因**，可以照它修。

另有一处**巧合式的掩盖**：既有规格的「续租失败不得静默退出」只有**一次性补偿 `halt`**（在错误出口）满足，
而那个出口**用的是同一条 fail-fast 取锁**⇒「到底撤没撤权」取决于**第二次**取锁运气。

**修法 = 一个预算一份定义（ADR 0027）**：

| 路径 | 预算 | 耗尽语义 |
|---|---|---|
| 撤权/故障处置（`observe`/`control`/`enforce_duty`） | `SESSION_REVOKE_BUDGET` = **250 ms**，有界重试 | **显式 `Conflict`**——撤权**要么落地、要么被报告**，**绝不静默跳过** |
| epoch 存活轮询（`live_epoch`） | `SESSION_POLL_BUDGET` = **5 ms**（一个 tick） | **`Ok(None)`** = 「本 tick 未观测到」，**不是故障** |
| bring-up（`prepare` 的 `Store::open`） | `BRINGUP_OPERATION` = **30 s** | 具名失败，**文案带已耗时** |
| 逐次调用 | `SHORT_OPERATION` = **5 s**（**不变**） | 同上 |

`live_epoch()` 签名变 `Result<Option<u64>>`：`Some` 正常／`None` 竞争／**`Err` 仅锁中毒**；
`SessionLockMiss { Busy, Poisoned }` 就是为此存在——把「没看到」与「出错了」在**类型上**分开。

**缺口二（预算与工作量不匹配）**——`Store::open` 在**全新数据根**上跑**全链迁移**
（`migration.rs` 的 `VERSION = 7`，7 个脚本、每段 `BEGIN IMMEDIATE` + `COMMIT`），连接以 **`synchronous=FULL`** 打开，
却曾用**逐次调用**的 `SHORT_OPERATION` = 5 s ⇒ 慢盘上 `DEADLINE_EXCEEDED`。
ADR 0004 明确 **30 s 是上界**（`Operation::new` 对 `timeout > 30s` 直接返回 `InvalidArgument`），
ADR 0038 称其为**安全层不变量**，同仓做**同类迁移工作**的既有取值一律 **30 s**
⇒ 新增 `BRINGUP_OPERATION = 30 s`（`pub`，`apps/engine` 的 `--provision` **复用同一常量**，一处定义）。

**被否决的三个替代**（写进 design、各给理由）：①把 `session_lock` 一律改成阻塞 `lock()`——
把一次宿主竞争**换成一次无限等待**，控制面 `status` 会挂住，而且它「修」的是运气不是语义；
②只在测试夹具里等——被修的是**产品代码对瞬态条件的处理**，不是调度假设，夹具等待会让缺陷在生产里原样保留；
③`--test-threads=1`——把「并发下必须成立」的断言降级成「串行下成立」，是**放宽**而不是**修**。

**刻意的不改**：`apps/engine/src/pipeline.rs` 的两处 5 s **保持原样**——它们在**已打开的 store** 上读、**不含迁移**
⇒ 这是判据的**结果**，不是遗漏。

测试全部落在**既有**目标内 ⇒ **不新增测试目标**（目标数不动，增量只能来自项数）：

- 3 条行为测试（`crates/nai-runtime/tests/composition.rs`）：**持锁 20 ms 期间** `control(Mute)` **仍返回 `1`**；
  持锁 30 ms 期间 `live_epoch()` 为 **`None` 而非 `Err`**；**正向前置**「撤权后 epoch 确实变化」
  ——否则「恒报未观测到」也能让测试变绿。
- 2 条策略钉死（`crates/nai-runtime/src/composition.rs`）：逐项钉死四个常量与三条不等式
  （`BRINGUP > SHORT`、`POLL ≤ REVOKE`、`REVOKE ≤ BRINGUP`）；在**新建临时目录**上真调 `open_bringup_store`
  并断言 `remaining() > SHORT_OPERATION`。
- **持锁时序用 mpsc 握手**确保持锁**先于**被测调用——**不用 sleep 竞猜**。
- **五组非空洞注入**各自被抓住、源文件按**内容哈希**复原为 `IDENTICAL`：去掉重试截止／把 `Ok(None)` 改回 `Err`／
  `BRINGUP` 退回 5 s／`bringup()` 换 `SHORT_OPERATION`／`open_bringup_store` 内换 `short()`。
  **第五组是发现「只钉常量对『常量对、调用点接错』是瞎的」之后补的**——把 `open_bringup_store` 提成
  `prepare` **真正调用**的那个函数，再让它进测试。

**明确不做**：不改断言、不放宽窗口、**不用 sleep 掩盖**、不改 CI workflow、不降 `--test-threads`、
不改 `apps/engine/tests/*` 任何一行、**不新增契约名**、**不新增 registry 条目**、不改任何既有 `status`、不改基线文本。

**边界（如实）**：`apps/engine/tests/engine.rs` 是 `#[cfg(target_os = "linux")]` ⇒
**本机 Windows 门控不能证明 Linux 侧修复**，只能**以 CI 结论为准**；family 1 的链是**从源码读出的、
不是从失败日志读出的** ⇒ **不主张**已复现当时失败，只主张「这条链存在且被消除」。
**本轮不新增条目 ⇒ 不产生新的基线偏差**（W68 的 B07、W67 的「影子在 R1 无落点」仍待升基线轮次收录——**两项均已在 W71 收录**，见 V3.10 §58.2 的 E1–E4）。
见 docs/kernel.md（W70 段）、docs/reports/2026-10-12-w70-transient-host-conditions.md、
openspec/changes/archive/2026-10-12-w70-transient-host-conditions/。

### W71 升基线 V3.10（订正版；V3.10 §58、§56.2；跨轮次累积偏差的收录）

W71 **不是**范围版，也**不是**设计变更，而是**订正版**——把 **W68** 与 **W67** 两处**已登记、但没被收进基线**的偏差并入正文。
§56.2 明写「**纯追加 MUST NOT 被用来让矛盾长期存续**」：一旦已声明与已登记冲突，追加只是把矛盾**登记第二遍**。
W65 那一次是**同步发现**的偏差，本轮是**跨两轮累积**的，故这是 §56 规则的第一次跨轮次适用。

| # | 偏差 | 实测 | 登记于 |
|---|---|---|---|
| 1 | 注册表新增 **B07**（`r1`）后，§48.5 仍写「B \| B01–B06」、§57.3 计数行仍写「= **87**」 | 注册表 **88** 条、`B01–B07`；`slice_0` 10 / `r1` **37** / `r2` 9 / `capability_pack` 29 / `r3` 1 / `experimental` 2 | W68 |
| 2 | §5.9／§47.3 的爬坡是**四段**（含影子），而 §1.4／§17.3 把影子列在 **R2**、`SessionMode` 只有**三**取值 | **H18 的默认启用集合里有一个阶段在 R1 内没有落点** | W67 |

**升版而非就地改 V3.9**（先例）：`V3.9.md` 与 `V3.8.md` **各自只有一次提交**（升基线那次）；
W54 的 A12、W55 的 A13 **没有**就地改 V3.8，而是在 **W65 升基线**时被收录进 §48.5 ——**每次基线变更生成新文件**，
被 ADR 批准的基线产物因此**保持不可变**。产出 `docs/直播数字人 + 智能体系统技术方案 V3.10.md`
（**追加第 58 章**）与 **ADR 0049**（只替代 ADR 0048 的决策 1；决策 2–5 继续有效）。

**订正清单 E1–E5 = 14 行 1:1 行替换**（+ 追加 §58 共 66 行；另有 3 行属**版本标识**：标题／副标题／§0.2）：

| # | 位置 | 订正 |
|---|---|---|
| **E1** | §1.4、§17.3、§48.5（R1 汇总）、§51.2、§57.3（R1 行），**5 行** | R1 清单增补 **B07** |
| **E2** | §48.5 家族索引 B 行 | 「B \| B01–B06 \| 第 16 章」⇒「B \| B01–B07 \| 第 16 章、53」 |
| **E3** | §57.3 计数行 | `r1` **36** ⇒ **37**；合计 **87** ⇒ **88** |
| **E4** | §40.4（纪律 5）、§47.3、§48.5（H18 行），**3 行** | 「默认只在**演练、影子与人工辅助**下启用」⇒「默认只在**受监督自动以外的阶段**下启用」 |
| **E5** | §56.2 的「活文档 ⇒ 就地订正」行，**1 行** | 把订正对象写成**四情形**（见下） |

**E4 = 与阶段枚举解耦**（否决「给影子一个 R1 契约落点」——那会扩张 R1 范围、与 V3.9 的收缩反向；
也否决「只登记不订正」——与 §56.2 冲突）。改为**相对 §5.9 爬坡位置的判据**（「受监督自动**之前**」）：
① 语义等价（四段里受监督自动之前的恰为演练／影子／人工辅助）；② 对缺少某阶段的发布线自洽——
R1 内影子**只是不存在**，MUST NOT 为对齐枚举而新增；③ §5.9 的四段定义不改（它描述**设计目标**，不是**默认启用集合**）。
**代码侧一字未改**：`session_mode()` 早已对 `Shadow` 返回 `None`，由 `every_stage_but_shadow_has_a_session_mode` 钉死。

**E5 的四情形（写入 §56.2，据 W65 实测得出）**——**首版判据曾被先例当场否证**（初稿把「章节指针」也列为保留，
而 W65 升版时**把活文档里的 `V3.8` 一并更新**、含轮次记录标题；存活的 `V3.8` 只有三类、**零例外**）：

| # | 情形 | 处理 |
|---|---|---|
| ① | 声明「**当前基线是哪一份**」的位置 | 更新；或改为**不写死版本号**（§56-C9 先例） |
| ② | **`§NNN` 的版本前缀**（行号契约的锚点） | **随基线更新**（W65 先例） |
| ③ | **关于某一版的陈述** + **该版专属版本章节**（§39／§48.1–48.2／§56／§57.1/§57.2/§57.4/§57.5） | **保留原文** |
| ④ | 代码 doc-comment 版本前辍 | 不改（§56.2 末行） |

**两种错都必须避免**：留下旧版前缀 ⇒「`§NNN` 按当前基线解析」失去锚点；一并替换关于某一版的陈述 ⇒
**「V3.9 做的收缩」被误记为「V3.10 做的」**（29 条能力包标记因此改写为 `【V3.10 §51.3（V3.9 收缩：自 rN 降为能力包）】`）。

**契约证据**：整文件 **15 hunk = 14 个 `c` 型（全 1:1）+ 1 个 `a` 型（`2647a2648,2713`，尾部追加）+ 0 个 `d` 型**；
限定**前 2647 行**则**只有 14 个 `c` 型、零 `a`·`d`** ⇒ **行号一一对应，全部既有 `§NNN` 引用继续有效**。
规模 **2647 行 / 216,450 字节 ⇒ 2713 行 / 226,578 字节**。**注意**：整文件**不是**「零 `a`·`d`」——
那条尾部追加 hunk 必须一并报出，否则会被 `diff` 直接推翻。

**门控**：`cargo xtask check --profile full` **EXIT=0**，测试目标 **91（73 `Running` + 18 `Doc-tests`）**、
**858 passed / 0 failed**——与 **W70 完全相同**（本版零 Rust 与零测试改动 ⇒ 数字不变本身就是「未误改代码」的旁证）。

**活文档同步**：改动前实测 **179 行 / 206 处**（口径：`*.md`/`*.yaml`/`*.json`/`*.mjs`/`*.ts`/`*.tsx`/`*.rs`，
排除 `docs/reports/`、`docs/adr/`、`openspec/changes/archive/`、`.workbuddy/` 与基线文档本身）；机械处理 **102 行**（73 直改 + **29 能力包标记改写**）+
部分保留行 **19 处** + 顺带订正 **9 处**。**改完做幸存者审计**（残留 **89 行 / 22 文件**，逐个交代去留），
审计抓到 **1 处误留**（`openspec/specs/platform-matrix/spec.md` 的基线声明实为**当前指针**）。
**顺带订正**：W68 把计数行写作「**§57.4** 的计数行」，实测在 **§57.3**——同一条错引**散落四处活文档**
（`tests/registry.yaml`、本文件、`docs/kernel.md`、`docs/development-plan.md`；**C6 教训重演**）；
W68/W69/W70 段里「仍待升基线轮次收录」共**六处**加**闭环标记**（原文保留，只加标记）。

**明确不做**：不加能力、不改发布线归属、不改任何 `status`、不新增契约名（能力目录零漂移）、不重排 W 编号、
**不给 §16 的验收注册表增行**（插入行会使其后全部行号位移、破坏 `§NNN` 引用契约；故新增项只在 §48.5 家族索引与
§57.3 发布线正本增补，与 A12–A14 的处置一致）、不改 `docs/reports/` 与 `openspec/changes/archive/`、
不改轮次记录里指向**该轮产物**的文件路径（W65 的 `…V3.9.md` 与 W56 记录仍留 `…V3.8.md` 同理）。
**本版新登记两处门控缺口（只登记、不裁定）**：① `admission-registry` 的「逐号在册」断言写「**每个**『基线已定义完整编号段』的家族」
而 §48.5 有**九族**连续段、门控只断言 M 与 H（括号是「当前清单」还是「举例」，两种读法都成立，且**先于本版存在**）；
② **基线文本与注册表的一致性没有门控**——本版所修的「§57.3 计数 ≠ 注册表实测分布」**没有任何断言**会发现，
靠人读发现、跨了 W68 → W71 两轮。见 docs/reports/2026-10-12-w71-baseline-v310-errata.md、
openspec/changes/archive/2026-10-12-w71-baseline-v310-errata/。
**两处缺口均已由 W72 关闭**（② 补上对账门控；① 裁定括号为**示例**、判据来源改为 §48.5 逐行读出），见下节。

### W72 冻结基线与注册表的对账门控（缺②、缺①的关闭；并订正 W71 的 E5 残留）

W72 **不改基线正文**（`… V3.10.md` 零改动）、**不改任何 `acceptance` 条目**、**不改 Rust 行为**——
它是**补断言**，不是升基线。校准的正是 W71 报告里如实登记的两处门控缺口。

**断言为什么是「对账」而不是「相等」**：基线被 ADR 批准后**不可变**，实现轮次**没有权限**改 §57.3。
若写成 `§57.3 == 注册表`，W68 那样的合法新增会立刻让门控变红，只能「跳过门控」或「开一次升基线轮次」——
**这不是严格，是死锁**。故写成：

```
在册 == 基线声明 ∪ baseline_deviations
```

**台账不是豁免**，它有三条性质：**具名**（id + 登记轮次 + 非空理由）、**相称**（MUST 对应一处**真实**差异；
张冠李戴时真实差异与条目**两条都失败**）、**自清理**（基线收录后不清空 ⇒ 按**陈旧条目**失败）。
⇒「**已登记但尚未收录进基线**」从 W68→W71 期间那种「在报告、`notes`、活文档里各写一遍」的**说法**，
变成**可判定**的状态（ADR 0042 决策 4 的同一形式：在册≠测得 ⇒ 已声明≠已收录）。

**缺口①的裁定**：`admission-registry` 需求里「（当前为 M01–M18 与 H01–H18）」是**示例**，不是枚举；
「基线已定义完整编号段」的取值 MUST 从 **§48.5 的家族索引逐行读出**（当前 **9** 族，全部是连续段）。
`COMPLETE_FAMILIES`（M 18／H 18）**保留**——它与新判据挡的不是同一件事：前者挡「§48.5 自己被改小」，
后者挡「注册表少了 §48.5 声明的某一项」，且**只有后者覆盖 S/R/A/B/O/P/X**。
⇒ 缺口①不需要单独的实现轮，由②的实现顺带关闭。

**核实中发现并订正的第三件事（不在原计划内）**：**W71 的 E5 订正没有传播到它自己的规格差量。**
基线 §56.2 情形②明写「`§NNN` 的版本前缀随基线更新」，而 W71 新增的 `spec-driven-delivery` 需求却写
「版本归属引用（形如「V3.9 §51.3」）MUST NOT 随基线号变更」，其场景甚至要求「改写为 V3.10 §51.3 **MUST 被拒绝**」
——与 §56.2 情形②、也与 W71 **自己落地的 29 条能力包标记**（`【V3.10 §51.3（V3.9 收缩…）】`）**直接相反**。
处置按 §56.2 的**载体**分界：`openspec/specs/`（**活的主规范**）**就地订正**为四情形；
`docs/adr/0048` 的勘误段与 `docs/adr/0049` 的「后果」行**追加勘误段**（原文不改写）。

**非空洞（本轮的关键守卫）**：门控最可能变成废纸的方式不是「漏改」，而是**解析悄悄落空**——
§57.3/§48.5 的章节号被改，解析出空集。故：① 定位按**章节号**（改措辞不影响、改号码必然失败，有专门用例）；
② 「该节找不到」与「设计漂移」**分开具名**；③ 家族列与发布线列**逐项钉死**（`familiesInOrder`、`rowLabels`）；
④ 用例锚点用 `replaceOnce`（**恰好命中一次**）——它**当场开火**：`\`r1\` **37**` 命中 **2** 处，
因为 **§58.5 逐字复述了同一条计数行**（**W71 已把这条断言用散文写在 §58.5**，本轮把它变成机器可读的）。

**交付**：`scripts/contracts/baseline.mjs`（定位/解析/对账/指针自洽）＋ `tests/registry.test.mjs`（**4 → 11** 用例，
经 `engineering_checks` 自动进入 `portable` 与 `full`，**不需要新的 xtask 步骤**）＋
`tests/registry.yaml` 的 `baseline_deviations`（**当前为空**——台账为空本身就是「W71 已全部收录」这条断言）＋
**ADR 0050** ＋ 三份规格差量（`admission-registry` MODIFIED+ADDED、`spec-driven-delivery` MODIFIED）。
**注入验证 21 组**（19 组期望报错、2 组期望通过）：改计数／改合计／漏线／多出未知线／移线／新增 id／删除 id／
缩范围／扩范围／章节号被改（§57.3 与 §48.5）／陈旧条目／id 非法／缺轮次／缺理由／非列表／重复／张冠李戴／
指针失配／指针缺失／逃生通道可用。

**门控**：测试目标数与通过项数与 **W71 相同**（本版零 Rust 改动）；Node 用例 **4 → 11**。
**明确不做**：**不做「规格 vs 基线」的自动对账**（§56.2 四情形是**语义**规则，只能靠读）——
残余风险的证据就是本轮本身：**规格里的矛盾是靠人读发现的**。见
docs/adr/0050-baseline-registry-reconciliation.md、docs/reports/2026-10-12-w72-baseline-reconciliation-gate.md、
openspec/changes/archive/2026-10-12-w72-baseline-reconciliation-gate/。

### W73 注册表形状契约接电与台账存活期（并订正 W72 对 A12–A14 的表述）

W73 从 W72 遗留第 2 条（§56.2「紧接着」vs「跨轮次收录」**未裁定**）出发，**核实之后改了计划**：
核实发现的问题比那条遗留更靠前——**规格里写着「schema 校验 SHALL …」，而那句校验根本没跑。**

**核实一（本轮的核心发现）**：`schemas/tests/registry.schema.json` **不被任何执行体消费**——
不在 `scripts/contracts/validation.mjs` 的 `sources`（11 份）、全仓无 `include_str!`、无任何 `*.mjs` 读它。
`tests/registry.yaml` 的形状一直只由 `registry.mjs` 的**手工检查**把关，而手工解析**对未知键静默忽略**。
实测把注册表喂给该 schema：**只报一处错**——**W72 新增的 `baseline_deviations` 未被声明**；
其余 88 条、六条发布线、映射段**全部通过**。⇒ 该 schema **不是文档残留**（它随 W65/W71 同步更新过 `description`），
它只是**从未接电**；**一次合法的往前推进一步，就把一个无人检查的契约变成了假契约**。
这也意味着 `admission-registry` 里「schema 校验 SHALL 拒绝重复编号、未知 release_line/status…」与
「MUST NOT 出现『注册表写得进去、schema 认不出来』的状态」两条要求，此前都是**靠手工检查代偿**的。

**核实二**：`schemas/**/*.schema.json` 逐个查消费点，孤儿**不止一个**（`tests/registry`、`rag-golden/case-set`），
而「有没有人读这份 schema」**当前没有任何守卫**——它能默默发生两次，就会发生第三次。
（`config/upgrade-package` 是**只在 Rust 侧**被消费的实例：`nai-upgrade-core/src/manifest.rs` 的 `include_str!`
——故消费口径 MUST 含 Rust 扫描，只扫 Node 会把它误判成孤儿。）

**核实三（裁定「紧接着」的证据）**：把三种历史实例的「偏差出现 → 基线收录」逐条读出来，
**三条全部在下一次升基线轮次被收录，零跳过**（爬坡 W67→W71；B07 W68→W71）。
**并订正 W72 报告的一处表述**：A12–A14 **不是**「跨轮次收录」，而是——W54/W55 新增 A12/A13/A14 时
**活索引 §48.5 还不存在**（§48 是 V3.8 的新章），当时存在的 `§39.5` 是 **V3.7 的版本专属章节**（§56.2 情形③），
它的落后是**设计**；偏差是**在 W56 出现的**（那份活的跨版本索引**诞生时就落后于注册表**），
直到 W65 被收录，中间**九轮**没有任何记录说「这里不一致」⇒ 它属于**静默漂移**，
正是 W72 的对账门控今天会当场抓住的形状。

**裁定与交付**（[ADR 0051](docs/adr/0051-registry-schema-wiring-and-ledger-lifetime.md)）：

1. **注册表接电**：schema 补 `baseline_deviations` 定义（并列入 `required`——「缺键」与「空台账」不是一回事）；
   `validation.mjs` 导出 `AJV_SOURCES` 并把注册表纳入；`tests/registry.test.mjs` 断言注册表满足其 schema。
   **只改 `description` 不算修复**，**删掉 schema 也不算**：三条既有规格要求以它为前提。
2. **形状层与语义层只有一份定义**：两者在 **id 家族 pattern / 发布线枚举 / 状态枚举**三处重叠，
   每处 MUST 逐字一致，不一致时**指名是哪一处**。⇒ 重叠允许，漂移不允许（不是「同一检查第二处定义」——
   重叠的三处加了一致性断言，而在此之前它们**已经**是三处，只是无人比对）。
3. **台账的寿命**：条目扩为 `{id, since_baseline, registered, carried_at?, reason}`，
   **不变式：任一条目的 `since_baseline` MUST 等于当前基线版本**。跨版只有两种处置：
   **收录**（删条目）或**显式结转**（改 `since_baseline` + 记 `carried_at` + 非空理由）。
   ⇒ 把台账的**长度**换成**寿命**：长度可以靠沉默增长，寿命不能。
   **为什么锚「下一次基线修订轮次」而不锚「N 轮」**：字面「同一轮内」**不可行**
   （基线被 ADR 批准后不可变 + 就地订正 MUST 1:1 行替换 ⇒ 改 §57.3 只能发生在一次专门的升基线轮次里）；
   而轮数是**外部观察值**（§48.7 的同一纪律），取 3／9／11 都无依据。
4. **`schemas/` 消费守卫**：每份 `*.schema.json` MUST 要么被**执行体消费**、要么在**具名例外清单**里附非空理由；
   消费关系 **MUST 派生**（校验器清单 / Rust 的 `include_str!` / 代码里的路径字面量），MUST NOT 写死。
   本轮具名例外**恰一条**：`rag-golden/case-set`（执行体是 `golden.rs` 的 `parse_case_set`，产出稳定 token；
   再接一份 schema 会形成同一形状的第二处判定，两份判定的失败措辞不同）。
   **扫描只认代码文件、且去掉整行注释**——不这么做，注释里提一句路径就会被算成「有消费者」，
  守卫会在**危险的方向**上变瞎（删掉真正的加载点、只留注释，孤儿照样判不出来）。

**非空洞**：新增注入 **27 组**——schema 形状 **8** 组（未知顶层键／未知条目字段／坏 id／坏线／坏状态／
空 `acceptance`／少一个 D 键／缺台账键）、一致性断言 **3** 组（改小 `ID_RE`／删一条发布线／加一个状态）、
台账寿命 **8** 组（跨版存活／`carried_at` 不晚于登记轮次／坏 `carried_at`／缺 `since_baseline`／
缺版本入参／坏版本串 ⇒ 全部期望失败；**合法结转**与**相称条目** ⇒ 期望通过）、
`schemas/` 消费守卫 **8** 组（删一项消费／给已消费的加例外／例外缺理由／例外指向不存在的文件 ⇒ 期望失败；
注释里的路径不算消费 ×3、代码里的仍算数 ⇒ 期望通过）。
**各组都被对应守卫抓住**；并保留「扫描非空且命中 Rust 侧消费」的守卫，防守卫因扫描恒空而恒真。
W72 的 **21 组**注入继续有效（同一文件）。实测消费报告：**14 份 schema / 13 份被消费 / 1 份具名例外**，
扫描 **240** 个代码文件。

**门控**：新守卫经 `engineering_checks` 自动进入 `portable` 与 `full`（`tests/registry.test.mjs` 与
`tests/contracts/schema.test.mjs` 都已在其中）⇒ **不需要新的 xtask 步骤**；
**不改基线正文、不改任何 `acceptance` 条目（88 条、六条发布线、全部 `status` 原样）、不改 Rust 行为**，
台账**仍为空**（W71 已把三处偏差全部收录）。见
docs/adr/0051-registry-schema-wiring-and-ledger-lifetime.md、docs/reports/2026-10-12-w73-registry-schema-and-ledger-lifetime.md、
openspec/changes/archive/2026-10-12-w73-registry-schema-and-ledger-lifetime/。

### W74 消费守卫的边界（扫描集、语言闭包、来源自负其责与单一生成清单）

W74 从 W73 遗留第 2 条（`generate.mjs` 用**模板路径**消费 10 份 schema、其清单是 `AJV_SOURCES` 的子集）出发，
**核实之后改了计划**：核实把 W73 消费守卫的**三条声称里的两条证伪了**，并牵出另外两条缺陷与一条宿主约束。

**F1（非空洞断言自我满足）**：守卫写着「三条派生来源各自都被行使」，判据是「`platforms` 与 `registry`
都在消费集合里」——而这两份**都在 `AJV_SOURCES` 里**，所以那句话**只由 Ajv 清单独自满足**。
实测把 `for (const pattern of [rust, literal])` 改成 `[rust]`：**照旧 13/13 绿**
⇒ 整条路径字面量来源可以被删除，而**没有任何断言变红**。

**F2（扫描的是工作区，不是仓库；最严重）**：放一个**未跟踪**的 `.tmp-fake-consumer.mjs`（内含一句
schema 路径字面量），一个真孤儿立刻被洗绿（`13/13 pass`）。「有一份 schema 没人读」是本守卫**唯一**
要抓的东西，而一句临时脚本就能让它失效。

**F3（两个缺陷互相掩盖）**：`CODE_EXTENSIONS` 缺 `.yaml`/`.js`，而
`stripCommentLines(text, file.endsWith('.yml'))` 只对 `.yml` 用 `#` 注释语法。**唯一暴露该缺陷的文件**
（`tests/registry.yaml`，正是 W73 写下「注释里的路径不算消费」那句注释的地方）**因为扩展名叫 `.yaml`
而根本不在扫描集内**——两个缺陷各自躲在对方的盲区里。

**F4（第二处清单）**：`validation.mjs` 的 `AJV_SOURCES`（12 条）与 `generate.mjs` 的 `jsonSources`（10 条）
**互不校验**；生成物 `packages/contracts-ts/src/generated/config/*.ts` **已入库**
⇒「生成面 ⊆ 校验面」这条**可以判定却无人判**。

**F5（宿主约束，并订正一处归因）**：本机 node 对 `git`（裸名与绝对路径）、`process.execPath`、`cmd.exe`、
`sh` **一律 `EBUSY`** ⇒ **宿主禁止 node 派生任何子进程**（原先把 `schema --check` 的 `EBUSY` 归因为
「node 自旋 node」，本轮订正：AGENTS.md 就地订正、五份历史报告追加勘误段）。
⇒「用 `git ls-files` 取仓库文件集」这条路**不可行**；**W75 订正**：不可行的只是「**在 node 里派生 `git`**」——`.git/index`（DIRC）可直接**纯 JS 解析**，F2 已按 [ADR 0053](docs/adr/0053-repository-scoped-evidence.md) **根除**。

**裁定与交付**（[ADR 0052](docs/adr/0052-consumption-guard-boundaries.md)）：

1. **扫描集是结构性的**：从仓库根递归、**跳过所有以 `.` 开头的目录**，再叠加**具名排除**（恰 4 条：
   `node_modules`/`target`/`graphify-out`/`dist`），且**每条 MUST 被 `.gitignore` 忽略**——
   「它不是仓库的一部分」就是那条理由，`.gitignore` 就是证据。**无害性断言**：跳 dot 段与不跳 dot 段的
   **消费集合 MUST 相同**（实测唯一差异是 `.github/workflows/foundation.yml`，其非注释行不含任何 schema 路径）。
2. **语言闭包**：`CODE_LANGUAGES`（扩展名 → 注释语言）**派生** `CODE_EXTENSIONS` ⇒「新增可扫描扩展名」与
   「声明它的注释语言」是**同一件事**；`.yml` 与 `.yaml` MUST 一起覆盖。
3. **来源自负其责**：来源收为**两条**（Ajv 清单、Rust `include_str!`），判据从「各自都被行使」换成
   **独有覆盖**——Ajv 独有 **5 份**（含 `tests/registry`、`platform-matrix/platforms`）、
   Rust 独有**恰 1 份**（`config/upgrade-package`）。**路径字面量来源被移除**：它零独有覆盖，
   且是唯一会读任意扩展名文件的来源（F2 的**全部入口**）；按「假阳性与假阴性不对称，永远选安全方向」取舍。
4. **生成面与校验面共用一份清单**：`validation.mjs` 导出 `TS_GENERATED_SOURCES`，`generate.mjs` **删除**
   硬编码 `jsonSources` 改为导入；断言 **生成面 ⊆ 校验面 + basename 互异 + 生成目录与清单双向相等**
   ＋防回退（生成器源码 MUST 引用该清单）。这**闭合了 ADR 0051 遗留 2** 自己提出的修正方向。

**验证**：`node --test tests/contracts/schema.test.mjs` **15/15**（13 → 15）；**六组注入逐项开火并还原**，
外加重现 W73 的洗绿攻击：**真孤儿 + 未跟踪的伪造消费者 ⇒ 现在仍被抓**（W73 时被洗白）。
**门控路径不变**（新断言写在既有测试文件里，已经在 `engineering_checks` 内）⇒ **不需要新的 xtask 步骤**；
**不改基线正文、不改任何 schema 文件、不改任何 `acceptance`、不改 Rust 行为**，生成物**零漂移**。见
docs/adr/0052-consumption-guard-boundaries.md、docs/reports/2026-10-12-w74-consumption-guard-integrity.md、
openspec/changes/archive/2026-10-12-w74-consumption-guard-integrity/。

### W75 仓库受限的消费证据（判据的输入必须等于它断言的对象；并订正 W74 的「不可行」陈述）

出发点写的是 **W74 §八遗留 1**（「彻底消除需要『仓库文件集』这个输入，而取它需要派生 `git` 子进程」），
**核实把这条陈述拆成两半**：①「node 派生 `git` 会 `EBUSY`」**成立**（W74 实测，本轮不动）；
②「**因此取仓库文件集不可行**」**不成立**——`.git/index` 是 Git 的 DIRC 格式，**纯 JS 可读**：
实测 `magic=DIRC version=2 entries=922`、**完整解析 922/922**。探针同时钉住三者关系：工作区扫描集 **282**、
索引里代码文件 **283**、【A】工作区扫描到但不在索引 = **0**、【B】在索引但不在扫描集 = **1**（dot 目录的
`.github/workflows/foundation.yml`）、【C】工作区 ∩ 索引 = **282** = 扫描集。
另牵出 **F6**：`codeFiles()` 的 `dir.replace(/^\.\//, '')` 对初始 `dir === '.'` 不生效 ⇒ **根目录文件路径带 `./`**、
子目录不带；当前无可观测影响，但**任何「与仓库路径集比较」的判据都会因此假阴性**——而本轮正是要做这个比较。

**裁定与交付**（[ADR 0053](docs/adr/0053-repository-scoped-evidence.md)）：

1. **仓库文件集 = `.git/index` 的路径集**：新增 `scripts/contracts/git-index.mjs`（纯 JS，DIRC v2/v3/v4；
   `.git` 为文件时读 `gitdir:` 指针）。**为什么是「已跟踪」而不是「git 会发布的内容」**：
   `git ls-files --cached --others --exclude-standard` **含未跟踪文件** ⇒ 那个伪造消费者也在集合里，
   **修不了 F2**；守卫跑在**提交前**，「未跟踪」正是「尚未承诺为仓库内容」。
2. **消费证据的来源 MUST ⊆ 已跟踪路径集**（`repositoryErrors`）——F2 的**精确**修复。
   **约束「证据来源」而不是「扫描集」**：后者会让任何一个尚未 `git add` 的代码文件把门控**假红**
   （闭环在提交时才 `git add`），而攻击面恰在证据来源上，故精确打击零假阳性。
3. **扫描集路径是 POSIX 相对路径**（修 F6）＋「MUST NOT 有 `./` 前缀」断言。
4. **索引不可解析 MUST 失败**且**说得清是哪一类**（`missing`/`bad-magic`/`unsupported-version`/
   `truncated`/`corrupt`）——「判据的输入等于被断言的对象」是这条守卫的前提，前提缺失不得静默退化。

**实测的一条更重要结论**：W74 之后 F2 **难以洗绿**，但挡住它的是一条**形状**断言
（`exclusiveTo(RUST)` 的具名钉死——孤儿的加入让独有集合多一员），**不是**关于**来源**的断言 ⇒
**那是副作用，不是设计**：任何人**合法地**让一份新 schema 只被 Rust 消费，都会让那条钉死变红，
而最顺手的处置就是**把它改松**。实测（V1c）放宽形状断言后 `repositoryErrors` **独立兜住**；
两条都停用（V1b）则 **17/17 全绿**（洗绿成立）。⇒ **「碰巧挡住」不能算挡住。**

**验证**：`node --test tests/contracts/schema.test.mjs` **17/17**（15 → 17）；**注入逐项开火并还原**
（V1a/V1b/V1c、V2a 索引缺失、V2b magic 不符、V3 路径前缀），外加解析器字节流单测
（v2/v3/v4 ＋ 四类失败 ＋ 空索引）。**门控路径不变**（解析器**不是**测试文件 ⇒ `engineering_checks`
零改动）；**不改基线正文、不改任何 schema 文件、不改任何 `acceptance`、不改 Rust**，生成物**零漂移**。
**订正既有陈述**：`AGENTS.md` 就地 1:1 订正；`docs/adr/0052`（两处）与 W74 报告 §八遗留 1 追加勘误段。
见 [ADR 0053](docs/adr/0053-repository-scoped-evidence.md)、
[W75 报告](docs/reports/2026-10-12-w75-repository-scoped-evidence.md)、
`openspec/changes/archive/2026-10-12-w75-repository-scoped-evidence/`。

### W76 待裁定项的登记与寿命（悬空的问题也要有寿命）

出发点是一个盘点问题——**「离方案设计和规划任务还有哪些未执行」**。全仓核实后，主干线未达标项（18 条）
与外部阻塞项（5 条）都已登记在案，**唯一一类「有载体而无台账」的是基线四章「待决策与待确认」**
（§18.2、§39.7、§48.7、§57.5，一条**累积链**：后一章沿用前一章全部）。去重后 **34 条**，
分章新增数 **10/11/10/3**。

**核实先行——四条已被基线自身回答**（故正确处置是「已答性核实并关闭」，不是「新裁定」）：
MCP `sampling` 可接受场景 → §46.2「默认禁用 + 三条件」；上下文利用率观测线 → §41.4「本轮**不设**告警阈值」；
`a2a` 契约名是否设立 → §46.5「**不新增** + 三条理由」；能力包打包粒度 → §51.3 已是**三行独立包**。
这与 W67 把「『通过 §47.2 的回归』是全六类还是相关类」核实为**不是偏差**同型。

**裁定与交付**（[ADR 0054](docs/adr/0054-pending-decisions-ledger.md)）：

1. **五态封闭枚举，每态一个必填字段**：`answered_by_baseline`→`basis`、`decidable_in_repo`→`recommendation`、
   `needs_precondition`→`precondition`、`needs_external`→`closure`、`out_of_scope`→`note`。
   把「这条有没有人管」换成可判定的「**那个字段在不在**」——一条没有 `closure` 的外部项与空注释没有区别。
2. **依据 MUST 指回真实存在的章节**：台账里每一个 `§NNN`（含 `introduced_in`／`also_in`）MUST 在基线
   章节标题集合里 ⇒ 写 `§99` 立刻失败。**脚本只证「章节存在」，不证「该章节真的回答了该问题」**——
   后一半刻意留给人读，写在 ADR 决策 2 里。
3. **外部条件写成「取到什么算闭合」**：§52.1 的硬门槛就是「**可留存**的书面答复」——
   「待平台确认」与空注释没有区别。
4. **前置 MUST 真实存在**（registry 验收 id 或基线工作项 `WNN`）——挂在不存在的东西上等于挂在空气上。
5. **覆盖性与分布逐章、逐类写死**（`CHAPTER_COUNTS` 10/11/10/3、`DISPOSITION_COUNTS` 4/5/4/20/1、总数 34）。
   只校验总数对「内容被改小／**被改类别**」是瞎的（W66 教训）。
6. **与 `baseline_deviations` 不合并**：那是「已发生的偏离」，本台账是「未决的问题」，维度不同。

**验证**：`node --test tests/registry.test.mjs` **19/19**（14 → 19）；**文件级注入三组逐项开火并还原**
（F1 依据指向不存在的 `§99` ⇒ 报 `PD-25: references §99, which does not exist in the baseline`；
F2 首次出现章被改 ⇒ **两侧**章计数都报；F3 处置越出封闭枚举 ⇒ 越界 id 与分布两条都报），
测试内另覆盖五态必填字段缺失、前置不存在、删条目、id 重复、陈旧基线五类。
**门控路径不变**（断言**并入既有测试文件** ⇒ `engineering_checks` 零改动）；**不改基线正文**
（§48.7／§57.5 属 §56.2 情形③的版本专属陈述）、**不改 `acceptance`、不改 schema、不改 Rust**。
见 [ADR 0054](docs/adr/0054-pending-decisions-ledger.md)、
[W76 报告](docs/reports/2026-10-12-w76-pending-decisions-ledger.md)、
`openspec/changes/archive/2026-10-12-w76-pending-decisions-ledger/`。

### W77 bring-up 预算的单一归属点（「一个预算一份定义」在跨 crate 场景的落实）

**问题**：`crates/nai-control-api/src/tests.rs::seed_approval_store` 自造
`Operation::new(Duration::from_secs(10))` 去跑 `Store::open`（全新数据根 + 全链迁移）——
**预算与工作量不匹配**，而 **W75 在 CI 上的失败正落在该文件 `:364`**。

**核实否掉了最直觉的修法**：`nai-runtime` **依赖** `nai-control-api`（`nai-runtime/Cargo.toml:17`）
⇒ 反向引用 `nai_runtime::BRINGUP_OPERATION` 构成**依赖环**。而 `nai-storage` 是双方的共同依赖。

**裁定**（[ADR 0055](docs/adr/0055-bringup-budget-single-home.md)）：

1. **定义点归属「工作类别的执行者」**：bring-up 的执行者是 `Store::open`（在 `nai-storage`）
   ⇒ `BRINGUP_OPERATION` **下移**到 `nai-storage`；`nai-runtime` 改 **re-export**
   （`nai_runtime::BRINGUP_OPERATION` 仍可用，`apps/engine` 零改动）。
2. **提取 `Operation::CEILING`**：它既是公开常量、也是 `new()` 的拒绝阈值
   ⇒「bring-up 取 ADR 0004 上界」从**注释里的默契**变成**可判定的等式**。
3. **`SHORT_OPERATION` 不上移**：它**只有一个消费者**；为不存在的问题造公共定义就是假契约（W73）。
4. **同值可以不同名，但每个语义只有一个定义**：bring-up 用 `BRINGUP_OPERATION`，
   演练/冒烟「用满上界」用 `Operation::CEILING`。把「数值相同」与「语义相同」分开——
   否则 upgrade 的演练夹具会被迫自称 bring-up，那是**语义错位**，比重复更坏。

**11 个复制位点（落在 9 个文件里）、本轮共改 14 个文件**：其中只有 **1 个是错的数值**（10 s），
其余是**正确的数值、错误的位置**
——所以本轮修的是**结构**，不是数字。最强的证据是 `live_provider_smoke.rs` 与 `golden_eval.rs`
各有一个**私有 `const OPERATION_CEILING`**：那正是 `composition.rs` 注释所禁止的
*"a second, drifting copy"* 的**活标本** ⇒ **「复制数值」与「复制定义」是两种复制，只有后者会漂移**。

**判据（7 组非空洞注入全部按预期开火）**：

- `nai-security`：`CEILING == 30s` ＋ `new(CEILING)` 成功 ＋ `new(CEILING + 1ms)` 失败
  （**第一条必须把 30 s 写死**——只断言 `new(CEILING).is_ok()` 是**自指**的：`CEILING` 改小后
  拒绝阈值跟着变小，检查照样通过。外部锚点是 ADR 0004）；
- `nai-storage`：`BRINGUP_OPERATION == Operation::CEILING`；
- `tests/registry.test.mjs`（**并入既有文件 ⇒ `engineering_checks` 零改动**）：
  ① `pub const BRINGUP_OPERATION` 全仓**恰 1 处**且在 `crates/nai-storage/src/lib.rs`；
  ② 定义行 MUST 引用 `CEILING`；③ `Operation::new(Duration::from_secs(30))` 字面量**恰 0 处**；
  ④ `const OPERATION_CEILING` 私有影子**恰 0 处**。
  扫描**剔除整行注释**、**只扫 `.rs`（因此读不到自己）**。

**门控注意**：`portable` 只跑 `cargo test --workspace` 与 `cargo test -p nai-xtask --lib`
⇒ **examples 不在其中**；本轮改了三个 example，**必须自己点名**跑一次
`cargo clippy -p nai-provider-core -p nai-xtask --all-targets -- -D warnings`。

**判据的边界如实登记**：它挡的是**已知的两种复制方式**（具名字面量、具名影子常量），
不是「一切复制」——若有人新起一个别的名字的 30 s 常量，扫描抓不到。**这是边界，不是缺陷。**

**不改基线正文、不改任何数值、不改任何行为、不新增依赖边。**
见 [W77 报告](docs/reports/2026-10-12-w77-bringup-budget-single-home.md)、
`openspec/changes/archive/2026-10-12-w77-bringup-budget-single-home/`。

### W78 能力包的打包粒度（清单式散文需要一个划分断言）

**问题**：W76 把「能力包的打包粒度」登记为一条**已答**的事实，并在规格里写下
「打包粒度已由 §51.3 的能力包清单回答」。这句话是对的，但它只是**一句真话，不是一条判据**。

核实后的图景：**§51.3 的表格从未被任何断言读过。** 现有两条门控都只到**集合**层
（`registry.mjs` 的 `CAPABILITY_PACK_IDS` ↔ registry；§57.3 的「能力包」行 ↔ registry），
而 `CAPABILITY_PACK_IDS` 本身是**手抄的镜像**——它写得对，却**无法证明自己仍然对**。

⇒ 两个后果：① 把 `M13–M18` 从「市场客户端」挪走，只要 §57.3 与常量不动，**全部断言照绿**
（W66 的「钉住的是条数还是内容」教训第二次适用）；② §51.3 的八行说的是**每一个编号属于哪一个包**
——那是一个**划分**，而集合层对划分是瞎的。

**裁定**（[ADR 0056](docs/adr/0056-capability-pack-granularity.md)）：

1. **判据的来源是 §51.3 的表格原文**，不是那个常量。新增 `parseCapabilityPacks(text)`
   （复用 `sectionTable` + `expandIds`，后者本就覆盖 M 与 H 家族）。
2. **划分与并集是两条判据**：`U` 正确不蕴含 `R` 是划分（`{M01–M12, M05–M08}` 的并集一模一样），
   反之亦然；两者错误形态不同，**必须并列具名**。
3. **分界判据 = 两侧都非空 + 互斥**：§51.3 表后「保留在 R1 的基础纪律」列出的编号与包并集互补
   （「*一个确定性的上下文装配函数能不能满足它？*」的可判定形式）。**刻意不写死那个集合**——
   两侧数据都来自正文，断言因此**不需要一份手抄**。「非空」那半条必须有：解析恒为空会让
   「互不相交」**恒真**。
4. **表形状逐项写死**（`CAPABILITY_PACK_ROWS`）：标签与编号都写下来。写死的常量在这里是**断言**，
   不是**镜像**——这个区分正是本轮全部工作的要点。
5. **三向一致**：`§51.3 ∪` == registry 的 `capability_pack` 行 == `CAPABILITY_PACK_IDS`，
   任一侧不符具名给出**差异编号与差异所在的那一侧**。
6. **依据非空逐行判、与行数无关**：不许塞进「行数相符」分支——短路会让「未检查」像「已通过」。

**9 组非空洞注入全部按预期开火**（F1 行内编号改小 / F2 行标签改写 / F3 编号跨两行 /
F4 `H18` 移入包行 / F5 常量漏 `H16` / F6 删整行 / F7 章节号被改 / F8 在册侧漏一条 /
F9 依据置空），逐项**具名**而非只报「测试红了」。

**不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust**；判据并入 `reconcile` 的第 ④ 区块
⇒ 既有 `against()` 助手与**全部既有用例自动带上它**，`engineering_checks` **零接线改动**。

**边界如实登记**：只读 §51.3／§57.3／§48.5；§51.2 **刻意未读**（它是散文、无表）。
当时 `sectionTable` 对无表章节会**静默取到 §51.3 的表**——W78 只登记了这条限制，
由**文档与记性**维持；**W79 已把它封闭**（见下节）。
见 [W78 报告](docs/reports/2026-10-12-w78-capability-pack-granularity.md)、
[ADR 0056](docs/adr/0056-capability-pack-granularity.md)、
`openspec/changes/archive/2026-10-12-w78-capability-pack-granularity/`。

### W79 按章节号取表的节界（搜索域 MUST 封闭在本节内）

**问题**：W78 的规格**刻意不读 §51.2**，理由是「它是散文、没有表格，而 `sectionTable` 取的是
『标题之后第一张表』⇒ 对它调用会**静默取到 §51.3 的表**」。这条限制**只靠文档与记性维持**——
它是一条写给人的告诫，**不是一条判据**。而 `sectionTable` 是**三个解析器（§48.5／§51.3／§57.3）
共同的地基**——地基上的错会同时污染三张表。

**实测**（只用公开 API，不改被测文件）：一段「§57.3 标题在、本节内没有表、后面跟着 §57.9 的一张表」
的文本会让 `parseReleaseLines` 返回 `found = true`、`errors = []`，并把 §57.9 的
`slice_0` 四行当成 §57.3 自己的行 ⇒ **成功与失败两种情形的返回值形状完全相同**，
上层无法区分。这不是「解析器还不会读一段散文」，而是「解析器会读错一段散文**并且读得很自信**」。

**裁定**（[ADR 0057](docs/adr/0057-section-boundary.md)）：

1. **搜索域由标题层级封闭**：`[标题行, 下一个层级 ≤ 该标题的行)`。
   **层级更深**（`####`）**不**截断——写成「遇到任何标题就停」会把子标题误当节界，
   于是本节里明明有表却报「本节内无表」，那是**假阳性**方向。V3.11 当前**无 `####`**
   （`h1=1/h2=59/h3=259`）⇒ 这半条语义**不可观测**，故**由合成存根验证**，不靠「反正现在没有」蒙过去。
2. **三态分开具名**：`{found:false}`（章节号被改指）/ `{found:true, hasTable:false}`（章节在、**本节内无表**）
   / `{found:true, hasTable:true}`（解析成功）。把后两者合成「解析出的表是空的」会让**上层用错误的
   措辞定位**（§57.3 去报「计数行为空」、§51.3 去报「declares 0 capability packs」）。
3. **MUST NOT 回落到邻节的表**，也 **MUST NOT** 用一张空表代替。
4. **把「刻意不读 §51.2」从记性变成会被检查的前提**：`sectionTable` 因此**导出**；
   「§51.2 本节内无表」成为一条**会红的判据**——若某次基线修订给它加了表，
   那正是应该回去重读 ADR 0056 遗留 2 的时刻。§57.5 是第二个样本（它的下一节 `## 58`
   恰好带着两张表，正是最容易借错的形状）。
5. **边界若收得过紧，由同一组判据自己抓住**：`§48.5／§51.3／§57.3` 三张表 MUST 落在各自节界内
   ——三条同时报「本节内无表」就是边界的非空洞性证据。
6. **不与 `pending-decisions.mjs` 的章节抽取合并**：那里按**正则**抽章节号、**不取表**，
   绑在一起会让任一方调整时牵动另一方（W66「不合并不同维度」）。

**F0 先证基线为绿，F1–F7 七组注入逐项开火且具名**（F1 §51.3 表删 / F2 §48.5 表删 /
F3 §57.3 表删 / F4 §51.2 被加表 / F5 §57.5 被加表 / F6 `###` 层级写错 / F7 `####` 正确层级
——**F7 是反向注入**：必须**仍然解析成功**，证边界没被收得过紧。**只验「会红」不够，
还要验「不该红时不红」**）。
还原**按字节哈希核对**：本机 `pathlib.write_text` 会把 `\n` 变成 CRLF，**内容哈希相同 ≠ 字节相同**
（同一族教训见 W69「`copyFileSync` 连 mtime 一起复制」）。

**门控**：`cargo xtask check --profile full` **EXIT=0**（4m33s），测试目标 **91（73 `Running` + 18 `Doc-tests`）**、
**860 passed / 0 failed**；零 Rust 改动 ⇒ `passed` 与 W71–W78 逐项相同。

**顺带订正一处计数（W77/W78 报告）**：两份报告记的「**92 = 74 `Running` + 18**」**多算了一个**
——`74` 里含 cargo 的包装行 `     Running \`…nai-xtask.exe check --profile full\``，它不是测试目标。
权威口径是 `^test result:` 的**行数**（= **91**），与 `^     Running ` 的 74 行 = 1 包装行 + 73 目标
两处对上 ⇒ 正确值 **91 = 73 + 18**，与 **W71 当时记的 91（73 + 18）一致**，`860 passed` 不变。
**这不是口径变化，而是 W38/W71 已订正过的同一处多算又犯了一次**（W77 报告还为此写了方向相反的归因）。
按 §56.2，两份报告作为历史记录**保留原文 + 追加勘误段**。

`tests/registry.test.mjs` **24 → 27**（随 `engineering_checks` 既有条目自动进 `portable`／`full`，
接线零改动）。**不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边。**
见 [W79 报告](docs/reports/2026-10-12-w79-section-boundary.md)、
[ADR 0057](docs/adr/0057-section-boundary.md)、
`openspec/changes/archive/2026-10-12-w79-section-boundary/`。

### W80 待裁定台账的引用完整性（扫描域 MUST 覆盖所有承载 `§NNN` 的字段）

**问题**：W76 的台账校验器立了正确的一条断言——「条目里引用的每个 `§NNN` MUST 真实存在于当前基线」
——但它的**覆盖有两个同形的洞**，病因都是**一份会漂移的手写清单**。

**洞一（字段级）**：扫描域被手写成三个 `sectionRefs(...)` 调用（`basis`／`introduced_in`／`also_in`），
而 `REQUIRED_FIELD` 里**另外三个同样承载 `§NNN` 的字段**（`recommendation`／`closure`／`note`）
**一条都没扫**。**实测**：`recommendation` 4 条、`closure` 9 条、`note` 4 条——共 **17 条引用无人核**；
把 `PD-24.recommendation` 的 `§40.4` 改成 `§99.9` ⇒ **返回空错误**。
既有测试的注释预见了这个形状（「`basis` 之外的引用同样受检，否则**复述处可以随便编**」），
只是把「之外」当成了那两个字段。

**洞二（表级）**：`const field = REQUIRED_FIELD.get(entry.disposition); if (field) {…}`
——某处置若漏在 `REQUIRED_FIELD` 里，`field` 是 `undefined`、整段跳过 ⇒ **它的条目没有必填字段**，
校验对它**沉默**（实测：删掉一个必填字段项后，删掉对应条目的字段**不再报错**）。

**裁定**（[ADR 0058](docs/adr/0058-ledger-citation-coverage.md)）：

1. **扫描域取导出的单一清单** `CITATION_FIELDS`（`basis`／`introduced_in`／`also_in`／
   `recommendation`／`closure`／`note`）+ `ID_LIST_FIELDS`（`precondition`，按 id 校验）；
   引用扫描**遍历清单**，MUST NOT 手写。**`note` 必须在里面**——它不是 `out_of_scope` 的专属字段，
   实测 5 条里有 4 条挂在别的处置上（注解）⇒ **按「字段是否存在」触发，不按处置触发**。
2. **错误具名到字段**：`PD-24: recommendation references §99.9, …`（**条目 + 字段 + 章节号**三件齐，
   否则同一个坏章节号出现在多个字段时无法定位）。
3. **`shapeErrors()`**（导出，无条件并入 `validatePendingDecisions`）：`REQUIRED_FIELD` 键集 MUST ==
   `DISPOSITIONS`；两清单 MUST 不相交；每个必填字段 MUST ∈ 两清单之一
   ⇒ **新增处置的字段不可能在两个检查之间落空**。
4. 判据 MUST **在每个字段上开火**（三个此前未扫的字段各一次）**并逐字钉清单内容**
   ——只钉「关系」不够：三者一起被拿掉时，字段集同步变窄、关系断言仍全绿。

**注入 F0 + F1–F3（字段级）+ F4–F5（表级）+ F5b（证明洞是真的）逐项开火且具名**：
F4 把清单换回旧三字段 ⇒ `required field note for out_of_scope is covered by no check`；
F5 删掉一个必填字段项 ⇒ `disposition decidable_in_repo has no required field`；
**F5b 在 F5 前提下删掉某条 `recommendation`，`requires a non-empty recommendation` 不出现**
——**证明洞二真实存在**，F5 不是在一段没被证明有意义的分支上打转。
表级注入用**临时模块副本**（该文件无 import ⇒ 可独立 import），动手前先断言锚点**恰好出现一次**。

**门控**：`cargo xtask check --profile full` **EXIT=0**，测试目标 **91（73 `Running` + 18 `Doc-tests`）**、
**860 passed / 0 failed**；零 Rust 改动 ⇒ `passed` 与 W71–W79 逐项相同。
`tests/registry.test.mjs` **27 → 28**（随 `engineering_checks` 既有条目自动进 `portable`／`full`，接线零改动）。
**台账数据一字未改**（实测 17 条引用**全部已可解析** ⇒ 基线保持全绿）。
**不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边。**
见 [W80 报告](docs/reports/2026-10-13-w80-ledger-citation-coverage.md)、
[ADR 0058](docs/adr/0058-ledger-citation-coverage.md)、
`openspec/changes/archive/2026-10-13-w80-ledger-citation-coverage/`。

### W81 台账字段集的封闭性（扫描清单之外 MUST 为空）

**问题**：W80 把扫描域收成一份**导出的单一清单** `CITATION_FIELDS` 并**遍历它**——修掉了「清单内字段
整片漏扫」。但遍历一份清单的守卫是**开世界扫描**：**清单的补集就是盲区**。清单挡得住「清单上的字段
被编造」，挡不住**不在清单上的字段被新增**。

**洞三（数据级）**：清单之外的键**既不被扫、也不被拒**。**探针实测**：给 `PD-01` 加
`rationale: "见 §99.9"` ⇒ `validatePendingDecisions` 返回**空错误**；改成值干净的 `"见 §40.4"`
**同样返回空错误**。⇒ **给台账加一个字段，它的引用就永久不受检。**

**洞四（同形）**：`question`（**34 条全部**都带）**只有「非空」检查、不扫引用**——它是**字面意义上的
复述字段**（复述基线的提问），W80 的推理「复述处可以随便编」对它最直接适用，而 W80 的清单**唯独漏了它**。

**裁定**（[ADR 0059](docs/adr/0059-ledger-field-closure.md)）：

1. **新增并导出** `LEDGER_FIELDS`（台账**允许出现**的全部字段，**逐字写死**——派生写法会让
   「三类别之并 == 它」成为**恒真式**）与 `STRUCTURAL_FIELDS`（`id`／`disposition`，由格式与枚举判）。
2. **闭世界**：`validatePendingDecisions` 断言条目里出现的每个键 MUST ∈ `LEDGER_FIELDS`，
   否则报 `PD-01: unknown field rationale …`（**条目 + 字段名**）。
   **只看名字、不看值**——若写成「未知字段**且**其值含 `§NNN`」才报，一个含引用的字段会在将来某次
   被扫到时**无声通过**（判据的输入 MUST 等于被断言的对象）。
3. **`question` 进 `CITATION_FIELDS`**（实测 0 命中 ⇒ 不引入红；方向为安全侧——
   选「禁含 `§`」会**禁止**一个合法引用，那是假阳性方向）。
4. **三类 MUST 两两不相交**；`shapeErrors()` 的「并集 == `LEDGER_FIELDS`」是**内容断言**（双向具名）。

**注入 F0 + F1–F4（数据级）+ F5–F8（表级）逐项开火且具名**：
**F2 是值干净的未知字段 ⇒ 同样失败**（证明挡的是**字段**，不是它的内容）；
**F4 是反向注入**（`question` 里放**合法** `§` ⇒ **不红**，证边界没被收得过紧）；
F5 把 `question` 移出清单 ⇒ `field question is allowed by LEDGER_FIELDS but no check category claims it`；
F6 把 `note` 移出 `LEDGER_FIELDS` ⇒ `field note is classified but not listed in LEDGER_FIELDS`；
F7 把 `id` 也塞进引用清单 ⇒ `field id is claimed by two check categories (citation, structural)`；
F8 掺一个幽灵字段 ⇒ 同 F5 的措辞。表级注入用**临时模块副本**（该文件无 import ⇒ 可独立 import），
动手前先断言锚点**恰好出现一次**（注意 `  'question',` 在两个清单里**各出现一次**，锚点必须取三行块）。

**门控**：`cargo xtask check --profile full` **EXIT=0**，测试目标 **91（73 `Running` + 18 `Doc-tests`）**、
**860 passed / 0 failed**；零 Rust 改动 ⇒ `passed` 与 W71–W80 逐项相同。
`tests/registry.test.mjs` **28 → 30**（随 `engineering_checks` 既有条目自动进 `portable`／`full`，接线零改动）。
**台账数据一字未改**（实测当前字段集**恰好**等于 `LEDGER_FIELDS`、`question` **0 命中** `§` ⇒ 基线保持全绿）。
**不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边。**
见 [W81 报告](docs/reports/2026-10-13-w81-ledger-field-closure.md)、
[ADR 0059](docs/adr/0059-ledger-field-closure.md)、
`openspec/changes/archive/2026-10-13-w81-ledger-field-closure/`。

### W82 台账信封的闭世界（闭世界 MUST 在每个层级上成立）

**问题**：W81 的闭世界断言**只覆盖一个层级**（条目内部），于是**信封**（台账顶层）成了盲区。
三个已知信封键（`schema_version`／`baseline`／`entries`）**各有**具名检查 ⇒ 这不是「检查漏写」，
而是**键集不封闭**：**已知键各有人管 ≠ 键集封闭，新增的第一个键就落进盲区**。

**探针实测**：顶层加 `revision: "见 §99.9"` ⇒ `validatePendingDecisions` 返回**空错误**；
值干净的 `"见 §40.4"` **同样空错误**；把一个**条目字段名**（`question`）写到顶层
**也空错误**（条目闭世界看不见它、信封闭世界还不存在 ⇒ **两个闭世界同时在缝里放行**）。

**裁定**（[ADR 0060](docs/adr/0060-ledger-envelope-closure.md)）：

1. **新增并导出** `TOP_LEVEL_FIELDS`（`schema_version`／`baseline`／`entries`，**逐字写死**）。
2. **信封闭世界**：台账顶层出现的每个键 MUST ∈ `TOP_LEVEL_FIELDS`，否则报
   `unknown envelope field revision …`（**报键名**）。**只看键名、不看值**（同 W81 决策 2）。
3. **信封字段必需**：声明的每个顶层字段 MUST 出现在台账上，否则报
   `envelope field baseline is missing …` ⇒ **声明一个字段就必须在数据里有它**
   （错加幽灵字段会**立刻红**，而不是静静地什么都不检查）。
4. **两个层级的字段名集 MUST 不相交**（`shapeErrors()`）⇒ 报
   `field note is declared as both an envelope field and an entry field`。这是 UB3 那个洞**唯一**的守卫。
5. **两层各自保留措辞**（一层报**条目 + 字段名**、一层报**键名**），MUST NOT 合并
   ——合并会让「错在哪一层」在措辞上消失。

**注入 F0 + F1–F4（数据级）+ F5–F9（表级）逐项开火且具名**：
**F2 是值干净的顶层未知键 ⇒ 同样失败**（证明挡的是**键**，不是它的值）；
**F4 逐个删除信封字段** ⇒ 各报 `envelope field <name> is missing`；
F5 把 `baseline` 移出声明集 ⇒ 被当成**未知键**拒；F6 掺幽灵字段 ⇒ `envelope field ghost is missing`；
F7 把条目字段 `note` 塞进声明集 ⇒ 不相交断言具名；
**F8／F9 是反向注入**（移出 `entries` 只报未知键、**不**误报必需缺失；声明集**顺序打乱不红**）。
表级注入用**临时模块副本**（该文件无 import ⇒ 可独立 import），锚点先断言**恰好出现一次**。

**门控**：`cargo xtask check --profile full` **EXIT=0**，测试目标 **91（73 `Running` + 18 `Doc-tests`）**、
**860 passed / 0 failed**；零 Rust 改动 ⇒ 与前十一轮逐项相同。
`tests/registry.test.mjs` **30 → 32**（随 `engineering_checks` 既有条目自动进 `portable`／`full`，接线零改动）。
**台账数据一字未改**（实测顶层键**恰好**等于 `TOP_LEVEL_FIELDS` ⇒ 基线保持全绿）。
**不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边。**
见 [W82 报告](docs/reports/2026-10-13-w82-ledger-envelope-closure.md)、
[ADR 0060](docs/adr/0060-ledger-envelope-closure.md)、
`openspec/changes/archive/2026-10-13-w82-ledger-envelope-closure/`。

### W83 引用字段的验证范围（触发条件 MUST 只由**字段存在**决定）

**问题**：W80 给台账确立了「**扫描按「字段是否存在」触发，MUST NOT 按处置触发**」这条纪律
（当时的发现是注解型的 `note`），**但它只落到了 `§NNN` 一侧**。台账里的**登记 id 引用**
（`precondition`，`ID_LIST_FIELDS` 的唯一成员）的校验是**按处置触发**的：
`if (entry.disposition === 'needs_precondition' && …)`。

**探针实测**（用真实注册表）：把 `precondition: ["W99"]` 挂到**非** `needs_precondition` 的条目上
⇒ `validatePendingDecisions` 返回**空错误**（`needs_external`／`answered_by_baseline`／
`decidable_in_repo` 三种宿主全部静默）；**对照组**：同条目的 `closure='§99.9'` **被抓**——
**同一个洞的两半，只有一半被 W80 补上**。

**裁定**（[ADR 0061](docs/adr/0061-reference-field-verification.md)）：

1. **新增并导出** `REFERENCE_FIELDS`（八个：七个引用字段 ＋ `precondition`，**逐字写死**）
   ——引用扫描域的**单一来源**。
2. **扫描循环由 `REFERENCE_FIELDS` 驱动，触发条件里不出现 `disposition`**——
   「**必填**」是**预设**断言（按处置触发正确）；「**引用可解析**」是**内容**断言（按处置触发错误），
   两者 MUST 分开。
3. **错误措辞「条目 + 字段 + 值」三件齐**，字段名由 `${field}` 生成（写死的字面量在新增字段时**说谎**）。
4. **只拒「不可解析的引用」，不拒「放错位置的引用」**——可解析的 `precondition` 出现在非专属处置上
   MUST NOT 红（假阳性方向）。
5. **`shapeErrors()` 双向具名断言**：`REFERENCE_FIELDS` ↔ `CITATION_FIELDS ∪ ID_LIST_FIELDS`
   （方向②正是本轮洞的近亲：曾在类别里、却只在一种处置下被扫）。

**注入 F0 + F1–F4（数据级）+ F5–F8（表级）逐项开火且具名**（8/8）：
F1–F3 三种坏值走**三条不同分支**（作业项 / 验收 id / 两者都不是）都失败并报**条目 + 字段 + 值**；
**F4 是反向注入**（合法 id 在非专属处置上 MUST NOT 红）；F5 把 `closure` 移出扫描域 ⇒
`is not a reference field`；F6 掺幽灵字段 ⇒ `no check category owns it`；
**F7／F8 是反向注入**（顺序打乱不红、还原后基线仍绿）。表级注入用**临时模块副本**（该文件无 import）。

`tests/registry.test.mjs` **32 → 34**（随 `engineering_checks` 既有条目自动进 `portable`／`full`，接线零改动）。
**台账数据一字未改**（实测 `precondition` **只**出现在 4 条 `needs_precondition` 条目上 ⇒
触发条件放宽后**报错集合不变**，基线**保持全绿**）。
**不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边。**
见 [W83 报告](docs/reports/2026-10-13-w83-reference-field-verification.md)、
[ADR 0061](docs/adr/0061-reference-field-verification.md)、
`openspec/changes/archive/2026-10-13-w83-reference-field-verification/`。

### W84 测试文件的可达性闭世界（在册测试文件 MUST 被某个登记站点覆盖）

**问题**：Node 测试靠**逐条列举**（`xtask/src/lib.rs`、`xtask/src/registry.rs` 里手写
`node --test <path>`）加**两个 glob**（`package.json` 的 `tests/ui/*.test.mjs`、
`apps/desktop/package.json` 的 `tests/*.test.mjs`）接上电。**列举的盲区恰好是它的补集**。

**探针实测**（非推断）：把 `tests/kernel/w84-probe.test.mjs`（内容只有一句 `assert.fail(...)`）
加入 `.git/index` 后跑 `cargo xtask check --profile portable` ⇒ **`EXIT=0`**、
`test result` 行数 **91**（73 `Running` + 18 `Doc-tests`）、**860 passed / 0 failed**、
而 `grep -c "w84 probe"` = **0**（探针用例名在日志里**零出现**）。
⇒ 不是「未登记 = 不跑」，而是**静默的不跑**：连「忽略了一个文件」都没有任何信号。

**裁定**（[ADR 0062](docs/adr/0062-test-reachability-closure.md)）：

1. **新增并导出** `TEST_REGISTRATION_SITES`（七个站点，逐字写死 `{id, host, marker, base, tier}`）
   ——「测试在哪里被接上电」的**单一来源**。
2. **覆盖集由 marker 原文解析得出**（`resolveSite`），MUST NOT 另行声明一份——否则「表 == 它自己」
   是**恒真式**（W80 决策 5）。用**已跟踪路径**而非 `readdir`：按**仓库成员**计算（W75），
   未跟踪文件不参与覆盖集 ⇒ 探针**必然**触发闭世界。
3. **假契约 + 非空洞**：marker MUST 在 host 里**逐字存在**；站点 MUST 至少覆盖一个**在册**文件。
   「非空洞」**不是**「站点数 > 0」——后者对一个空 glob 是瞎的。
4. **闭世界 MUST 在「扩展名层」也成立**：`looksLikeTest`（宽）与扩展名 ∈ `TEST_EXTENSIONS`（窄）
   分开判——`foo.test.ts` 连「未被覆盖」都不会被报出。**清单的窄，正是它闭合的原因**（T2 注入）。
5. **台账不是豁免**：`LOCALLY_UNREACHABLE` 条目 MUST 带理由、MUST 是在册测试文件、
   MUST **确实未被覆盖**（自清理，已可达即红）。
6. **「覆盖集非空」与「两集不相交」成对**：台账为空时不相交退化成恒真 ⇒ 必须有 `covered.size > 0`，
   否则「全不覆盖」也能绿（「非空是互斥类断言的必要一半」）。
7. **入参显式注入，缺任一项即报错**。

**注入 F0 + F1–F9（数据级）＋ T0–T5（表级，临时模块副本）共 15/15 开火且具名**：
**F0** 真实仓库绿；**F1** 是洞的直接复现（在册却不被覆盖的文件被具名）；**F2** marker 漂移 ⇒ 假契约；
**F3** glob 缩窄 ⇒ 恰好丢掉的文件落进闭世界；**F4** 空站点 ⇒ 非空洞；**F5** 已可达的台账条目 ⇒
过期豁免；**F6** `typed.test.ts` ⇒ 扩展名盲区；**F7／F8 是反向注入**（真未覆盖的台账条目不红、
站点顺序打乱不红）；**F9** 站点表清空 ⇒ 覆盖集为空。表级 **T1** 从真实表删一个站点 ⇒ 恰好 4 个
ui 文件红；**T2** 放宽 `TEST_EXTENSIONS` ⇒ 盲区**重开**（0 错误）而窄清单报 1 条；
**T3** 清空真实站点表 ⇒ 10 条错误含空覆盖；**T4/T5** 还原后仍绿、源文件逐字未改。

`tests/registry.test.mjs` **34 → 47**（随 `engineering_checks` 既有条目自动进 `portable`／`full`，接线零改动）。
**探针文件在归档前删除**，基线**保持全绿**（基线 91 / 860 与 W71–W83 逐项相同）。
**不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边。**
见 [W84 报告](docs/reports/2026-10-13-w84-test-reachability-closure.md)、
[ADR 0062](docs/adr/0062-test-reachability-closure.md)、
`openspec/changes/archive/2026-10-13-w84-test-reachability-closure/`。

### W85 OpenAPI 操作集的闭世界（判据 MUST 遍历文档自己的操作键）

**问题**：`validation.mjs` 的 `apiDocument()` 有两道**逐操作**检查——权限声明
（`x-required-permission`）与实现状态（`x-implementation-status`）——但「哪些操作要被查」由一张
**手写的方法清单**决定：`for (const method of ['get', 'post', 'put', 'delete'])`。
`patch`／`head`／`options`／`trace` 都是 **OpenAPI 3.0 的合法方法键**，全部落在清单之外。

**探针实测**（往 `paths` 里加操作、`SwaggerParser` 校验、再跑那段循环的**逐字逻辑**）：
一个**无**权限、**无**状态的 `patch` 操作 ⇒ 判定返回**空错误**（静默通过）；同位置换成 `get`
⇒ 立刻报错。**`SwaggerParser` 两种情况都放行**——它管结构合法性，不管覆盖。
⇒ 一个 HTTP 方法只要落在清单之外，权限与状态**都不再被问**：**一整类操作可以从契约门口走进来而无人过问**。

**同族第二处**（潜伏，当前计数为零）：`implemented` 的八条**只被用来反查文档**，从未要求
**指回文档里真实存在的操作** ⇒ 一条**只有声明、没有对象**的契约可以零信号存活。

**裁定**（[ADR 0063](docs/adr/0063-openapi-operation-closure.md)）：

1. **判据遍历文档自己的操作键**（W83 的应用），MUST NOT 遍历手写子集——「新方法」与「老方法」
   走同一条路径。
2. **`HTTP_METHODS` 逐字写死**（八个，OpenAPI 3.0 全表），MUST NOT 由「文档里现有方法」派生
   （否则「现有方法 == 清单」是**恒真式**）。仓库此刻只用四个，但清单 MUST NOT 随之收缩。
3. **`PATH_ITEM_FIELDS` 逐字写死**（五个），path item 的**每个键** MUST ∈ 两者之一
   （`x-` 扩展前缀按规范放行）⇒ `patch` 被**检查**、`gets:` 这类笔误被**具名拒绝**。
4. **相邻层不相交**（W82）：`HTTP_METHODS ∩ PATH_ITEM_FIELDS == ∅`。
5. **声明无孤儿**（W75）：`IMPLEMENTED_OPERATIONS` 的每一项 MUST 指回文档里真实存在、
   且状态为 `implemented-loopback` 的操作。
6. **非空洞**：文档 MUST 至少暴露一个操作，否则逐操作检查全部为真但**空转**（同 W84）。

**注入 F0 + T1–T8（表级 11/11，含 T7／T8 两条反向注入）逐项开火且具名**：
**T1 是本轮的核心表级证据**（把 `HTTP_METHODS` 收缩回旧的四方法 ⇒ 逐操作检查**不再覆盖 patch**，
只剩键集闭世界在挡）；T2（删 `get` ⇒ 声明侧出现孤儿）；T3（空声明表 ⇒ 非空洞断言开火）；
T6（声明表掺幽灵 ⇒ `no such operation`）；**T7 是反向注入**（声明顺序打乱不红）；
**T8 是反向注入**（还原后基线仍绿）。表级注入用**临时模块副本**（该文件无 import，副本可独立加载）。

`tests/contracts/schema.test.mjs` **17 → 24**（随 `engineering_checks` 既有条目自动进 `portable`／`full`，
接线零改动）。**`schemas/openapi/control.yaml` 一字未改**（实测其 path item 键全部落在闭集内、
42 个操作全部带权限声明、状态全部与声明一致、八条 `implemented` **零孤儿** ⇒ 收紧后
**报错集合不变**，基线**保持全绿**）。
**不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边。**
见 [W85 报告](docs/reports/2026-10-13-w85-openapi-operation-closure.md)、
[ADR 0063](docs/adr/0063-openapi-operation-closure.md)、
`openspec/changes/archive/2026-10-13-w85-openapi-operation-closure/`。

### W86 OpenAPI 文档集的闭世界（操作集判据 MUST 覆盖仓库里每一份描述）

**问题**：W85 把「哪些**操作**被逐项校验」从手写方法清单改成遍历文档自己的操作键，但
「哪些**文档**被校验」仍是手写清单——`validation.mjs` 的 `apiDocument()` 把描述路径**硬编码**
成 `schemas/openapi/control.yaml`。同目录的 `remote-knowledge.yaml` 是它的补集：
**本地门控从不读它**（唯一读它的 `generate.mjs` 只在 **CI-only** 的 `cargo xtask schema --check`
里跑，而本机对该步 `EBUSY`），而它自己的生成器（W86 改动前位于 `generate.mjs:125`）又给
「remote 的每个操作必须有 `operationId` 与 `x-required-permission`」**另写了一份定义**——
措辞与覆盖都与判据不同，是**同一件事的第二处定义**。

**探针实测**（两份真实文档 + `SwaggerParser`）：往 `remote-knowledge.yaml` 加一个**无**
`x-required-permission` 的 `patch` 操作 ⇒ `SwaggerParser` **放行（没拦）**，而现有本地判据
**根本不接触这份文档** ⇒ **本机门控全绿**。同位置换成 `get` 同样无人过问。
⇒ 一份**承载鉴权语义**的契约文档，可以在本地绿 + CI 结构校验绿的情况下新增一个**不声明权限**的操作。

**裁定**（[ADR 0064](docs/adr/0064-openapi-document-set-closure.md)）：

1. **文档集由已跟踪路径派生**（W75／W84）：`OPENAPI_DIR` 下扩展名 ∈ `OPENAPI_EXTENSIONS` 的
   **在册**路径；闭世界**只看名字**，MUST NOT 手写「有哪些文档」。
2. **`OPENAPI_DOCUMENTS` 是逐文档声明的单一来源，逐字写死**（`status` + `implemented`）——
   「已实现的是哪些操作」是**每一份文档各自的属性**（remote 自述 `client-only`，表为空）。
3. **文档集闭世界是双向的**：在册描述 ⊆ 声明集（新增文档忘了声明 ⇒ 无人校验）；
   声明集 ⊆ 在册描述（文档改名/删除而声明留着 ⇒ 没有对象的覆盖）。
4. **判据逐文档参数化，`declared` 是必需入参**：缺参／类型不对 ⇒ 具名失败，MUST NOT 退回
   另一份文档的默认值（否则**判据的输入不再等于被断言的对象**，而断言看上去仍在跑）。
5. **权限规则只有一处定义**：`generate.mjs` 改为**调用** `operationErrors`，删掉复述的那半句，
   只保留它真正需要的 `operationId`（生成 `*_PATH`／`*_METHOD` 常量要名字）。
6. **非空洞**：文档集非空、每份文档至少一个操作、且**至少一份**文档声明了非空 `implemented`
   ——否则「声明无孤儿」在每份文档上**空转**。

**注入 F0 + F1–F10 + T1–T2（共 21/21）逐项开火且具名**：**F1 是本轮洞的现场**
（remote + 无权限 `patch` ⇒ 具名；对照组 `SwaggerParser` 放行）；F2 未声明的在册描述；
F3 陈旧声明；F4 空声明集；F5 无人声明已实现；F6 缺 `tracked`；**F7 缺 `declared` 入参 ⇒ 具名
（MUST NOT 静默降级）**，且用错文档的声明会得到**八条孤儿**（证明「用错对象」有害）；
**F8／F9 是反向注入**（合法新操作不红、声明顺序无关）；F10 扩展名层（只看名字）；
**T1 打判据的输入**（声明集缩回一份 ⇒ remote 逃逸被指名）；**T2 三条文本守卫**
（生成器代码里不再有第二份权限规则——整行注释不算，W74；判据不再硬编码文档路径）。

`tests/contracts/schema.test.mjs` **24 → 29**（随 `engineering_checks` 既有条目自动进 `portable`／`full`，
接线零改动）。**两份 OpenAPI 描述一字未改**（实测各自通过闭世界 ⇒ 收紧后**报错集合不变**，
基线**保持全绿**）。
**不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边。**
见 [W86 报告](docs/reports/2026-10-13-w86-openapi-document-set-closure.md)、
[ADR 0064](docs/adr/0064-openapi-document-set-closure.md)、
`openspec/changes/archive/2026-10-13-w86-openapi-document-set-closure/`。

### W87 升基线 V3.11（把 §56.2 的归档边界写死；消除 §56.2 自身的自相矛盾）

W87 **不是**范围版，也**不是**设计变更，而是**订正版**——把 **W78** 登记、**连续八轮未收录**
（W78–W86 §八）的一条**判据边界**补入正文，并顺带订正 §56.2 自身的一处**自相矛盾**。

**问题（W78 §八第 8 条，八轮复述）**：§56.2 把 `openspec/changes/archive/` 列为「保留原文 + 追加勘误段」，
但**未区分「上一轮已发布的归档」与「本轮刚归档、尚未入库」**。W78 撞上后者，按「保护已发布原文」的
立意作了**就地订正**；W76 §九第 6 条则对**已提交**的归档追加了勘误段。两者**处置不同而判据相同**——
判别项是**「该原文是否已进入提交」**，不是「轮次」（「本轮」在多人协作下无定义）。

**第二处（W87 核实中发现）**：§56.2 的情形③ 把「**§56**」整体列为**保留原文**的版本专属章节，
而 V3.10 §58.2 的 **E5 自己就就地订正过 §56.2**，ADR 0049 决策 4 的「不改」清单亦**不含 §56**
（只列 §39、§48.1/§48.2、§57.1/§57.2/§57.4/§57.5）⇒ 该行**自相矛盾**：若「§56」整体冻结，
「随版本累积订正」这条**一般规则就无法被订正**，E5 与 E2 都无从发生。

两者都不属于「可纯追加」：§56.2 明写**纯追加 MUST NOT 被用来让矛盾长期存续**；边界不定，
则**每一轮作者都要重新判一次**同一件事。故本版是 §56 规则的**第二次跨轮次适用**（第一次是 V3.10）。

**裁定**（[ADR 0065](docs/adr/0065-design-baseline-v311.md)）：

**产出**：新建 `docs/直播数字人 + 智能体系统技术方案 V3.11.md`（= V3.10 + **2 处 1:1 行订正** +
**版本标识 3 行** + 追加 **§59**）：

- **E1** §56.2「历史记录」行：补明**保护对象是「已发布的原文」**——判据 = 该原文**是否已进入提交**；
  同一轮工作区内新建、尚未提交的产物 ⇒ **就地订正**，不产生勘误段。
- **E2** §56.2 情形③：把「§56」精确为「**§56.1／§56.3／§56.4**」，并注明「§56.2 是一般规则」。

**契约证据**（`diff`，非 `-u`）：整文件 **5 hunk = 4 个 `c` 型**（`1c1`、`3c3`、`29c29`、
`2573,2574c2573,2574`）**+ 1 个 `a` 型**（`2713a2714,2766`，§59 追加在末行之后）**+ 0 个 `d` 型**；
限定**前 2713 行**（既有正文）则**只有 4 个 `c` 型、零 `a`·`d`** ⇒ **行号一一对应，全部既有 `§NNN`
引用继续有效**；规模 **2713 行 / 226,578 字节 ⇒ 2766 行 / 233,290 字节**。

**指针同步按 §56.2 四情形分流**：改动前实测 **143 行 / 31 文件**（口径见报告），**逐条判定**
（MUST NOT 以「版本号出现即改」批量替换）后**幸存 35 行 / 9 文件，全属情形③**（升基线链条、
`V3.10 §58.2` 的 E1–E5 收录记载、W71/W72 轮次记录、被拒改写的举例、`tests/registry.test.mjs:155`）。
情形①（声明当前基线）：`docs/design.md`、`README.md`、`AGENTS.md`、`openspec/config.yaml`、
`xtask/src/registry.rs` 输出串、`apps/desktop/src/main.tsx` 界面文案、**`docs/development-plan.md`
的「V3.11 切片工作项进展」标题**（W71 先例）；情形②（`§NNN` 前缀）：`tests/registry.yaml` 的
**29 条能力包标记**与 release_line 标签、各 `docs/*.md`／`openspec/specs/*` 的章节指针、轮次记录标题。

**明确不做**：不改任何 `status`（88 条原样，H18 与 B07 仍 `pending`）、不改发布线归属
（§57.3 计数 `slice_0` 10 / `r1` 37 / `r2` 9 / `capability_pack` 29 / `r3` 1 / `experimental` 2 = **88**）、
**不实现任何代码**（Rust 行为零改动；随版本号同步的只有**字符串常量**——输出串、错误消息、
`baseline` 串、断言串）、不给 §16 的验收注册表增行、不重排 W 编号。**基线保持全绿**。
见 [W87 报告](docs/reports/2026-10-13-w87-baseline-v311-archive-boundary.md)、
[ADR 0065](docs/adr/0065-design-baseline-v311.md)、
`openspec/changes/archive/2026-10-13-w87-baseline-v311-archive-boundary/`。

### W88 待裁定台账的第六态与落地点（PD-09 结清）

W88 **不是**升基线、**不是**加能力，而是**结清一条已登记的推荐**——并由此发现台账缺了一整个维度。

**问题（W87 §八第 1 条 / ADR 0054 遗留 3）**：W76 的台账（`tests/pending-decisions.yaml`）把 34 条
待裁定项分成**五态**，五态回答的都是「**何处能闭合**」：基线答了／可在仓库内决定／等前置／等外部／
不在范围。**没有一态**表示「**仓库已经作出决定、并且已经落地**」⇒ 一个已结清的条目只能永远停在
`decidable_in_repo`，而那个状态的名字叫「**可**决定」。本轮采纳的 **PD-09**
（*静态数据加密所用的系统密钥保护机制*）正是这个形状。

**核实（先读后判，四条子句逐条落地）**：`crates/nai-security/Cargo.toml` 把 `keyring` **只**放在
`[target.'cfg(windows)'.dependencies]` 下；`NativeVault` 在 Windows 走系统凭据库、非 Windows 一律
`CapabilityNotReady`（**失败关闭，不是静默降级**，有 `production_backend_is_explicitly_unavailable`
钉住）；`schemas/platform-matrix/platforms.json` 的 `secret-protection` 能力行逐平台登记
（`windows` = supported、`linux`/`macos` = `explicit-unavailable`/`not_ported_to_target_os`、
`ios`/`android` = `out-of-scope`）⇒ 推荐的四条子句（按平台用系统密钥库／Windows 原生凭据首发／
缺后端的平台显式登记／MUST NOT 自造跨平台密钥库）**逐条成立**。

**同时排除了一条看似更省事的路**：**MUST NOT** 把它记作 `answered_by_baseline`——基线 §11.5 的原文是
「备份与录制提供可选加密，密钥由系统密钥保护机制封装（**具体机制待选型验证**）」，
**基线没有回答它**，是**仓库**回答了它。写成 `answered_by_baseline` 会是一条**不实声明**
（这条排除是**判据级**的，不是措辞级的）。

**裁定**（[ADR 0066](docs/adr/0066-decided-in-repo-landing.md)；ADR 0054 **追加勘误段**）：

- **新增第六态 `decided_in_repo`**（决定已在仓库内作出且已落地），必填字段 **`landing`（落地点）**
  ——与 `answered_by_baseline` 用 `basis`（答在哪一节）而不是「答案原文」是**同一条纪律**：
  台账钉「**管到哪**」这个可判事实，语义是否真的落地**刻意留给人读**。「每态**一个**必填字段」**不变**。
- **落地点 MUST 是仓库已跟踪的路径**，判据输入由 `.git/index` 派生（`git-index.mjs`，W75）——
  **MUST NOT** 遍历工作区（那是 W75 的 **F2 根因**：未跟踪文件会被算成仓库成员）。
  探针以 `node_modules` 为样本（磁盘上存在、索引里没有）把两种读法**分开**。
  为什么不用作业项编号：实测基线的工作项表**不含 W41–W55**（§47.6 的编号说明）⇒
  落地轮次 `W55` **无法作为 `W\d{2}` 解析**，那会让必填字段恒假。
- **新增第三类检查类别 `REPO_PATH_FIELDS`**，与 `ID_LIST_FIELDS` **不合并**（解析器不同：登记 id vs
  仓库成员路径）。四类 MUST 两两不相交、并集恰等于 `LEDGER_FIELDS`；`shapeErrors()` 的
  「不相交 / 并集相等 / 引用域双向相等」三条断言随之从三扩到四。
- **「清单型 / 文本型」改由类别表决定，MUST NOT 写死字段名**。原实现把分派写成
  `field === 'precondition'`（由**写死的字段名**决定）；本轮**实测改前那一行**：给 `PD-09.landing` 一个
  **合法跟踪路径的字符串** ⇒ 改前 **0 error**（当文本判，「非空」即过关）、改后 **1 error**
  （`requires a non-empty landing`）。
- **路径校验按「字段是否存在」触发，MUST NOT 按处置触发**（W80 在 `§NNN` 侧、W83 在 id 侧已确立；
  `landing` **出生即遵此律**）。
- **不引入 `tests/pending-decisions.schema.json`**：ADR 0054 遗留 5 说「等第二次结构变更再判」，
  本轮**正是**那次变更（6 态 + 1 字段 + 1 类别 + 1 依赖），判定为**不引入**——校验器已用
  **闭世界**（条目键集、信封键集）+ 封闭枚举 + 类别划分把形状钉死**且接上了电**，再写一份 schema
  会在**同一件事上出现第二处定义**（W86 的教训：两处定义会静默分家）。
- `validatePendingDecisions` 新增必填依赖 **`repositoryFiles`**；缺它即具名报错，**MUST NOT** 降级。

**数据侧**：`PD-09` 由 `decidable_in_repo` 迁到 `decided_in_repo`；`DISPOSITION_COUNTS`
`decidable_in_repo` **5 → 4**、`decided_in_repo` **0 → 1**（总数仍 **34**、分章计数不变）；
`recommendation` **原文保留**为注解（它就是被采纳的那份建议，删掉就丢了「决定了什么」）。
`tests/registry.test.mjs` **47 → 50**；非空洞注入 F0–F7（含 F3「把 `landing` 移出扫描域 ⇒ 同一个坏值
**静默**」与 F4/F5 的改前/改后对照）。

**明确不做**：不改基线正文、不改 `tests/registry.yaml`、**不实现任何代码**（Rust 零改动）、
不新增依赖边、不新增契约名、不改余下 4 条 `decidable_in_repo` 的处置、不重排 W 编号。
见 [W88 报告](docs/reports/2026-10-13-w88-decided-in-repo-landing.md)、
[ADR 0066](docs/adr/0066-decided-in-repo-landing.md)、
`openspec/changes/archive/2026-10-13-w88-decided-in-repo-landing/`。

### W89 观众长期记忆开关的单一来源与写入面（PD-31 结清）

W89 **不是**升基线、**不是**加能力——它把 W88 §八第 2 条登记的**一处张力**核实到底，并把结论落在**判定面**上。

**问题（W88 §八第 2 条）**：台账 `PD-31`（*观众长期记忆的字段集与保留期*）的推荐写着「R1 内判为
『**空集 + 默认关闭**』」，而 W57 已交付的 `crates/nai-kernel/src/evaluate/memory.rs` 里
`MemoryPolicy::default().allowed_viewer_fields` 是**非空**（`["preference", "answered_topic"]`）。

**核实（三条实测事实，比登记的张力更靠前）**：

- **F1 配置面是死键**：`schemas/config/runtime.schema.json` 的 `session_defaults` 是
  `additionalProperties: false` 且 **`required` 明列** `viewer_memory_enabled`；但全仓**非测试**代码里
  `composition.config()["session_defaults"]` **只被读一次**（`apps/engine/src/pipeline.rs:874`），
  且**只读同层的 `allow_auto_without_platform_grant`** ⇒ 把 `viewer_memory_enabled` 改成 `true`
  **没有任何效果**。同层字段「**一个有主、一个无主**」，这个**对照本身**就是判据（「已声明 ≠ 已执行」）。
- **F2 判定面只覆盖一半**：`viewer_memory_enabled` 只被 `read_records`（**读取面**）与
  `viewer_record_allowed`（**无生产调用方**）消费；**写入面 `admit_write` 既不接受策略、也不检查它**
  ⇒ 默认关闭下一条 `kind = "viewer"` 的候选**会被接受**（真跑改前那一行：返回 `Ok(())`）。
- **F3 措辞同形不同义**：推荐的「空集」论据是「§5.5 的默认关闭**使字段集没有对象**」，指**运行时被写入的
  字段值集合**；而 `allowed_viewer_fields` 是**启用后才生效的允许字段白名单**（D10 沿用，W57 报告 §七
  明写「**不新造取值**」）⇒ 台账**用同一个词指了两样东西**。

⇒ 所以**不是实现缺陷**，而是**「默认关闭」的落点不闭合**：三处载体里**只有读取面一处有执行体**。

**裁定（[ADR 0067](docs/adr/0067-viewer-memory-single-answering-point.md)，决策 1–8）**：

- **`viewer_memory_gate(policy)` 是这条开关的唯一判定处**，读取面／写入面／请求准入面共用；
  MUST NOT 各自复写 `if !policy.viewer_memory_enabled`（一条开关答两次就会分家）。
- **写入面 MUST 也判它**（本轮补的**执行体**，不是新增策略）——判定是**能力级**的（不依赖条目内容）。
- **判据位置**：`admit_write` 里放在**必填项之后、来源判定之前**，与读取面「结构必填 → 能力开关 → 授权」同序；
  **顺序 MUST 有断言**，否则只是注释。
- **开关 MUST NOT 塞进 `MemoryCandidate`**：那是 W57 §四.1 已排除的「把门槛交给被审对象自己填」⇒
  `admit_write` 增加 `policy` 入参（全仓调用方只有本 crate 与其集成测试）。
- **拒因复用 `viewer_memory_disabled`**，不新造名字（同一件事 MUST NOT 有两个名字）⇒
  `MemoryRejection::ALL` 仍 **23**，既有总账断言**不放宽**。
- **legacy `viewer_record_allowed` 保留 `EvaluateError` 契约、只做错误类型映射**——**映射不是第二处定义**，
  `if !enabled` 这个判定在仓库里只剩一处。
- **F1 只登记不改**，MUST NOT 用「不动」伪装成「已处理」（修它要么改变运行时行为、要么立一条新类别的
  断言「配置面的 `required` 键 MUST 有消费者」，后者本身是一个**独立母题**）。

**非空洞注入（F0–F7）**：F0/F1 改前/改后对照（**真跑改前那一行**：默认策略下 viewer 候选改前 `Ok(())`、
改后 `Err(viewer_memory_disabled)`）；F2 **承重**（移出写入面 gate ⇒ 2 failed）；F3 **顺序断言**
（同时「未启用 + 不可信来源」⇒ 报能力未启用，改前会报 `untrusted_source_as_fact`）；F4 **反向不红**
（启用后合规候选通过、非观众类别不受影响）；F7 **反转 gate** ⇒ 读取面与写入面 **6 failed 同时变红**
（证明两面确实共用一个判定，而不是各自写着看似一样的代码）。

**数据侧**：`PD-31` 由 `decidable_in_repo` 迁 `decided_in_repo` + `landing`（仓库已跟踪路径），
并把「空集」**就地订正**为两个具名对象（W89 另**补正** W88 漏改的台账头部注释——它当时仍只列五态）；
`DISPOSITION_COUNTS` `decidable_in_repo` **4 → 3** / `decided_in_repo` **1 → 2**（总数仍 **34**）；
`tests/memory.rs` **21 → 23**。

**明确不做**：不改基线正文、不改 `tests/registry.yaml` 的 `status`（H03／H04 仍 `pending`）、
**不动运行时**（F1 只登记）、不改 `allowed_viewer_fields` 与 `max_retention_ms` 的取值、
不新增依赖边与契约名、不重排 W 编号。
见 [W89 报告](docs/reports/2026-10-13-w89-viewer-memory-single-answering-point.md)、
[ADR 0067](docs/adr/0067-viewer-memory-single-answering-point.md)、
`openspec/changes/archive/2026-10-13-w89-viewer-memory-default-off-single-home/`。

### W90 配置面的键级消费状态（受判 `required` 键的三分类闭世界）

W90 **不是**升基线、**不是**给配置接线——它是 W89 遗留 1 那个**母题**（「配置面 `required` 键 MUST 有
消费者」）的兑现：把「同层字段一个有主、一个无主」那条**一次性对照**下推成**键级**的可执行判据。

**实测（扫描非测试、非生成、已跟踪源码，读取形状 `EXPR["<key>"]`）**：45 个 `required` 键里
8 个是**有 `required` 子键的容器**（消费由子键蕴含）⇒ **受判键 37**；其中 **[A] 有读取点 10**、
**[B] 无读取点但值被 `const` 钉死 4**、**[C] 既无读取点也未被钉死 23**。
⇒ **W89 的 `viewer_memory_enabled` 不是特例，是 23 分之 1**；改这 23 个键的值**不改变任何行为**。

**[C] 的成因逐段实测**（`scripts/contracts/config-consumption.mjs` 的成因表，6 条）：
① `constructor-injected`（12 条）——值以构造器/参数由调用方注入，而调用方在生产代码里**不存在**
（`Book::new(workspace, limits)`、`EventIngress::new(…, capacity, …)`、`Scheduler::new(…, policy)`、
`RemoteGateway::new(policy)` 的调用方**只有 `tests/`**）；② `hardcoded-default`（1 条：`Session::new`
写死 `SessionMode::Assisted`）；③ `surface-absent`（5 条：非测试代码里**连键名都不出现**）；
④ `same-name-other-object`（3 条：`cloud.*` 的同名者是 `nai-upgrade-core` 读的**升级清单**字段）；
⑤ `binding-consumer-absent`（1 条）；⑥ `awaiting-decision`（1 条：`viewer_memory_enabled`）。

**判据（四条，全部在既有 `tests/contracts/schema.test.mjs` 里跑 ⇒ `xtask` 零改动）**：

- **受判键集合派生**：`required` 里减去「自己有 `required` 子键的容器」；**容器由结构判定**，
  MUST NOT 写成键名清单。**无 `required` 子键的 object 键 MUST 自己受判**（`bindings` 就是这一条——
  它不能被任何子键蕴含）。MUST NOT 按章号/层级圈范围。
- **三分类双向闭世界**：`[A] ∪ [B] ∪ [C]` 两两不相交、并集**恰等于**受判键集合；
  **[A] 与 [C] 两侧都双向**——有读取点的键 MUST 具名在册（**观测不能替人签名**）、在册的键 MUST
  真的没有读取点（**签名不能遮住观测**）。
- **读取形状只认 `EXPR["<key>"]`**（`[` 前必须是表达式字符）：实测 `xtask/src/deps.rs:441` 的
  `&["knowledge"]` 是 Rust **数组字面量**，宽松正则会把它算成读取点、让零读者的容器键凭空获得读者。
  代价是「将来改用反序列化到结构体读配置会被报成未接线」——**假阳性方向**，安全。
- **两张台账手写 + 自清理**：`[A]` 每条写**证据文件**（断言文件里真有该形状、且被跟踪）；
  `[C]` 每条写成因 id（成因表每条 MUST 至少被一个键引用）。缺失/陈旧/遮蔽/未知成因都报错。

**扫描面按结构排除、每条排除付理由**：测试面按**路径段**（含 `crates/*/tests/`、`src/tests.rs`、
`*.test.mjs`）、生成面、判据自身。**定义面不靠具名排除**——`.json` 不在语言表里，schema 凭**语言闭包**
就在扫描集之外，测试侧另有一条断言钉住它（第一版把它写进排除表，**判据自己**报了「陈旧排除」）。

**非空洞注入 F0a–F12**（12 条 + 3 条非空洞断言）：F0a 三类非空／扫描面 > 100 文件；F0b 定义面凭语言闭包
在扫描集外；**F0c 两张台账都承重——清空 `WIRED` 必须报错**（**本判据第一版的真洞**：闭世界只要求
「落在三类之一」，而 [A] 由扫描自动成立 ⇒ 清空台账判据照旧全绿）；F1 把**真实源码文本**里
`store.rs` 的 `["max_buffer_ms"]` 挖掉 ⇒ `deepEqual` 钉死**唯一**一条消息；**F1′ 手工真跑改前/改后**
（真的改成 `["max_buffer_ms_"]`：改前 **30 pass / 0 fail**、改后 **29 pass / 1 fail** 且名指该键，
还原后 **sha256 逐字一致**、复测 30/0）；F2 证据文件不含形状；F3 在册遮蔽真读者；F4 陈旧在册；
F5 缺成因／未知成因；F6 把被钉死的键登记为未接线；F7 新增 `required` 键不接线不登记；
F8 取消钉死（证明 [B] 派生）；F9 容器边界三条；F10 数组字面量 vs 真索引；F11 整行注释不算；F12 陈旧排除。

**门控与计数**：`tests/contracts/schema.test.mjs` **29 → 30**（`tests/registry.test.mjs` 仍 50）；
`cargo xtask` 零改动；`check --profile full` 见 W90 报告 §六。

**明确不做**：**不给 23 个 [C] 键接线**（改变运行时行为 ⇒ 产品轮次）、不把配置面的键接到 `MemoryPolicy`
（ADR 0067 已裁定「判定面收口」）、不新增 JSON Schema 约束这份台账（ADR 0054 遗留 5 的同款判断）、
不动 `tests/pending-decisions.yaml`、不改基线正文、不重排 W 编号。
见 [W90 报告](docs/reports/2026-10-13-w90-config-surface-key-consumption.md)、
[ADR 0068](docs/adr/0068-config-surface-key-consumption.md)、
`openspec/changes/archive/2026-10-13-w90-config-surface-consumption/`。

### W91 配置键的归因（读取点 MUST 绑定到拥有它的 schema）

W91 **不是**接线轮——它把 W90 的键级门控从 `runtime` **一份** schema 扩到 `schemas/config/` **全部 6 份**
（`avatar-2d` 9 ／ `backup` 10 ／ `binding-plan` 6 ／ `provider` 12 ／ `upgrade-package` 22 + `runtime` 37）。

**为什么不能原样扩面（实测）**：W90 的读取形状是**裸键名** `EXPR["<key>"]`，**不带对象上下文**。96 个
受判键里 3 个裸名跨 schema 重名（`schema_version` ×5、`mode` ×3、`currency` ×2），通用名词**跨对象污染**
——实测 `["sample_rate"]` 命中 `apps/render-worker`、`["session_id"]` 命中 `crates/nai-control-api`、
`["path"]`／`["sha256"]`／`["id"]` 命中 `crates/nai-plugin-sdk`，全是**同名不同物**。⇒ **判定单元改成
`(schema, 键路径)`**（标签 `schemas/config/<x>.schema.json#<键路径>`），且读取点 MUST 落在**归因文件集**内。

**归因文件集（派生）**：`attribution(schema)` = Rust `include_str!` 该 schema 的文件 ∪ **具名外部读者**
（经 `composition.config()` 拿 `Value`）。实测 `avatar-2d → plugins/avatar-2d/src/lib.rs`、
`backup → snapshot.rs`、`provider`/`runtime → store.rs`、`upgrade-package → manifest.rs`、
**`binding-plan` → 无**；`runtime` 另加 `composition.rs`（外部读者）与 `pipeline.rs`（外部读者）——
正是 W90 的 [A] 里**唯二**跨出 `store.rs` 的条目。每条外部读者 MUST **自清理**。

**读取形状三态（必须全部实现，少一种就整族假阴性）**：① `index`——`EXPR["<key>"]`，顶层要求 `[` 前是
**表达式字符**（排除 Rust 数组字面量 `&["knowledge"]`），嵌套要求紧邻锚是 `["<parent>"]` **或** `<parent>`
（标识符 = **中间变量**写法，如 `let media = &config["media"]; media["lease_renew_ms"]`），`parent` =
键路径**倒数第二段**（派生）；② `serde`——`let x: T = serde_json::from_value(...)` 且 `struct T` 定义在
**同一文件** ⇒ `T` 字段名 ∩ 受判键名（**判定域**；实测 `struct Manifest` 与 `struct Config` **同在**
`plugins/avatar-2d/src/lib.rs`，拦下 Manifest 字段的是判定域而非同文件）；③ `helper`——**读键辅助函数**
（函数体含变量下标 `x[key]`，**或**传递调用已判定者，如 `amount` → `text`）以字符串字面量为实参调用
（`amount(&value, "install_bytes")`）。MUST NOT 推广成「任何带字符串实参的调用」。

**实测三分类**：**96 = [A] 有读取点 49 + [B] 值被 `const` 钉死 13 + [C] 既无读取点也未被钉死 34**；
形状分布 **serde 7 / index 28 / helper 14**。**关键回归**：`runtime` 的 37 键在扩面后 = **10 / 4 / 23**，
与 W90 **逐项一致** ⇒ 扩面没有改变既有判定（已写成测试断言 `[37, 10, 4, 23]`）。

**[C] 的成因 8 条**：`constructor-injected` 12、`hardcoded-default` 1、`surface-absent` 7（W90 的 5 +
`provider.region`／`provider.data_policy_ref`）、`same-name-other-object` 6（`cloud.*` 3 + `provider.timeout_ms`／
`max_concurrency` 命中**技能**字段、`provider.max_retries` 命中**知识源**字段）、`binding-consumer-absent` 1、
`awaiting-decision` 1、**`validator-only` 5**（`binding-plan` 只有 Ajv 校验、**无归因文件**）、
**`write-only` 1**（`backup.created_at_utc_ms` 写入侧生成、读侧从不读）。

**判据（五条，全部在既有 `tests/contracts/schema.test.mjs` 里跑 ⇒ `xtask` 零改动）**：`partitionErrors`
（三分类双向闭世界）／`evidenceErrors`（[A] 证据文件真有形状且被跟踪）／`causeErrors`（成因 id 在册且
成因表每条被引用）／`attributionErrors`（归因集自清理）／`scanExclusionErrors`（陈旧排除即报）。受判面
**派生自磁盘**（`schemasOnDisk('schemas/config')`），MUST NOT 是手写清单。

**非空洞注入 F1–F17 + 6 条非空洞断言**（受判面派生自磁盘、三类非空、三形状都出现、`binding-plan` 是唯一
无 `include_str!` 消费者的 schema、runtime 回归、`.json` 语言闭包、两张台账承重）。**F1 与 F12/F13 手工
真跑改前/改后**：真实挖掉 `store.rs` 的 `["max_buffer_ms"]`、`lib.rs` 的 `max_buffer_samples`、
`manifest.rs` 的 `"install_bytes"` ⇒ **改前 30 pass / 0 fail**、**改后 29 pass / 1 fail** 且**各只指名那一个
键**、**还原后 sha256 逐字一致**、复测 30/0。

**门控与计数**：`tests/contracts/schema.test.mjs` **30 → 30**（**替换而非新增**；`tests/registry.test.mjs`
仍 50）；`cargo xtask` 零改动；`check --profile full` 见 W91 报告 §六。

**明确不做**：不接线任何新增 [C] 键（产品决策）、不做跨语言（TS／生成面）的键级消费、不给 `binding-plan`
设具名例外（它是**事实登记**而非豁免）、不新增 JSON Schema 约束这份台账、不动 `tests/pending-decisions.yaml`、
不改基线正文、不重排 W 编号。**`provider.*` 的同名张力**（配置面 3 键的**同名者**在别处**真的被读**）已
登记 ⇒「配置里有这个键」MUST NOT 被读成「这些策略已生效」。
见 [W91 报告](docs/reports/2026-10-13-w91-config-key-attribution.md)、
[ADR 0069](docs/adr/0069-config-key-attribution.md)、
`openspec/changes/archive/2026-10-13-w91-config-key-attribution/`。

### W92 清空 `decidable_in_repo`（A2A 首版边界落规格、画像验证订正）

W92 **不是**接线轮——它把 `tests/pending-decisions.yaml` 里 `decidable_in_repo` 一态**清到 0**：
该态的必填字段是 `recommendation`（**"建议"**），判据只钉住「有一个建议」、**不钉**「建议落地了没有」。
三条各走一条路：

- **PD-22（入站是否对第三方开放）与 PD-24（委派深度是否可配置）落到同一段活规格**
  `openspec/specs/agent-harness/spec.md` 的 `A2A inbound is an external request; outbound delegation is bounded`
  ⇒ 处置迁 `decided_in_repo`、`landing` 指该规格文件。规格补两句：**R1 首版入站只响应本机与本地信任配置中
  已登记的受信对端**（原正文只说「对端身份由本地信任配置**派生**」——那约束"身份从哪来"、**不约束"谁可以到达"**，
  是实现可以"接受任意第三方再查身份"的那道缝）、**R1 首版委派链深度上限 MUST 为常量、MUST NOT 提供配置面**
  （原正文写「深度 MUST 有上限（**初值 1**）」——"初值"恰好暗示可调，而 PD-24 要裁的正是那个自由度）。
- **PD-34（用户画像的验证方式）订正为 `needs_external`**：基线 §54.2 标题即「初版，未经验证」、§57.5 明写
  「MUST 在 R1 实现期以**真实使用者验证**」⇒ 验证方式已被基线答出，未闭合的是**执行**（真人访谈）——
  必然在**仓外**。原登记 `decidable_in_repo` 是**不实分类**；也不是 `decided_in_repo`（闭合条件在仓库里不存在）。

**不越权**：基线 §46.3 的「初值 1」是**一般描述**、规格的「首版常量」是 **R1 具体化**，两者分层不矛盾
⇒ 基线正文本轮**一字不动**；§48.7 本就把它登记为待决策 ⇒ 规格补上是从"待决"变"已决"。

**「清空一个态」暴露的守卫隐含依赖（本轮教训）**：两处测试用例按**处置**定位样本
（`find(entry => entry.disposition === X)`），态一空即失去作用对象（实测立刻 **2 条红**：#16、#22）。
危险的修法是 `if (!item) continue`（把「该态没有实例」与「该字段不再被扫」混为一谈）；
采用的修法是**样本定位改为按字段存在**（`Object.hasOwn`），与 W80／W83／W88 的
「触发条件 MUST 只由**字段是否存在**决定」同源，并新增空态的双向一致断言与本轮成果断言。

**非空洞注入 F1–F8**（改真实台账 → 跑真实测试 → 抓具名 → 还原校验）：F1 具名串
`PD-22: landing crates/nai-security/src/no-such-file.rs is not a file tracked by this repository`、
F2 `PD-34: closure references §99.9, which does not exist in the baseline`、
F4 `PD-24: disposition decided_in_repo requires a non-empty landing`；
F5 用**工作区有、索引无**的 `node_modules` 证明判据读的是索引；F6／F7 反向（合法 landing／closure
挂到**别的**处置上）**不红**；F8 清空态后真跑 **50/0 全绿**；每次注入后 **sha256 逐字还原**。

**门控与计数**：`tests/registry.test.mjs` **50 → 50**（**替换而非新增**）；`DISPOSITION_COUNTS`
`decidable_in_repo` **3 → 0** / `decided_in_repo` **2 → 4** / `needs_external` **20 → 21**（总数仍 **34**）；
`cargo xtask` 零改动、**Rust 零改动**；`check --profile full` 见 W92 报告 §六。

**明确不做**：不实现 A2A（H13／H14 仍 `capability_pack` + `pending`，INV-33）、不改基线正文、
不改任何验收 `status`、不给"深度上限"加无消费者的常量（W73 假契约教训）、不重排 W 编号。
**落地是规范层的**——MUST NOT 被读作"A2A 已可用"（`delegation_depth` 在 Rust 仍零命中）。
见 [W92 报告](docs/reports/2026-10-13-w92-decidable-in-repo-settlement.md)、
[ADR 0070](docs/adr/0070-decidable-in-repo-settlement.md)、
`openspec/changes/archive/2026-10-13-w92-decidable-in-repo-settlement/`。

### W93 `decided_in_repo` 的落地点 MUST 带内容证据（`landing` 补锚点）

W93 补的是 **W92 自己造大的那个风险**：`decided_in_repo` 断言「决定已作出**且已落地**」，而判据只验
`landing` 的每个元素是**仓库已跟踪路径**——「落在哪」被验了，「**落在那里的东西还在不在**」没人验。
两个探针实测（改真实文件 → 跑 `node --test` → 还原）：① `PD-22.landing` 换成 `README.md`（真实存在、
已 git 跟踪）⇒ **50/50 全绿**；② `landing` 不动、删掉规格里 `PD-24` 的落地句 ⇒ **50/50 全绿**。
⇒ 「已落地」是一条**判据放过的空声明**（「已声明 ≠ 已执行（无执行体即假契约）」的形状）。

新增字段 **`landing_anchor`**：与 `landing` **等长、逐项对应**，第 *i* 个锚点 MUST 真实出现在第 *i* 个
落地点文件的**内容**里。设计上三处取舍：**平行数组**（不把 `landing` 改成对象数组——那会重写
`REPO_PATH_FIELDS` 的解析器并改 4 条既有数据形状）、**第五类检查 `ANCHOR_FIELDS`**（锚点是"文件内容
证据"，不是 `§NNN`／id／路径／结构字段中的任何一类，且与四类**两两不相交**）、**耦合断言
`COUPLED_REQUIRED_FIELDS`**（`landing` 与 `landing_anchor` **同生同灭且等长**——锚点若可选，"不给锚点"
就是绕过判据的那条缝）。判据新增依赖 `readTrackedText`，**读不到文件 MUST 具名失败**
（「没看到」与「出错了」分开，MUST NOT 静默降级）。触发条件一律**按字段存在**，与 `disposition` 无关。

**四条锚点**（逐条实测）：PD-09 → `[target.'cfg(windows)'.dependencies]`／`NativeVault`／
`secret-protection`；PD-22 → `R1 首版入站只响应本机与本地信任配置中已登记的受信对端`；
PD-24 → `R1 首版委派链深度上限 MUST 为常量`；PD-31 → `viewer_memory_enabled: false`。
PD-22／PD-24 的**落地点是同一个文件、锚点不同** ⇒ 正说明「落地点相同 ≠ 决定相同」。

**锚点守什么、不守什么（如实披露）**：把门槛从"路径存在"提高到"**所指文件里真有那段内容**"，
但它**不**验"这个串只应出现在那里"（指到另一个恰好含该锚点的文件仍绿），也**不**验那段内容
**语义上**兑现了决定（仍由人读，ADR 0054 决策 2）⇒ MUST NOT 被读作"已落地已被证明"。

**本轮合法地改了既有用例的前提**：W88 的反向用例「可解析 `landing` 挂别的处置上 ⇒ 不红」注入时
只给 `landing` 不给锚点 ⇒ 现在会红（原因是**缺内容证据**而非"放错位置"）⇒ 补上锚点后仍验原意。
与 W92 的「清空一个态暴露隐含依赖」同族：修法是**补齐前提**，MUST NOT 用短路把判据关掉。

**非空洞注入 P1/P2 + F1–F8**（全部真跑 + sha256 还原校验 OK）：**P1／P2 由绿转红是本轮成果**；
F1 锚点不在文件里、F2 长度不等（`correspond one-to-one`）、F3 有 landing 无 anchor、F4 空串、
F5 挂别的处置 ⇒ **红**；F6 反向（合法锚点挂别处）**不红**；F7 闭世界漏登记
（`unknown field landing_anchor`）、F8 类别并集漏掉 `ANCHOR_FIELDS` ⇒ **红**。
⚠️ 注入中的两处自我纠错：F1 探针值一度选了 `viewer_memory_enabled: true`（它**真的存在**于
`memory.rs` ⇒ 假阴性）；F7 的替换只替换注释前缀、把后半句留成裸代码 ⇒ `pass=0 fail=1` 是**注入自身**的错。

**另两态实测无可订正的数据（本轮未动）**：`answered_by_baseline` 4 条 `basis` 都真的答出了（§46.2
sampling 默认禁用、§41.4 不设阈值、§46.5 不新增契约名、§51.3 清单逐行独立）；
`needs_precondition` 4 条前置**全部未达成**（H05/06/07/08/13/14/16 与 W61 的 H11/H12 均 `pending`）。

**门控与计数**：`tests/registry.test.mjs` **50 → 51**（新增 1 个用例）；`LEDGER_FIELDS` **11 → 12**；
`shapeErrors()` 的类别划分**四类 → 五类**；`cargo xtask` 零改动、**Rust 零改动**。
**明确不做**：不实现 A2A、不改基线正文、不改任何验收 `status`、不把锚点做成"全仓唯一"（无法判定
且会把判据变脆）。
见 [W93 报告](docs/reports/2026-10-13-w93-landing-content-anchor.md)、
[ADR 0071](docs/adr/0071-landing-content-anchor.md)、
`openspec/changes/archive/2026-10-13-w93-landing-content-anchor/`。

### W94 `needs_precondition` 的「还在等」MUST 是真的（前置补达成状态）

W94 堵的是同一族缝的**第三种形状**（W93 堵「已落地」）：`needs_precondition` 断言「这条现在还不能
裁定，因为它在等前置」，而判据只验 `precondition` 的每个 id **解析得开**。探针实测：把
`PD-26.precondition` 换成 `["S03"]`——`S03` 在 registry 里是 `status: tested`、**早已达成**
⇒ **51/51 全绿**。⇒ 「还在等」是一条**判据放过的空声明**；而六个态里只有这一个态的
**有效期会随仓库自身进展而过期**（前置达成了，台账不会有任何信号，条目会一直挂着）。

判据新增：**每个前置 MUST 仍未达成**，已达成 ⇒ 具名失败。四处设计取舍：**注入事实而非判定**
（`acceptanceStatus` + `workItemAcceptance` 两张表；若注入 `preconditionMet(id)` 这类判定函数，
注入方算错时判据照样绿 ⇒ **守卫自我满足**——与 `repositoryFiles`、`readTrackedText` 是
**同一个陷阱的第三次出现**）、**`W\d{2}` 经关联验收派生**（基线工作项表**没有状态列**，只有
`编号|内容|关联验收|发布线` ⇒ 措辞只说「`W61` 的关联验收 `H11/H12` 均已达成」，
MUST NOT 冒充「W61 已完成」）、**取值域闭世界两道**（`MET_STATUSES` ∪ `UNMET_STATUSES` 必须
**恰好划分** `ACCEPTANCE_STATUSES`，否则将来新增一个 `status` 取值时判据会**静默地不认识它**
——与 W92「清空一个态」同族）、**不可解析 ⇒ 具名失败**（工作项表 36 行里 **11 行（W00–W10）
第 3 列是路径／包名**，无关联验收 ⇒ 不可判定）。

**`not_applicable` 归「未达成」，依据实测反例而非字面直觉**：`S12` 的 `notes` 明写「依赖真实平台
规则核对」⇒ 它**仍未闭合**；把它算成已达成会漏掉整整一类「不适用但仍悬空」的前置。

**本轮合法地改了既有用例的前提**（第二次出现）：W88 的反向用例取 `workItemIds` 的第一个元素
（实际是 `W00`），而 `W00` 现在**不可判定** ⇒ 改用「有非空关联验收、且那些验收全部仍未达成」的
工作项（实测 `W61`）——不是放宽断言，是把样本换成**本轮口径下真正还悬空**的那一个。

**非空洞注入 P1 + F1–F8**（全部真跑 + sha256 还原校验 **ALL-OK**）：**P1 由绿转红是本轮成果**；
F1 另一条已达成、F2 W 派生路径、F3 不可判定（`W00`）、F4 按字段触发（`PD-01` 是 `needs_external`）
⇒ **红**；F5 `S12`（`not_applicable`）**不红**、F6 合法 `pending` **不红**；F7 分类认领域外取值、
F8 域里没人认领 ⇒ **红**。
⚠️ 一处自我纠错：F7 第一次选的值不对——把 `tested` 从取值域拿掉后它仍在 `MET_STATUSES` 里，
运行时不会判成域外，报红的是 `shapeErrors` 的**另一个方向**；修正后 F7 改验方向②，
**运行时**遇域外 `status` 由测试 ⑤（`domainBreach.set('H05','deferred')`）覆盖。

**零数据改动**（纯判据）：4 条前置当前全部 `pending`（H05/06/07/08/13/14/16 与 W61 的 H11/H12）
⇒ 补判据后仍全绿。**门控与计数**：`tests/registry.test.mjs` **51 → 52**；必填依赖 **6 → 8**；
`LEDGER_FIELDS` 仍 12（不新增字段类别——达成状态是**同一字段的第二条断言**）；Rust 零改动。
**另两个候选实测无可做的事**：21 条 `closure` 确实闭合在仓外；`question` 逐字对账**过脆**
（34 条中 11 条未命中 ⇒ 会造 11 条假阳性）。
见 [W94 报告](docs/reports/2026-10-13-w94-precondition-freshness.md)、
[ADR 0072](docs/adr/0072-precondition-freshness.md)、
`openspec/changes/archive/2026-10-13-w94-precondition-freshness/`。

### W95 CI 工作流的每个 step MUST 具名在册（判定的消失不能没有信号）

W95 换了一层对象：`cargo xtask check --profile full`（本机门控）与
`.github/workflows/foundation.yml`（CI）是判定的两个执行入口，而**后者多跑一批判定步**
（`schema --check`、8 个 `test --suite`、含 `descriptor.test.mjs` 的契约步、`spec:validate`、
`openspec validate --archived`），**这个集合没有任何在册台账**——它只存在于注释里
（`tests/contracts/schema.test.mjs` 那句「本地门控从不读它」是一句**解释**，不是判据）。

两个探针实证：①往 workflow 插一个 step ⇒ 本机 3 套件（schema 30 / platform 22 / registry 52）
**全绿、零信号**；②**删掉** `schema --check` 这个判定步 ⇒ **同样全绿**——一个判定**静默消失**。
（`grep -rn foundation.yml crates/ xtask/` 为空 ⇒ 无替代守卫；`test-reachability` 只从 workflow
读**一个 marker**，其余 step 不在受判范围。）这与 W84「测试文件静默不跑」同族。

判据：**文件层 + 步层两层闭世界**。步的**身份**由解析得出（有 `name` 用 `name`，否则 `uses`／
`run` 首行）且 MUST 唯一；每步 MUST 归入闭集四类之一——`infra`／`local`／`ci-only`／`diagnostic`。
四处设计取舍：**`local` 必须付代价**（引出的 `{file, marker}` MUST 在门控源里逐字存在——W75
「已声明 ≠ 已执行」）、**`ci-only` 必须付代价且双向**（成因 ∈ 闭集四格：`host-subprocess`／
`compiled-artifact`／`platform-toolchain`／`not-in-profile`；且 MUST NOT 同时挂本机凭据）、
**`infra` MUST NOT 是后门**（`run` 命中判定命令形状 ⇒ 红）、**`diagnostic` 由结构认出**
（`if` MUST 含 `failure`——`runner.os == 'Linux'` 过不了这条）。

**分类是静态声明，MUST NOT 由运行时探测得出**（铁律二）：`EBUSY` 是宿主事实，换一台机器会对
同一个仓库得出不同分类；且「能跑但没接进门控」（`spec:validate`）与「根本跑不了」
（`schema --check`）风险不同、成因不同，运行时探测区分不出。

**非空洞注入 P1/P2 + F1–F8**（真实文件 + sha256 还原校验 **ALL-OK**）：**P1（加法）与 P2（减法）
由绿转红是本轮成果**；F1 未登记判定步、F2 删已登记步、F3 改名（两侧同时报）、F4 凭据指向不存在的串、
F5 成因域外、F6 ci-only 挂凭据 ⇒ **红**；F7 换成另一处**真实存在**的落点 ⇒ **不红**；
F8 新增一份未登记 workflow 文件（真文件 + `git add`）⇒ **红**。
**与 `test-reachability` 的边界实测**：删它的站点 ⇒ workflow 判据 **0 红** / registry **2 红**；
删一条 `WORKFLOW_STEPS` ⇒ workflow **5 红** / registry **0 红** ⇒ 二者**互不顶替**
（前者管「workflow 的 step」，后者管「测试文件的可达性」）。

**本轮有意的耦合**：新增测试文件必须**同时**在 `xtask/src/lib.rs` 登记、在 `test-reachability`
的站点表登记，否则 W84 的闭世界与 `registry.test.mjs` 的逐字清单**同时红**——这正是它该有的行为。

**明确不做**：不验「CI 真的跑了某步」（那是运行结果）；不验 `ci-only` 的成因是否成立（语义，
仍由人读）；不改 `foundation.yml` 一个字符。
见 [W95 报告](docs/reports/2026-10-13-w95-workflow-step-closure.md)、
[ADR 0073](docs/adr/0073-workflow-step-closure.md)、
`openspec/changes/archive/2026-10-13-w95-workflow-step-closure/`。

### W96 `cargo xtask` 的每一条命令 MUST 具名在册（usage 与 dispatch 必须对账）

W96 接着 W95 换**另一层对象**：W95 管「**CI 侧**的执行面」（`foundation.yml` 的 step 集合），
W96 管「**本机门控的入口面**」——`xtask/src/lib.rs` 的 `execute()` 这个
`match args…as_slice()` 的 **24 条臂**。它**住在同一个文件里**、与 `check --profile full`
的门控闭包相隔几十行，却**没有任何在册台账**；同一文件里的 `Usage: cargo xtask …` 字符串是
命令集合的**第二次书写**，两处没有任何机制要求一致。

三个探针实证（改真实 `lib.rs` → 跑本机 4 套件 → 还原 → `sha256` 逐字校验）：①插一条臂
`["check", "--audit"] => Ok(())` ⇒ registry 52 / schema 30 / platform 22 / workflow 13
**全绿、零信号**；②**删掉** `["check", "--secrets"]` 这条判定命令 ⇒ **同样全绿**（一条判定
**静默消失**）；③usage 字符串追加 `| check --audit` ⇒ **同样全绿**（usage 与 dispatch **分家**）。
（`grep -rn "Usage:" tests/` 为空 ⇒ 无替代守卫。）

判据：**命令面闭世界 + usage 双向对账 + `gate` 付代价**。
判定单元 MUST 是**命令签名**（空格连接的顶层字面量序列），MUST NOT 是裸命令名——W91 的教训
「同名」≠「同一个」：`check --golden` 与 `check --golden --dir` 是**两条**命令，签名区分它们；
匹配锚是整条签名，不是 `"--secrets"`（后者会在 `secrets::check` 这类**别的对象**上命中）。
**usage 与 dispatch 对账**（不是消除第二处——usage 是自然语言，不可能由 match 臂渲染）：
`usage ⊆ dispatch` 且 `dispatch \ usage ⊆ HELP_SIGNATURES`（三条具名豁免 `help`／`--help`／空串；
少写一条**判定命令**是缺陷，少写 `help` 不是）。四类闭集：`gate`（执行体在门控闭包里被执行）／
`standalone`（是判定命令但不在闭包里）／`artifact`（生成产物、不产生判定）／`help`。

**`gate` 的代价是「落在门控闭包内」，不是一个「逐字存在」的串**：`evidence.marker` MUST 在
**门控区域**（`engineering_checks` 的函数体 ∪ 各 `check --profile` arm 体，由**解析**得出）
内逐字存在。只要求「在文件里逐字存在」的话，marker 就能填 **arm 自己**的文本，把「本机门控
真的跑它」偷换成「这条命令自己存在」——**自我满足**的典型形态。这是本项目「注入事实而非判定」
这一族的**第五次出现**（`repositoryFiles` → `readTrackedText` → W94 两张表 → W95 `gateSources`
→ 本轮 `gateRegions`）。`standalone` 的成因取自闭集四格：`host-subprocess`／
`requires-compiled-artifact`／**`superseded-by-workspace-run`（本轮新增**：这 8 条不是「跑不了」，
而是「门控跑的是它们的**超集**」）／`deliberate-on-demand`（ADR 0043）。

**分类是静态声明，MUST NOT 由运行时探测得出**（铁律二）；**不可解析 ⇒ 具名失败**；dispatch／
usage／某一类为空 ⇒ **显式报错**（空态 MUST 被显式断言，W92 同族）。

**非空洞注入 P1–P6**（真实文件 + `sha256` 逐字还原 **ALL-OK**，每个注入都产生**具名**错误）：
P1 未登记臂 ⇒ `dispatches the command "check --audit", which nobody declares`；P2 删命令 ⇒
`a registered command that vanished is a judgement that silently disappeared` + `usage must not
promise what does not exist`；P3 usage 漂移 ⇒ `the usage string advertises "check --audit"`；
P4 把 `check-deps` 的落点移出门控闭包 ⇒ `which is not inside the local gate closure
(engineering_checks, arm@…)`；P5 删 `windows-local` arm 的独有落点 ⇒ 同上；P6 usage 少写
`sbom [--out <path>]` ⇒ `dispatched but never appears in the usage string`。

**与 W95／W84 的边界实测**（改真实表 → 跑两个套件 → 还原）：删一条 `WORKFLOW_STEPS` ⇒
workflow **2 红** / xtask-commands **0 红**；删一条 `XTASK_COMMANDS` ⇒ xtask-commands **4 红** /
workflow **0 红**；删新增的 `test-reachability` 站点 ⇒ registry **2 红** / xtask-commands **0 红**
⇒ 三者**互不顶替**（分别管「workflow 的 step」／「`execute()` 的 CLI 命令」／「测试文件的可达性」）。

**本轮有意的耦合**：新增测试文件必须**同时**在 `xtask/src/lib.rs` 登记、在 `test-reachability`
的站点表登记、并在 `registry.test.mjs` 的逐字清单里出现，否则三处**同时红**——这正是它该有的行为。
`tests/contracts/xtask-commands.test.mjs` **17 个用例**、`test-reachability` 站点 **8 → 9**、
**Node 套件 6 → 7**、`xtask/src/lib.rs` 只加一行登记（**Rust 零行为改动**，`execute()` 与 usage
一个字符未动）。

**明确不做**：不验「CI 真的跑了某条命令」；不验 `standalone` 的成因是否成立（语义，仍由人读）；
不验「`check --profile portable` 的证据专属于它」（如实披露的假阴性方向宽松）。
见 [W96 报告](docs/reports/2026-10-14-w96-xtask-command-surface.md)、
[ADR 0074](docs/adr/0074-xtask-command-surface.md)、
`openspec/changes/archive/2026-10-14-w96-xtask-command-surface/`。
（W97 就地订正：W96 段原写 `openspec/changes/2026-10-14-w96-…`，归档后真路径在 `archive/` 下。）

### W97 被测面的集合 MUST 由解析算出并交叉核验

W97 接着 W95／W96 补**层与层之间的接线**：前两轮的闭世界各自成立，但它们之间的包含关系
没有被计算过。「门控测哪些包」与「每个 `test --suite` 测哪些包」都由**实参**决定——而 ADR 0073
那 8 条 suite step 的 `localEvidence` **全部填了同一条 marker**（`--workspace --exclude HARNESS`）。
一条 `--workspace` 的证据回答的是「全 workspace 被跑了」，**没有一条**回答「这个 suite 自己的包集
被跑了」——**用超集冒充专属**。这是同一个陷阱的**第六次**出现（`repositoryFiles` → `readTrackedText`
→ W94 两表 → W95 `gateSources` → W96 门控闭包 → 本轮：包含关系必须**算出来**）。

三个 Mutation 实证（改真文件 → 真跑 → `sha256` 逐字还原）：①M1 把门控的
`--workspace --exclude HARNESS` 换成 `-p nai-storage` ⇒ **9 个 suite 全部失去覆盖 +
17 个成员不再被测**，零信号；②M2 某个 suite 的 `-p` 改成幽灵包名 ⇒ 本机与静态判据都不发现；
③M3 往 `members` 加成员 ⇒ 「它的测试有没有被跑」无人回答（**如实**：`--workspace` 形态**自适应**
于成员集合，`cargo test --workspace` 确实会跑到新成员 ⇒ 判据**不**该报警；真正的缺口在
「门控改用显式列表并漏掉某个成员」那一天，已由测试用例钉住）。

判据（新 `scripts/contracts/gate-coverage.mjs`）：**V1** suite 登记表与 dispatch 派生的 suite 集合
**双向**闭世界；**V2** 包集由 driver（`inline`／`delegated` 穿透 `schema::contracts`）从源码算出，
登记表**不写第二份**（W93）；**V3** 每个 `-p` 值 MUST 落在真实成员包名上（`HARNESS` 由
`xtask/Cargo.toml` 派生，不写死）；**V4** 每个 suite 的包集 MUST 被**单一条**门控命令覆盖
（不许用并集凑）；**V5** 每个成员 MUST 被这些命令的并集覆盖；**V6** CI 的每条 xtask 命令
MUST 在 dispatch 在册（复用 W96 的 `dispatchSignatures`，**不写第二套解析**）。

**空态 MUST 被显式断言**（W92，本轮实测出来的）：driver 标错形态时包集算成 **∅**，而 inclusion
对空集**恒真** ⇒ 加「MUST 至少解析出一条 test 命令」与「包集 MUST 非空」两条。**注释不是实参**：
评论区一句 `[…]` 形状的例子曾被当成实参数组去解析 ⇒ 加 `stripRustComments`（且须跟踪字符串状态，
`"https://x"` 里的 `//` 不是注释）。

`tests/contracts/gate-coverage.test.mjs` **24 个用例**、`test-reachability` 站点 **9 → 10**、
**Node 套件 7 → 8**、`xtask/src/lib.rs` **+1 行登记**（Rust 零行为改动）。
**明确不做**：不判「每条 suite 命令 MUST 有 CI step」——`test --suite upgrade` 在 CI 没有具名 step，
而仓库里**无任何记载**说明原因 ⇒ 给它配成因等于**给未知事实贴标签**（W90），只作为发现登记。
见 [W97 报告](docs/reports/2026-10-14-w97-gate-coverage.md)、
[ADR 0075](docs/adr/0075-gate-coverage.md)、
`openspec/changes/archive/2026-10-14-w97-gate-coverage/`。

### W98 npm 脚本的每个脚本 MUST 有具名处置（执行面：谁调用它）

W98 接着 W95／W96／W97 换**第四层对象**：前几轮管的是 CI 的 step 集合、`cargo xtask` 的命令面、
Rust 侧的包集合，而**npm 脚本的执行面**完全在外面——3 份 `package.json`、13 个脚本，每个都声称
做了某件事，却**没有任何判据**回答「**谁调用它**」。三个 Mutation 实证零信号（改真文件 → 真跑现有
6 个 portable 套件 → 备份拷回 → `sha256` 逐字还原）：① 往根 `package.json` 加
`"typecheck:fast": "node scripts/nowhere.mjs"`、② 把 `frontend-gate.mjs` 的 `STEPS` 里
`['apps/desktop','test']` 改成不存在的 `'tests'`、③ 加自引用 `"test:loop": "pnpm test:loop"`
——三者都是 **158 passed / 0 failed**。**如实记录的反例**：M1（CI 里 `pnpm spec:validate` 拼错成
`spec:validat`）**会红 2 条**，因为 ADR 0073 的 `localEvidence` marker 逐字比对抓到了——但它抓的是
「**那一行文本变了**」，不是「**那个脚本没有消费者了**」，**同一处文本、两种语义**。

四条纪律：① **判定单元 MUST 是 `(包目录, 脚本名)` 的配对**（W91）——`typecheck` 在三个包里同名、
`build` 在两个包里同名，键用 `dir\0script`，删一条 MUST **只**点名那一条；② **文档不是调用**：
`.md` MUST NOT 进引用扫描面（文档只能被「写错」证伪，把它当消费者，两个从不运行的脚本会**永久
绿灯**）⇒ `manual` 成因的证据 MUST 是 `.md` 且逐字存在；③ **自引用 MUST NOT 充当消费者**
（全 `package.json` 自动并入扫描面，因为**脚本互相调用是真实的**——根包的
`pnpm --recursive typecheck` 是一条**跨包**引用——**自引用排除与这个并入是一件事的两面**）；
④ **成因闭集按「可判定性」划分、MUST NOT 按语义划分**：3 格（`covered-by-driver`／`manual-alias`／
`manual`）的**判定域两两不同**，「冒烟」与「开发服务器」**不拆**（证据形态同为「文档里一行」，
拆开会让合法改签不被发现 ⇒ 两格不可区分 ⇒ 分类退化成标签）。

**两条硬点**：**`covered-by-driver` 的递归委托只覆盖同名脚本**（`pnpm --recursive typecheck`
不被 `build` step 认证——少了这条，「委托」会变成万能通行证）；**把运行时拒绝静态化**——
`frontend-gate.mjs` 只在运行时拒绝未知可执行名与未实现 shell 语法，而静态判据**只对驱动表里那 6 项**
复算同一件事（根包其余脚本头 token 是 `pnpm`／`openspec`／`cargo`，它们**本来就不经**该门控 ⇒
拿这把尺子量它们是**用错了对象**）。

**两条判据缺陷是本轮测试抓出来的（真收获）**：① 一行里两条命令（`pnpm a && pnpm b`）时贪婪
`[^\n]*` 把第二条吞进第一条 token ⇒ **第二条引用静默消失**（**假阴性，危险方向**）⇒ 改非贪婪
`[^\n&|;]*`；② `packages` 解析失败后仍被 `packages.keys()` 访问 ⇒ 崩而非具名。⇒ **凡「解析多行／
多命令」，MUST 用一行两条的样本验过**。另：「注释不是引用」的泛化换形——原始正则命中 4 行，
剥注释后**只有 1 行**是真引用（其余是文档注释与 `tests/registry.yaml` 的 `notes` 散文）。

`tests/contracts/npm-scripts.test.mjs` **34 个用例**、`test-reachability` 站点 **10 → 11**、
**Node 套件 8 → 9**、`xtask/src/lib.rs` **+1 行登记**（Rust 零行为改动，**被审对象一个字符未动**）。
注入 **P1–P15** 全红且逐字还原；边界 **b1–b5 = 4/2/4/3/2** 与 W95／W96／W97／站点清单**互不顶替**。
**如实披露**：`--filter P` 形态在引用扫描面里实例数为 **0**（只在 `docs/**`）⇒ 该分支当前只有
测试文件里的注入用例作证据，不写成「已覆盖」。
见 [W98 报告](docs/reports/2026-10-14-w98-npm-script-surface.md)、
[ADR 0076](docs/adr/0076-npm-script-surface.md)、
`openspec/changes/archive/2026-10-14-w98-npm-script-surface/`。
（W98 就地订正：W97 段与 W98 段原写 `openspec/changes/<name>/`，归档后真路径在 `archive/` 下；
全仓扫描后仅此两处失效，`reports/`／`adr/`／`archive/` 属历史记录，未动。）

### W99 站点图层声明（`tier`）MUST 由宿主解析出的区域兑付

W99 接着 W95–W98 换**第五层对象**：前几轮管 CI 的 step 集合、`cargo xtask` 的命令面、Rust 侧的
包集合、npm 脚本的执行面；本轮管 `TEST_REGISTRATION_SITES` 每条站点的 **`tier`**——「**在哪一层跑**」。
W84 把 `host`／`marker` 那一面管住了（逐字验、覆盖集非空、闭世界双向），而 `tier` **只有形状检查**
（`siteShapeErrors` 里的 `TEST_TIERS.includes(site.tier)`）⇒ 一个**未付代价的声明**。探针 P-A 实证
（改真文件 → 真跑 7 个 portable 套件 → 备份拷回 → `sha256` 逐字还原 `f61b2c73…728c`）：把
`tests/contracts/npm-scripts.test.mjs` 的注册从 `engineering_checks` **移进**
`["check","--profile","full"]` arm、站点仍声明 `tier: 'portable'` —— **192 passed / 0 failed，零信号**。
P-A 顺带证明第二件事：**注册点位置本身也没有守卫**（portable 面少跑一个套件，无人发现）。

**`tier` 的语义 MUST 先定死 =「跑它的最低一层门控」**——语义不定，反向断言就无从写起。四条纪律：
① **兑付面由解析得出，MUST NOT 是站点自己那段文本**（`gateRegions`／`driverSteps`＋`packageScripts`／
`workflowStepsOf`）；② **兑付形状由 host 派生，MUST NOT 留可选字段**——`portable` 有两种形态：
host 是宿主文件本身 ⇒ 兑付串 = marker（内联注册），否则 ⇒ `/<module>::\w+\(root/`
（`registry::check(root)?;` 是实例，而 marker 的字面量在 `registry.rs` 里）；**不给 `via`**
（「可选」就是判据的缝，一个可省的字段会让站点悄悄走默认分支）；③ **「出现过」≠「被调用」**——
`apps/desktop` 在驱动表里有 3 个 STEP，只有 `test` 的**脚本值**含 `tests/*.test.mjs`
⇒ `full` 的兑付 MUST 用**脚本体**；④ **反向断言不可省**——`full`／`ci` 的 marker MUST NOT 落在
portable 区域里，否则「把 marker 混进 portable 面、站点仍声明 `ci`」成了一条**合法的逃离路径**
（「最低一层」的语义恰好定义了这个方向）。

**注释陷阱本轮换一层、再实测一次**：`lib.rs` 第 46 行的**注释**里写着
`tests/contracts/descriptor.test.mjs` ⇒ `gateRegions` 的 `engineering_checks` 区域**原文含**、
**剥后不含**（实测 `raw=true / stripped=false`）⇒ **消费 `gateRegions` 的一方 MUST 自己
`stripRustComments`**（`gateRegions` 有意不剥），否则两个 `ci` 站点会被这条注释**自我满足**、
反向断言同时失效。取值域闭世界**两道**：`Object.keys(TIER_PAYMENTS)` MUST 等于 `TEST_TIERS`
（新增取值而不归类 ⇒ 静默地不认识它）＋ 每个取值 MUST 至少有一个站点（无实例的规则恒真）。

`tests/contracts/tier-payment.test.mjs` **22 个用例**、`test-reachability` 站点 **11 → 12**、
**Node 套件 9 → 10**（portable 面 **192 → 214**）、`xtask/src/lib.rs` **+1 行登记 + 一段注释**
（Rust 零行为改动，**被审对象一个字符未动**：`frontend-gate.mjs`、workflow、`package.json`×3、
既有 11 条站点）。注入 **P1–P6** 全红且逐字还原（`lib.rs` `4ac2bbc1…9e05`、
`foundation.yml` `2f46d5f5…8c1c`）；边界（W84／W95／W96／W97／W98／W99）=
P1 `0/0/0/0/0/3`、P2 `1/0/0/0/0/3`、P3 `0/0/2/0/0/3`、P4 `0/0/0/0/0/3`、P5 `0/0/0/0/0/3`、P6 `0/2/0/0/0/0`。
**另记一条流程缺陷（本轮踩到并当场修掉）**：探针循环若在**上一次被中断的**残留改动上取备份，
`RESTORE OK` 会退化成「把坏状态备份拷回」的**假绿** ⇒ **每轮 MUST 先按索引复原并断言干净，再取备份**。
**如实披露**：`portable` 的拒否靠**剥离注释后的文字域**，不是「marker 是否为一次真实调用」
⇒ 把某个 `ci`／`full` 站点的 marker 放进 `engineering_checks` 调用脚本的字符串字面量里，T2／T5 会放行
（**有意的宽松**）；且 `stepIdentity` 对只有 `run` 的步回落为 run 首行 ⇒ `ci` 的「具名 step」近乎恒真，
其实质约束是「某个 step 的 `run` 文本逐字提到它」。
见 [W99 报告](docs/reports/2026-10-14-w99-tier-payment.md)、
[ADR 0077](docs/adr/0077-tier-payment.md)、
`openspec/changes/archive/2026-10-14-w99-tier-payment/`。

### W100 判定步的**执行配置面** MUST 由解析出的平台集合兑付

W100 换**第九层对象**：前八轮各补**一个字段或一个集合**（`repositoryFiles` → `readTrackedText`
→ W94 两张事实表 → W95 `gateSources` → W96 `gateRegions` → W97 包集合 → W98 npm 脚本执行面
→ W99 站点层级声明）；本轮补的是一个**二维面**（`matrix` 的轴 × 每步的 `if:`）**加**一个**开关**
（`continue-on-error`）。W95 的解析器只产 `{identity, condition, run}`，`matrix`／`continue-on-error`
**根本不进判定域**，`condition` 只被 `diagnostic` 分支读过一次（`condition.includes('failure')`）
⇒ **一整个面没有人读**。六个探针（P-A1／A2／A3／A4／A6／B1）8 套件**全 `[0,…]` 零信号**；
P-A5（删诊断步 `if: failure()`）红 **2** 条作**对照组**，证明那些「零信号」不是判据整体失效。

**最危险的一个是矩阵**：`os: [a, b]` → `[b]` 之后，两条 Linux-only 步（其中一条就是三语言契约
套件）**永远不会跑**，而「21 个步都在册、身份都在、`kind` 都在闭集里」——**每一格都还是绿的**，
CI 也照样绿。八条纪律：① **可达性 MUST 是集合级，MUST NOT 是字段级**——`platformLegsOf` 返回
**执行平台腿列表**，`if: runner.os == 'macOS'` 在矩阵没有 macOS 时返回**空列表**（「写了条件」与
「会跑」是两回事；只问「有没有 `if:`」会把它放过去）；② **两层取值域拒否合并不了一处**——
`runner.os == 'FreeBSD'` ⇒ **取值域**（GitHub runner 只报三值）、`runner.os == 'macOS'` 而矩阵
没有它 ⇒ **集合为空**、`matrix.os: []` ⇒ **声明本身空洞**；合并后错误消息就分不清「不存在的平台」
与「矩阵里没有这个平台」；③ **失败语义 MUST 与角色一致，且是双向的**——`local`／`ci-only`
MUST NOT 带 `continue-on-error`（判定的结论必须能红，否则是**装饰**），`diagnostic` MUST 带
（诊断的问题不得**替换**它所转译的那次失败的结论）；仓库里「保证诊断自身的问题不会掩盖或替换
原始失败的结论」那句是**散文**——**注释不执行任何东西，判据才是兑付**；④ **成因与条件 MUST
互相兑现（双向）**——`platform-toolchain` 若没有平台限制，「它需要特定平台」在 CI 上**没有任何
可见后果**；反过来给与平台无关的成因（`host-subprocess` 等）加平台条件，会让该步在别的平台
**静默不跑**；比较用 **OS 族**而非标签计数（`runner.os == X` 筛的就是族）；⑤ **形状的支持集 MUST
按实测的平台事实写**——`sudo` 的支持集**不是** `['Linux']`（macOS runner 也有 `sudo`，只有
Windows 没有），写成 `['Linux']` 会在 macOS 矩阵下**误报一个正确的配置**；⑥ **`run` 要扫整段，
不是首行**——`sudo apt-get update && sudo apt-get install …` 里 `apt-get` 在 `&&` 之后，只用首行
会让 `debian-package-manager` 这一格**零实例**，与「每个形状 MUST 有实例」冲突；⑦ **非空洞要求
刻意不同（规则表 vs 字典）**——`CONDITION_SHAPES`／`PLATFORM_COMMAND_SHAPES` 是**规则表**
（每格 MUST ≥ 1 实例，同 W99 的 `TIER_PAYMENTS`），`RUNNER_OS_VALUES`／`RUNNER_LABEL_PREFIXES`
是**平台事实的字典**（只要求矩阵里出现过的标签可分类，要求「所有平台都出现在矩阵里」等于把字典
当断言）；⑧ **一处刻意的非对称**——一个在册步在 workflow 里**不存在**时本判据**沉默**，
那是 W95「a registered step that vanished」的判定域。

**两条判据都读 `condition` 但不重复判决**：W95 判「该步的**角色**与条件是否一致」（`diagnostic`
必须带失败条件），W100 判「**条件形状表**是否还有实例」；`if: failure()` → `always()` 会让两条
**同时**红，**消息与判定域都不同**（冗余，不是重复定义）。

解析器**只留一份定义却产出两个视图**：`workflowJobsOf` 给 job 视图（`matrix` 是 **job 的属性**，
不是 step 的——W95 的扁平视图把它丢掉了）；`workflowStepsOf` 在原有三字段上**增补**
`matrix`／`continueOnError`，W95 的 37 个用例**全部不变**。

`tests/contracts/step-config.test.mjs` **15 个用例**、`test-reachability` 站点 **12 → 13**、
**Node 套件 10 → 11**（portable 面 **214 → 229**）、`xtask/src/lib.rs` **+1 行登记 + 一段注释**
（Rust 零行为改动，**被审对象 `foundation.yml` 与 `WORKFLOW_STEPS` 的 21 条一个字符未动**）。
注入 **V1–V11** 全部真改文件、真跑 9 套件、逐字节还原（`foundation.yml` `2f46d5f5…8c1c`
前后一致），边界（W84／W95／W96／W97／W98／W99／W100）= V1 `0/0/0/0/0/0/3`、V2 `0/0/0/0/0/0/3`、
V3 `0/0/0/0/0/0/4`、V4 `0/0/0/0/0/0/2`、V5 `0/0/0/0/0/0/2`、V6 `0/0/0/0/1†/0/2`、
V7 `0/0/0/0/0/0/2`、V8 `0/2/0/0/0/0/3`、V9 `0/0/0/0/0/0/2`、V10 `0/0/0/0/0/0/3`、
V11 `0/4/0/1†/0/0/1†`（† = **夹具前提被扰动**的假红，非判据判定）。

**本轮新增两条探针陷阱、并印证一条旧教训**：① **Python `Path.write_text` 在 Windows 上把 `\n`
翻成 `\r\n`** ⇒ 破坏夹具里所有含 `\n` 的字面锚点，探针**自己制造红**（首轮实测得
`[0,0,0,4,0,1,0,0]`，排查后确认无信号）⇒ **修法 = `read_bytes`／`write_bytes` 逐字节写入，
并断言「`\r` 计数与原文一致」**；② **某些步名被测试当作夹具锚点**（`Kernel registration and
binding` 在 `workflow.test.mjs` 里出现 **11** 次）⇒ 对它们注入会**扰动夹具**、产生假红
⇒ **修法 = 探针选零命中的步**（先统计命中次数再选）；③ **跨套件红 MUST 先确认前提**——
V6／V11 除本判据外还打红了别的套件，根因是**那些用例的 `replace` 锚点已被探针改掉**
（注入变成 no-op ⇒ 断言前提没了）——**不是判据在响**（V11 上 `stepConfigErrors` 返回 `[]`，
本判据确实沉默）⇒ **`patch()` 必须自带「改不到就断言失败」**。

**如实披露**：`if:` 的求值是**形状级**的（只认 `runner.os == '<os>'` 与 `failure()`，其余
**具名拒绝**，拒否方向落在**假阳性**）；R5 是形状级且会扫到 `run` 里的注释文本；
**「步身份 ↔ 判定目标」的绑定**在本轮**仍未管**（P-B1 零信号）——**W101 已就地订正为「已兑付」**
（`target` 与 `run` 解析的双向对账；见下文 W101 段）。
见 [W100 报告](docs/reports/2026-10-14-w100-step-configuration.md)、
[ADR 0078](docs/adr/0078-step-configuration.md)、
`openspec/changes/archive/2026-10-14-w100-step-configuration/`。

### W101 在册判定步的**目标绑定** MUST 由 `run` 解析兑付

W101 换**第十层对象**，而这一次换的不是又一个字段或集合，而是**一条声明与它实际跑的对象的配对**：
前九轮（`repositoryFiles` → `readTrackedText` → W94 两张事实表 → W95 `gateSources` → W96
`gateRegions` → W97 包集合 → W98 脚本执行面 → W99 站点层级声明 → W100 执行配置面）补的全是**集合层**，
而**集合层对「配对」是瞎的**。W95 的在册条目只声明 `{host, identity, kind}`，其中
**`identity` 是名字，不是目标**——`- name: API Provider transport and retry…` 与
`run: cargo xtask test --suite provider` 之间**没有任何断言**。

**实测（改真文件、真跑 9 个 portable 套件、逐字节还原、`sha256` 逐字一致，基线 229/0）**：
`--suite hosts` 与 `--suite session` **互换** ⇒ 全 `0`；`Scheduling…` 改跑 `--suite provider`
（scheduler 在 CI 消失、provider 跑两遍）⇒ 全 `0`；`Knowledge…` 改跑 `--suite storage` ⇒ 全 `0`；
**把 provider 判定步整条改跑 `cargo xtask check --secrets`（目标性质整个换掉）⇒ 全 `0`**。
两个「看似有信号」的探针（改 profile、`schema --check` → `schema`）红的其实是 W97 的一条**越界断言**
（`commands.includes('check --profile full') && …('schema --check')`）——它钉**具体串**，
与 W97 **自己的边界声明**（「不验签名的取值」）矛盾，既**假阳性**（合法改 profile 即红）
又**假阴性**（同形状替换看不见）。

八条纪律：① **集合层对「配对」是瞎的**——两轮都能说出「有 21 个步、8 个 suite 各跑一次」，
却说不出一句「这个步跑的是哪个目标」；② **`target` 住条目自己身上，MUST NOT 另开一张表**——
一个 step 是**一个对象**，另开表就是同一件事的第二处定义，两张表会各自漏掉对方的新增；
③ **钉取值不等于钉形状**——W96 管「入口的**形状**」（`check --profile <P>` 是一条签名，不含值），
W101 管「这一步调用的是哪一个**入口实例**」；对象不同，两个答案都必要；④ **判据的输入 MUST 是
「这一步自己的 `run`」，且 MUST 先按 shell 段界（`&&`／`||`／`;`／`|`／换行）拆段**——
`cargo xtask test --suite kernel && cargo xtask check --secrets` 里两条命令**都会被执行**，
「一行取一条」只会看到第一条 ⇒ 第二条判定**静默逃出**目标绑定（**假阴性方向，危险**）；
⑤ **`infra`／`diagnostic` MUST 双向**（不声明目标 **且** 解析出**零个**目标）——单向是装饰，
真正的后门是「把一条判定塞进环境准备」；W95 也有一条同目的检查，但它**只看首行**、只认 5 个正则，
本条是**整段解析**（实测两条**都**红，但断言不同）；⑥ **对账 MUST 是双向集合相等**——
解析出而没声明（目标被悄悄换掉）与声明了而没解析出（凭据是空话）都具名，
且判定单元是 **`(shape, value)`** 而非裸值（W91：同名 ≠ 同一个）；⑦ **判分由在册条目驱动、
不重复报 W95 的判决**——未知 `kind`、已消失的步、重复身份一律跳过 ⇒ 本判据对**改步名是沉默的**
（实测 C1：`stepTargetErrors` 返回 `[]`，W95 报 2 条）；⑧ **越界断言就地收回形状级**——
W97 那条具体串断言改为 `/^check --profile \S+$/`、`/^schema(?: --check)?$/`，
取值绑定只留 W101 一处（「同一件事 MUST NOT 两处定义」+「按对象性质分」）。

**同族用例的前提构造 MUST 遍历**站点表**自己的键**：本轮新增一个站点，一次让**三条既有用例**
变红（`registry`／`tier-payment`／`step-config` 的可达性正向前提）——根因是 W99／W100 把前提写成
`new Set([...tracked, '<自己那一轮的文件>'])`，**前提跟着轮次走而不是跟着站点表走**。
修法是导出 `declaredMarkerPaths()`（遍历站点表、取无通配的 marker）并让前提改用它。

解析器**只留一份**：`targetsOf` 的 `xtask` 那一支**调用** W97 的 `ciXtaskCommands`（剥壳规则抽成
共享的 `stripShellSyntax`，行为一字不变），MUST NOT 写第二份剥壳逻辑。

`tests/contracts/step-target.test.mjs` **16 个用例**、`test-reachability` 站点 **13 → 14**、
**Node 套件 11 → 12**（portable 面 **229 → 245**）、`xtask/src/lib.rs` **+1 行登记 + 一段注释**
（Rust 零行为改动、**被审对象 `foundation.yml` 一个字符未动**、`WORKFLOW_STEPS` 只**增补** `target`）。
注入 **V1–V11** 全部真改文件、真跑 10 套件、逐字节还原（`foundation.yml` `2f46d5f5…8c1c` 前后一致），
边界（W84／W95／W96／W97／W98／W99／W100／W101）= V1 `0/0/0/0/0/0/0/0/0/2`、
V2 `0/0/0/0/0/0/0/0/0/2`、V3 `0/0/0/0/0/0/0/0/0/2`、V4 `0/0/0/0/0/0/0/0/0/2`、
V5 `0/0/0/0/0/0/0/0/0/2`、V6 `0/0/0/2/0/0/0/0/2/2`、V7 `0/0/0/2/0/0/5/0/1/2`、
V8–V10 同为 `0/0/0/0/0/0/0/0/0/2`、V11 `0/0/0/0/0/1/0/0/0/3`、
C1（改步名，**对照组**）`0/0/0/2/0/0/0/0/1/2`（**本判据 `[]` 沉默**，其余全是「前提：W95 一致」被扰动）。

**探针方法论（本轮踩到并写下来）**：① **第一轮探针的红大多不是判据在响，是夹具锚点被扰动**——
选探针样本前 MUST 先统计该文本被多少夹具锚定（`Kernel registration and binding` 那一整段
`name+run` 就是 `workflow.test.mjs` 的锚点；一改它，`.replace()` 匹配不到、注入变 no-op，
用例因**前提没了**而红）；② **别把「身份被顺带改掉」当成「目标被改」**——无 `name` 的步
（`- run: pnpm spec:validate`）其身份**就是** `run` 首行 ⇒ 改它会**同时**触发 W95 的两条，
实测 V7 的 W101 判据返回 `[]`（沉默），红的是 W95 与 W98；③ **跨套件红 MUST 逐条读
`not ok` 与它的 assertion message**，才知道红的是「判据」「夹具前提」还是「上游级联」。

**如实披露**：不剥 `run` 里的 shell 注释（W97 的既有边界，假阳性方向安全）；不实现 shell 语义
（变量展开、条件分支 ⇒ `if`/`for` 包裹的命令被当成**无条件目标**）；不判目标在矩阵各腿上的等价性
（W100 的对象）；每个形状的**实例数不钉**（W92 的教训：一次合法的目标收敛不该让判据变红）。
见 [W101 报告](docs/reports/2026-10-14-w101-step-target-binding.md)、
[ADR 0079](docs/adr/0079-step-target-binding.md)、
`openspec/changes/archive/2026-10-14-w101-step-target-binding/`。

### W102 workflow 的**触发面**（`on:`）MUST 由解析兑付

W102 换**第十一层对象**，而这一次换的是一个**前提**：前十一轮补的全是「workflow **里面**」的层
（步集 W95 → 命令面 W96 → 包集合 W97 → npm 脚本执行面 W98 → 站点层级 W99 → 执行配置 W100 →
目标绑定 W101），而「这个 workflow **什么时候会跑**」——`on:` ——**不在任何判定域内**。
`workflowJobsOf(text)` **只读 `doc?.jobs`，从不读 `doc?.on`**；全仓 `grep`
「`.on` / `trigger` / `pull_request` / `branches:`」在 `scripts/contracts/*.mjs` 与 `tests/**` 下
**零命中** ⇒ 它是 W95–W101 **全部判定的前提**，而这个前提**没有任何判据**。

**实测（改真文件、真跑 10 个 portable 套件、逐字节还原、`sha256` 逐字一致，基线 245/0）**：
删掉 `pull_request`（PR 不再被门控，P-C）⇒ 全 `0`；把整个 `on:` 缩成仅 `workflow_dispatch`
（push/PR 都不再触发，P-D）⇒ 全 `0`；把 `push` 限定到一个**不存在**的分支（P-E）⇒ 全 `0`。
这三条不是「漏了一个字段」——**它们让整套门控静默地不再运行**，而所有步层判据照样全绿，
因为「有人会跑这个 workflow」从来没被兑付过。仓库里对此**只有一句散文**
（「workflow 有 `on: push`，每次 push 都已自动触发」，本文件 `:651`、`docs/development-plan.md:541`）。
**散文不是判据。**

九条纪律：① **「前提」是判据的补集**——七轮判据能说出「有 21 个步、8 个 suite 各跑一次、
每步跑哪个目标」，却说不出一句「这个 workflow 会不会被跑起来」；② **在册表 `WORKFLOW_TRIGGERS`
单独一份，MUST NOT 塞进 `WORKFLOW_STEPS`**——触发器**不是任何一个 step 的属性**，而是整个 job 集的
**前置条件**（W101 决定 1 的判据是「它是不是**这个对象**的属性」，此处答案为否 ⇒ 另立一表；
「同一件事 MUST NOT 两处定义」在这里的正确形式是**只有本表**声明有哪些触发器）；
③ **判定单元 MUST 是 `(event, filter-key)`，MUST NOT 是裸键名**——`branches` 在 `push` 与
`pull_request` 上是**两个可分别篡改的过滤器**，按裸名比较看不见互换（W91：同名 ≠ 同一个）；
④ **至少一条自动触发 MUST 在在册侧与解析侧各自断言**——双向闭世界让两侧**大多数情况**同真同假，
但**不是所有情况**：补上 `workflow_dispatch` 同时删掉 `push` ⇒ 事件集对账成立而解析侧仍须红；
⑤ **事件名是闭世界**（`AUTOMATIC_EVENTS` ∪ `MANUAL_EVENTS`、**不相交**），**`kind` 由事件表推导、
MUST NOT 成为在册字段**（能由结构蕴含的 MUST NOT 再具名一次；手写一定会漂移，而漂移方向恰好是
「把手动触发当成自动触发」⇒ **假阴性，最危险**）；⚠️ **「除手动表以外都算自动」是错的**——
那样一个**拼错的事件名**（`pus`）会被推导成自动触发 ⇒「至少一个自动触发」在一个**永不触发**的
workflow 上通过；⑥ **过滤器键闭集**＋取值 MUST 是非空字符串数组；⑦ **解析侧只认形状、判定侧才认域**
（域检查塞进解析器会让一个笔误 `branch:` 少个 `s` 把**整个触发面**判成「读不懂」，丢失粒度）；
⑧ **缺输入／空注册表／`on:` 缺失或空／事件重名／不可解析 ⇒ 具名失败**；
⑨ **条目字段集闭世界**——「`kind` 不存」这条决定的**兑现物**是「键集 MUST 恰好等于
`TRIGGER_FIELDS = ['host','event','filters']`」：探针 **P-B5** 实测，给一个条目**手写**
`kind: 'automatic'` 时判据**一个字都不说**（它不读 `kind`）——那不是少报，而是**一条没人会读的
声明静静地挂在表上**，正是母题「声明 MUST 付代价」的反面（错拼的 `filter` 同理）。

⚠️ **层级差是立论核心**：W100 已经在「**步**」这一层判了「会不会跑」，但它的输入是
`job.strategy.matrix` × `step.if` ⇒ **它读不到 `on:`**。一个 workflow 可以**每个步都「会跑」**，
而整个 workflow **永不触发**——两层对象不同，**不可互推**。

**形状域（实测，`yaml` v2.9.1）**：`on:` 三写法（`on: push`／`on: [push, pull_request]`／映射）
都归一化；⚠️ **`on` 是字符串键、不是布尔 `true`**（`yaml` v2 默认 YAML 1.2 core schema，
实测 `Object.keys(doc)` 为 `['on']` ⇒ `doc.on` 可靠，**不需要**防御 `doc.true`）；事件配置 MUST 是
`null` 或「值为**非空字符串数组**的映射」，`schedule: [{cron}]`（值=数组）与
`workflow_dispatch: {inputs: …}`（值=对象）⇒ 解析器**抛具名错误**（域外 ⇒ 具名，MUST NOT 静默降级）。

`tests/contracts/workflow-triggers.test.mjs` **15 个用例**、`test-reachability` 站点 **14 → 15**、
**Node 套件 12 → 13**（portable 面 **245 → 260**）、`xtask/src/lib.rs` **+11 行**（4 行登记 + 7 行注释；
登记行超出 rustfmt 行宽 ⇒ 由 `cargo fmt` 折行，见 W102 报告 §六.1）
（Rust 零行为改动、**被审对象 `foundation.yml` 一个字符未动**）。
注入 **P-A1–P-A8 + P-B1–P-B5（13 条）+ 1 条等价改写对照** 全部真改文件、真跑 **11** 套件、
逐字节还原（`foundation.yml` `2f46d5f5…8c1c`、`workflow-triggers.mjs` `ddb0c886…e67b` 前后一致、
两文件 `CR=0` 不变）；**判据直调**（不依赖测试管线）边界矩阵（W84 / W95 / W96 / W97 / W98 / W99 /
W100 / W101 / W102）：**每一条注入都是 `W95=0 / W100=0 / W101=0 / W102≥1`**
（P-A2 4 条、P-A4／P-B3／P-B4 2 条、其余 1–3 条）——**只有 W102 一个判据在响**，
这是「本轮补的确实是一层新对象」的直接证据；**P-A8（`on:` 等价改写为列表写法）
四个判据全 `0`**（A 组改 `.github/workflows/foundation.yml`、B 组改 `scripts/contracts/workflow-triggers.mjs`）。

**探针方法论（W101 的同型教训在本轮再次踩到并写下来）**：⚠️ **夹具锚点脆 ⇒ 假红**。P-A8
第一次跑出 2 条红，读 `not ok` 后确认**不是判据在响**——两条正交性用例的锚点是字面串
`'  pull_request:\n'`，`on:` 一换成列表写法就**匹配不到** ⇒ 用例因**前提没了**而红。
**处置 = 修前提**（写一个覆盖两种写法的 `withoutPullRequest()`，第三种写法出现时**具名失败**）、
**MUST NOT 改断言**（铁律一：断言对、前提错 ⇒ 修前提）。修后 P-A8 `0`。
另：**bash 内联 `\n` 不可靠**（本轮一次 `python -c "…'on:\n  push:\n'…"` 的锚点计数判错、
一次 `node -e` 把 `\n` 当字面）⇒ **探针一律写进文件再跑**。

**就地订正（活文档）**：`scripts/contracts/test-reachability.mjs` 的台账注释原文写
「实测 **9** 个测试文件全部落在某个站点的覆盖集里」——随站点增补**早已不实**，且正是
「**易漂移的计数 MUST NOT 进活文档**」的禁令 ⇒ 改为**不钉数字**的表述并写明理由。
⚠️ **同形不同义**：[ADR 0062](docs/adr/0062-test-reachability-closure.md)／本文件与
`docs/development-plan.md` 的 **W84 段**里的「**七个站点**」是**关于 W84 那一轮**的记录，**不订正**。

**如实披露**：当前仓库的 `on:` **没有任何过滤器** ⇒ 过滤器对账的单元集合在真实仓库上是**空集**，
其非空洞由**合成世界**（覆盖一个带过滤器的单元 + 一个裸事件）与**注入用例**（P-A3／P-A4／P-B2）
共同证明；**不判事件配置**（`workflow_dispatch.inputs`／`schedule.cron`，出现即具名失败）；
**不判分支/路径是否真的存在**（那要读 refs，且宿主瞬态 MUST NOT 改变结论）；**不判
`permissions:`／`concurrency:`／`env:`**（workflow 头部是各自独立的一层）。
**遗留**：P-A／P-B／P-F 实测零信号的**跨步数据传递**缝（门控日志的写方
`tee "$RUNNER_TEMP/nai-gate.log"` 与读方 `ci-diagnose.mjs "$RUNNER_TEMP/nai-gate.log"`
是**两处独立字符串、无对账**）本轮**不裁**——「一条待裁定项只在本轮裁一个」。
见 [W102 报告](docs/reports/2026-10-14-w102-workflow-trigger-surface.md)、
[ADR 0080](docs/adr/0080-workflow-trigger-surface.md)、
`openspec/changes/archive/2026-10-14-w102-workflow-trigger-surface/`。

### W103 H15 的 runtime 接线：判定面之后的**那一步**必须有执行体

W66 把 §47.1 的观测判定落成 `crates/nai-kernel/src/evaluate/harness.rs::evaluate_observation`，
并在模块文档里写死「Appending it to the actual hash chain in `nai-storage` is runtime wiring and is
**not** claimed here」。这句话当时**诚实**，同时**不可核实**：
`grep -rn 'evaluate_observation\|ObservationRecord\|ObservationPolicy' crates/ apps/` 只命中
`crates/nai-kernel/tests/*.rs` ⇒ 生产代码**零调用方**。于是两条命题**无法被判**：
① 一条被判**不可落日志**的记录会不会**照样**进链；② 进链的载荷是不是判定输出的**脱敏材料**。
⇒ 母题「**有判定面 ≠ 有执行体**」的又一次现身（W54 桌面壳、W68 讲品循环同型）；
「有判定面」是「有执行体」的必要条件，**不是**充分条件。

**落点是推出来的**：接线必须同时看见 `nai-kernel` 的判定与 `nai-storage` 的只追加链，
而 `crates/nai-runtime` 同时依赖两者；放 kernel 会引入 kernel→storage 越界依赖、放 storage
会让存储层认识 harness 领域类型 ⇒ `cargo xtask check-deps` 会红。**零新增依赖**。

**决策一：判定与追加合并为一个入口**。`Composition::record_observation(record, policy)`——
不暴露「先 `evaluate` 再 `append`」两个出口：那会把「不许追加未判定的记录」**推给每个调用方**，
一个人忘了就破了，而**测试看不出来**（不变量 MUST 由**结构**保证，不是由纪律保证；
W92/W93「把门槛交给被审者自己填」在**接口层**的又一实例）。

**决策二：runtime 是传输者，不是加工者**。可落日志 ⇒ 追加载荷**恰为**判定输出的脱敏材料
（层标签 + `verdict.redacted` + `verdict.redacted_viewer`），MUST NOT 取 `record.fields`、
MUST NOT 添加任何判定**没审过**的字段 ⇒ **不带** `record.subject`。⚠️ 判定只扫 `fields`，
`subject` **永不被审** ⇒ 把它塞进载荷就是一条「凭据从 `subject` 溜进日志」的旁路，而判定
**永远发现不了**它。把未审的部分补进结果不是「补全」，是把判定**扩不到的地方**变成**默认可信**。

**决策三：不可落日志 ⇒ 具名拒绝且一个字节都不落盘**。`ObservationOutcome` 是枚举 ⇒
「已落日志」与「已拒绝」**类型上**可分，不可能被读成同一个 `Ok`（铁律二）。

**决策四：本轮避开先做 O05（升级数据源）**。O05 体量更小、还能解锁 O02，但接数据源**必然改
OpenAPI 契约** ⇒ 必然触发 `scripts/contracts/generate.mjs` 再生三语言产物，而校验入口
`cargo xtask schema --check` 在宿主机**跑不了**（`spawnSync(process.execPath)` 报 `EBUSY`，
managed/system node 均复现）——**CI-only**。⇒ **「本机验不了的改动」不满足「可自主推进」的定义**
（「小」不是判据，「本机可验证」才是）。

**非空洞注入四组**（真改 `crates/nai-runtime/src/observation.rs`、真跑、逐字节还原、`sha256` 一致）：
I1 载荷改取 `record.fields` ⇒ **1 条红**；I2 拒绝分支不再早退 ⇒ **4 条红**；
I3 只接一类 layer ⇒ **5 条红**；I4 载荷被 `record.subject` 富化 ⇒ **1 条红**。
⚠️ **I3 的样本定位 MUST NOT 按「态」定位**（W92 的教训）：五类的断言**遍历 `OBSERVATION_LAYERS`
自己**，MUST NOT 遍历手写的层名清单。⚠️ **I4 是设计选择带出来的守卫**：样本不是「凭据藏在字段里」
（那会被判定拦住、与 runtime 无关），而是**凭据藏在 `subject` 里而字段干净**——判定放行、
runtime 若富化就泄漏，正是能**区分两种实现**的那一例。

另：`composition.rs` 的 `next_trace` 只**放宽可见性**到 `pub(crate)`，**不**复制一份格式
（ADR 0064「一处定义、两处使用」）；`DecisionKind::Observation` 用既有的 telemetry 级口径，未改。

**如实披露**：**H15 保持 `pending`**。本轮可测的是「判定**之后**那一步真的发生且形态正确」；
**五类观测的真实产生者**（装配裁剪、意图判定、记忆读写、工具调用、缓存命中）**仍未接线**，
属 **H01/H02/H03/H04/B07** ⇒ 报告 MUST NOT 出现「五类观测已进决策日志」的措辞。
降级侧（`evaluate_degradation` → `CapabilityView` 与界面）**不做**，属 H17 的另一轮。
见 [W103 报告](docs/reports/2026-10-14-w103-observation-log-wiring.md)、
[ADR 0081](docs/adr/0081-observation-log-wiring.md)、
`openspec/changes/archive/2026-10-14-w103-observation-log-wiring/`。

### W104 H02 的 runtime 接线：判定面之后的**那一步**必须有执行体

**缝型**：`crates/nai-kernel/src/evaluate/assembly.rs::rebuild_assembly`（W69 交付的 H02 判定）
在生产代码里**零调用方**（`grep` 只命中 `crates/nai-kernel/tests/*.rs`），而 W69 的模块文档与
`docs/kernel.md` 自己写着「把装配记录追加进 `nai-storage` 的哈希链……是 runtime 与证据生产侧的事」。
⇒「一次被判可重建的装配会不会照样进链」「进链的载荷能否**只靠链**重排同一段片段序列」两条**无法被判**——
与 W103（H15）**完全同型**，是本仓「**判定面存在、执行体不存在**」的**第七次**现身。

**实现**：新增 `crates/nai-runtime/src/assembly.rs`——
`record_assembly(record, fragments)` 是「判定 + 追加」的**唯一**入口（不暴露两个出口 ⇒ 不变量由结构保证）；
可重建 ⇒ 追加 `DecisionKind::Assembly`（telemetry 级），载荷**恰为**判定放行的那一份：按**记录进入顺序**的
片段 `id`／来源标识／**字面正文** + 模型与提示词版本（`content` MUST NOT 为空、MUST NOT 退化成内存指针）；
不可重建 ⇒ `AssemblyRefusal` 具名拒绝、**一个字节都不落盘**。
`rebuild_assembly_from_log(record, limit)` **只读**真实链：按**日志自身顺序**解出片段后交给**同一个**
`rebuild_assembly`（判定是**调用**、不是复写），映射为 owned 的 `AssemblyRebuild`。
`tests/assembly.rs` **12 项**，含**遍历 `RebuildRejection::ALL`** 的非空洞守卫（样本构造函数对拒绝枚举
**穷尽 match** ⇒ kernel 加变体而 runtime 未覆盖即**编译失败**）。`composition.rs` 把 `short()` 放宽为
`pub(crate)`（复用既有逐次调用预算，ADR 0027「一处定义」）。

**本轮最重要的实测发现——`assembly` 种类是两处定义**：`migrations/sqlite/0005-decision-log.sql` 把种类写成
`CHECK(kind IN(…12 个…))`，与 Rust 的 `DecisionKind` 枚举并存。只改枚举不改 CHECK ⇒ 插入被 **schema 拒绝**，
且**不响亮**：telemetry 的「写失败 ⇒ 进有界缓冲」让条目静静躺在内存里（`ack.persisted == false`）。
**W104 首跑即复现**（3 个用例红在 `persisted` 与「日志里没有 assembly 行」）。⇒ 新增迁移
`0007-decision-log-assembly-kind.sql`：SQLite 不能就地改 CHECK，故**逐列重建**该表、`INSERT … SELECT`
**逐字复制**既有行（含四个哈希列与 `seq`）、`DROP` 旧表、`ALTER … RENAME` 回原名、重建索引与两个只追加触发器；
链不受影响（`entry_hash` 的 preimage 不含表名）。`migration::VERSION` 6 → 7、`SQL` 数组 +1 段。
**升级路径 MUST 被实测**（否则 `INSERT … SELECT` 只在空表上跑过）：新增
`migration::tests::version_six_upgrade_preserves_the_decision_chain_and_accepts_the_assembly_kind`——
先在 v6 库上**证明旧 CHECK 拒绝 `assembly`**，再升级，断言既有行逐字保留 + 新种类被接受 + `DELETE` 仍被触发器拒绝。

**连带的版本号面**：`backup.schema.json` 的 `db_schema_version` 与 `migration::VERSION` 成对（ADR 0045）⇒
四处同步（schema const、其 TS 生成物、`tests/contracts/schema.test.mjs` 样本并把负例由 5 改 6、清单断言）。
⚠️ **如实披露**：`db_schema_version` 属 `TS_GENERATED_SOURCES`，再生入口 `cargo xtask schema --check`
（→ `scripts/contracts/generate.mjs`，内部 `spawnSync(process.execPath)`）在宿主机 `EBUSY`、**本机跑不了**；
生成物手工同步**一个 token**，依据是**已提交的一对**（schema 的 `const: 6` 与 `backup.ts` 的
`db_schema_version: 6;` 逐字对应）⇒ 该不变量当前成立，手工同步保持它成立，CI 的 `schema --check` 复核。
**把不可再生面压缩到一个 token 并明说**，不是把本机验不了的改动当成已达标。
另：`docs/storage.md` 的迁移段与 `db_schema_version` 句**此前已陈旧**（W51 升到 6 时未同步，仍写 5），本轮一并就地订正为 7。

**非空洞注入五组**（真改文件、真跑、逐字节还原、`sha256` 一致、复跑基线 4/4 与 12/12 全绿）：
I1 载荷 `content` 落 `null` ⇒ **5 红**；I2 拒绝分支不再早退 ⇒ **7 红**；I3 片段按 id 排序落盘 ⇒ **1 红**；
I4 载荷漏 `model_version` ⇒ **1 红**；I5 迁移 CHECK 去掉 `assembly` ⇒ storage **1 红** + runtime **4 红**。
⚠️ **两组注入逼出了两处测试缺口并已补齐**：I3——原往返用例用**升序** id，排序实现与忠实实现不可分
⇒ 新增 `the_log_preserves_the_entry_order_rather_than_re_sorting_it`（非升序 id）；I4——原先**无断言**
钉住载荷携带版本号 ⇒ 补两条载荷断言。**没有这两处修补，I3/I4 都会是零信号。**

⚠️ **归档当场逼出一个真实缺陷——校验面错位**：`openspec validate <change>` **只**校验变更自身的 delta 结构，
**不施加** spec 级的「需求正文 **> 500 字符**」规则 ⇒ `agent-harness` 那条 MODIFIED 需求被追加句从 **424**
推到 **652**，**归档前照样报 `is valid`**，归档后第一条 `validate --specs` 立刻 **1 红**。
根因：`pnpm spec:validate`（= `openspec validate --all --strict`）与 `openspec validate --archived` 在本仓属
**`not-in-profile`**（`scripts/contracts/workflow-steps.mjs` 明写「本机能跑、没接进 `check` profile」）——
**门控不跑、CI 跑**（`.github/workflows/foundation.yml`）⇒ **不补跑就会红在 CI**（「本机绿 ≠ CI 绿」差的是整个校验面）。
处置＝把追加句压到房风格最短形式（保留边界语义、去掉需求名逐字引用），**活 spec 与归档 delta 两处 1:1**
（归档目录本轮**未提交** ⇒ 按「同轮未提交 ⇒ 就地订正」就地改，不追勘误段）；
复跑四条 validate 全绿（`--specs` **43/0**、`--changes` 无、`--all` **43/0**、`--archived` **102/0**），
并**复跑第三次门控 EXIT = 0**（计数现测不变 **93/883**）。

**如实披露**：**H02 保持 `pending`**。本轮可测的是「判定**之后**那一步真的发生、且只靠链可重建」；
**装配器的真实产生者**（把九段片段喂给模型、§41.2 分段预算初值标定、§47.2 六类回归属 **H16**）**仍未接线**
⇒ 报告 MUST NOT 出现「装配器已接入」「已不依赖当时内存」这类运行期断言。H17（`evaluate_degradation` →
`CapabilityView` 与界面）**不做**，属另一轮。
见 [W104 报告](docs/reports/2026-10-14-w104-assembly-log-wiring.md)、
[ADR 0082](docs/adr/0082-assembly-log-wiring.md)、
`openspec/changes/archive/2026-10-14-w104-assembly-log-wiring/`。
