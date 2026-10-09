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
| D17 | （V3.5 遗留：升级器） | D03–D05/D14 | 未开始 | 后续按 V3.6 W15/R1 推进 |
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
| W03 | 存储与决策日志 | R04、R07 | 哈希链及原子有序冲刷/critical优先已实测；R07仍待桩回放与播放ACK |
| W04 | 宿主与 IPC | S11、A06 | 已实现/实测（D05）；wasm/remote 只留契约 |
| W05 | output-keeper | R01、R03、S12 | 看门狗演练进程已实现/实测：强杀源约10ms撤权、停止续租578ms到期、partial卡读250ms期限、keeper重启需更高epoch floor；帧槽位所有权与tmpfs-file传输见W06行；引擎broker授权keeper只读FD单帧消费与待机见W10行；正式窗口/音轨/媒体协调和P0设备独立通道未开始 | [报告](reports/2026-10-09-engine-broker-frame-delivery.md) |
| W06 | 2D render-worker | A01 | 2D软件像素生产核心与帧槽位所有权/tmpfs-file传输已实现/实测；引擎经broker声明spawn pid并继承写FD完成真实单帧见W10行；独立worker/keeper装配、protected handle broker与Windows帧率未完成 | [报告](reports/2026-10-09-w05-w06-frame-slots.md) |
| W07 | Provider 核心与适配器 | A02 前置 | 已实现/实测（D08）；真实付费生成 Blocked |
| W08 | 默认智能体、守门与手工平台 | S01–S04 | 已实现/实测（D10/D13） |
| W09 | 最小桌面控制台 | S02、R01、O02 | loopback 控制与审批 API 子切片已实现/实测；审批 GUI/真实候选来源/完整预检与生产 root 接线未开始 |
| W10 | 集成与故障注入 | R01–R03、R07、A01 | 引擎真实装配：控制面+监督+broker/keeper单帧投递、强杀撤权、预热期halt不提交已实测；全链路播放ACK与正式IPC未做 | [报告](reports/2026-10-09-engine-broker-frame-delivery.md) |

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
