# 引擎装配（apps/engine）

`nai-engine` 是本仓库第一个可启动的装配可执行程序：它把存储 owner、共享会话、loopback 控制面与
受监督媒体子进程装配成一个进程（ADR 0020）。它不是生产形态：无 protected IPC、无设备/窗口绑定、
无真实 TTS/真实设备 PlaybackAck（只有演练软件替身）、无 Windows 认证。

## 参数

```
nai-engine --data-root <dir> --workspace <id> --actor <id> --profile <name>
           --token-file <path> [--control-port <u16>] [--halt-file <path>]
           [--keeper <path> --render-worker <path>]
           [--media-drill --pcm <path> --avatar <manifest> --avatar-config <config>
            --asset <id=path>... --license <id>... [--slot-bytes <n>] [--time-base-den <n>]]
           [--event-file <script.json> --rehearsal-provider
            [--inject <none|knowledge|guard|generate|speech|storage>]]
nai-engine ... --provision <runtime-profile.json>      # 可信 owner 一次性配置
```

- `--provision`：建工作区、授予运行时权限、写入并校验运行时 profile，随后退出 0（不监听）。
- token 文件必须 `0600` 且内容为 32–256 个 url-safe 字符；权限全集的临时令牌（TTL 1h）。
- `--keeper` 与 `--render-worker` 必须成对给出（`Supervisor::new` 会校验两个二进制都存在）。
- `--halt-file`：显式优雅停机请求。`tokio::signal` 需要 `signal-hook-registry`，该 crate 不在离线
  缓存内且 crate 内禁 `unsafe`，因此本轮用文件触发；被强杀进程不计为优雅停机。
- `--event-file` 需要 `--rehearsal-provider` 同时给出；`--inject` 只在给出 `--event-file` 时可用。
  `--event-file` 与 `--media-drill` 不互斥，但手工事件链路只到「决策日志」为止，不驱动媒体。
  `--inject speech` 需要脚本给出 `render` 段；否则拒绝启动，而不是静默忽略注入。

## 启动与运行

1. `Composition::prepare`（唯一存储 owner、配置校验、Session/MediaGate/Registry/ApprovalBackend）。
2. `ControlApi::with_shared_session`：控制面与装配根共用同一会话，撤权 epoch 立即对两侧可见。
3. 绑定 `127.0.0.1:port`（`port=0` 时取临时端口），stdout 打印一行：
   `nai-engine control: http://127.0.0.1:<port>/api/v1`。
4. 监督线程（给出 keeper 时）：spawn keeper（floor = 现场 epoch）→ 以 `floor+1` 签发 Rehearsal 租约
   并 arm → 每 tick **先续租、再 observe**。控制面撤权后停止续租（telemetry
   `renewal_halted_after_revoke`），**不自动重签**；续租失败不静默退出，由 `observe()` 走
   静音→撤权→critical 日志→待机。

## 媒体演练（--media-drill）

Linux `/dev/shm` 私有池 → root 以 `FrameBroker` 声明真实 spawn pid → keeper 经继承的**只读**池 FD 重建
读取端并触发 UDS → broker 按 `/proc` 证据（pid 存活、属 pool owner、持有 pool 文件）授权 → render-worker
（`--ready` 预热，冷启动 ~290ms 超过 250ms 槽位预算）继承写 FD、reserve 并原子写 ticket → 读
`write_started`/`committed` → root accept commit（槽 pin Reading）→ `begin_consume` 后由 keeper 读回
owned 像素 → 匹配 seq/epoch/`buffer_ref`/SHA256 的 keeper 诊断 → `release` → 写 finish/回收 worker。
结果记为 telemetry（`media_drill_committed`，含 buffer_ref/SHA256/字节数与 keeper 观察）。

预授权/等待期间每 tick 续租、`observe()` 并复检共享 epoch 与 halt；worker 事件经 reader 线程入有界队列，
owner 非阻塞轮询，卡读不阻塞期限/停机。keeper 只给 `consumed_buffer_ref`/`consumed_sha256` 诊断，
**不**产生 PlaybackAck、**不**声明 Ready，`output_available` 恒 `false`。

**未接线（如实）**：broker 连接对端认证（UDS 仍只作活性触发，核验的是声明 pid+uid+FD 能力，ADR 0018/0021）、
SCM_RIGHTS/Windows 命名管道句柄传递、protected IPC/信号停机、PlaybackAck 生产端与真实设备/窗口。

## 手工事件演练（--event-file）

Slice-0 链路演练：显式 `manual` 来源的手工事件经「归一化/去重 → 知识检索 → 智能体决策 → 守门四阶段 →
文本生成 → 决策日志（哈希链）」编排（ADR 0022）。脚本是一个 JSON 输入，字段见
`apps/engine/src/pipeline.rs::Script`：

- `session_id` / `workspace_id`：必须与现场会话一致；不一致拒绝，不重绑定（候选哈希只在作用域内有效）。
- `session`：操作员的会话起场——`operator`、`duty_ttl_ms`、`mode`（`assisted`/`auto_supervised`）、
  `lifecycle` 迁移序列。会话生命周期**从不**作为隐藏副作用被移动。
- `knowledge` / `knowledge_revision` / `top_k`：检索语料、修订与深度。
- `events`：手工事件（`source` 必须 `manual`，`kind` 为 `comment`/`gift`），每项带 `generation_epoch`
  与候选 `expires_at_utc_ms`。
- `approvals`：操作员审批记录，`candidate_hash` 绑定到**确切候选对象哈希**；`preview_candidates`
  可离线算出同一哈希作为审批草案，使审批是构造一致而非约定一致。
- `render`（可选，ADR 0023）：必须显式完整的渲染闭包——`manifest`、`config`、恰三条 `assets`
  （id + path）、至少一项 `licenses`、非空 `voice_ref` 与 `output_id`、`playback`
  （`complete`/`partial`/`invalid`/`cancelled`）。相对路径按**脚本自身目录**解析；任一缺失或非法
  在驱动任何会话状态前被拒绝，绝不代填默认资产、voice 或输出身份。

运行：

1. 装载并校验脚本（含 `render` 闭包）→ 建检索索引（复用 `nai-knowledge-core::VectorSpace` 的真实余弦
   排序，但嵌入为**精确词表索引**而非哈希技巧，所以「无知识命中」是真实可核验事实）→ 一次性准入 2D
   渲染闭包（素材许可/hash/闭包准入）。渲染闭包准入失败即整轮失败，不会退化成某个事件的失败。
2. 授予值守租约 → 快照现场 gate（session/workspace/epoch/生命周期）→ 校验脚本作用域一致。
3. 逐事件编排：复检 epoch/值守 → 检索（未命中记 `knowledge_no_hit`）→ 智能体决策 → 守门四阶段
   （审批绑定候选哈希）→ 文本生成 → 决策日志落库。守门缺失/未通过或陈旧 epoch **不外发**。
4. 给出 `render` 时的后半段（ADR 0023）：守门若**改写过候选文本**，被改写对象不再持有已审批哈希，
   事件在 `guard_superseded` 停下，**不发声**；随后复检值守 → 经 `SpeechSynthesizer` 接缝合成为有界
   PCM（样本总数由文本与采样率预先算出）→ 按帧边界切成有界 `AudioBlock` 流驱动 2D 渲染核心，产出
   **真实 RGBA 帧**并计入视频水位 → 双水位提交 → 演练播放槽按 `(output_id, utterance_id, 预期样本
   总数)` 发 ACK。只有「已提交 + `actual_local_playback_completed` + `played_sample_count == 预期总数`」
   才记 `PlaybackAck`；`partial`/`invalid`/`cancelled` 分别记 `playback_incomplete`/`playback_invalid`/
   `playback_cancelled` 且**不**记该 kind。
5. 以 `Injection::None` 重放整批并比对决策日志逐项一致（`replay_matched`）；校验哈希链（`chain_verified`）。
6. stdout 打印一行 `nai-engine pipeline: {summary}`；`chain_verified` 或 `replay_matched` 为假即返回错误。

`--inject` 仅供演练故障注入：使知识/守门/生成/语音/存储阶段失败，触发固定顺序
「P0 静音 → 撤权（epoch 递增）→ critical 日志 → 有界退出」；存储失败**不回退**已执行的静音。

**诚实边界（如实）**：文本生成用 `nai-provider-core` 的 **rehearsal-only 确定性桩**
（`rehearsal-deterministic-v1`），语音用同层同纪律的 **rehearsal-only 确定性语音桩**
（`rehearsal-speech-deterministic-v1`，离线、可复现、无网络、无凭据）。summary 显式标注
`rehearsal: true`、`real_provider: false`、`real_tts: false`、`devices: false`、`frame_slot: false`、
`keeper_consume: false`、`output_available: false`；`playback_ack` 仅表示「每个产出了文本的事件都走到了
经核验的播放完成」——输出身份是**演练软件替身**（`rehearsal-sink`），**不是**设备/硬件认证；
`delivery` 为 `memory`（帧只存在于内存，不写槽位、不经 keeper）。脚本不含 `render` 时链路止于文本，
`delivery: "none"`、`playback_ack: false`、无音频、无帧、无播放。两个桩都不得用于真实集成、付费调用
或任何认证结论。

## 停机

halt 文件 → 撤 token → P0 静音 + 撤权 + critical `cancellation` → 监督线程 stop（kill + 有界回收）→
HTTP watch 停机 → 退出码 0。

## 验证

```bash
cargo test -p nai-engine --offline
cargo test -p nai-engine --test pipeline --offline    # 手工事件链路集成（真实 engine 进程，13 项）
cargo test -p nai-provider-core --offline             # 文本/语音接缝与桩
cargo test -p nai-media-core --offline                # 时轴与播放通道
cargo build -p nai-output-keeper -p nai-render-worker --offline   # supervisor 测试需要
pnpm test:engine-smoke            # 真实 engine + 桌面 ControlClient
```

实测见 [报告](reports/2026-10-09-engine-assembly.md)：无 token 401、mute 后 epoch 递增并落 critical
日志、keeper 强杀后撤权且控制服务保持可达、HTTP 撤权后停止续租、媒体演练真实提交、halt 后退出 0。
`halt_during_media_drill_prewarm_never_commits_a_frame` 的**确定性由夹具保证**，不依赖「halt 请求发得足够早」：
把首个 avatar 素材换成**无写端命名管道**让 render-worker 钉在 prewarm（其素材按序读取，读第一项即阻塞），
并**等到该 worker 进程真的被派生**才写 halt；断言与决策链校验不放宽。该测试与夹具均为 Linux-only
（Windows 不编译），见 [报告](reports/2026-10-11-w48-deterministic-prewarm-halt-drill.md)。
手工事件链路的真实进程证据（哈希链校验、回放一致、守门缺失/陈旧 epoch/注入失败不外发）见
[报告](reports/2026-10-09-engine-manual-event-pipeline.md) 与 [ADR 0022](adr/0022-engine-manual-event-pipeline-boundary.md)。
文本→语音→2D→播放完成的证据（`PlaybackAck` 只在完整完成时出现、帧总数与声明样本总数一致、注入语音
失败有序停机、无 `render` 时链路不变）见
[报告](reports/2026-10-10-engine-text-to-playback-ack.md) 与 [ADR 0023](adr/0023-engine-text-to-playback-ack-boundary.md)。
