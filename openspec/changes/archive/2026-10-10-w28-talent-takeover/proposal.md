## Why

`tests/registry.yaml` 里 B05 是 `pending`，notes 写着「混音/回声未实现（W11）」。与 R06/B04 不同，这不是「机制已就位、只缺设备证据」，而是一条**真的没写**的代码缺口。V3.6 §8.5 用四句话把它说成一组必须同时成立的性质：

| 出处 | 原文 |
| --- | --- |
| §8.5 | 接管 = 暂停自动 + 数字人静音 + 清队列 + 可选真人音轨接入 |
| §8.5 | 真人麦克风由 keeper 混音输出，独立于 TTS 音轨；接管时 TTS 轨静音。设备使用稳定标识，监控默认设备变化 |
| §8.5 | 只允许一条有效收音路径，避免 OBS 与伴侣重复收音造成回声；接管前做回声测试 |
| §8.5 | 恢复自动互动必须明确操作，并复核授权、事实版本与值守租约，不因真人讲话结束而自动恢复 |
| §30.4 B05 | 真人接管：出声、无回声；恢复自动需复核 |

这四句话有共同的形状：**它们是「不会发生」的性质**——两轨不会同时出声、不会出现第二条收音路径、不会因为真人讲完话就回到自动。把否定写成「记得检查」的代码，是这类需求最常见的失败方式。本轮把它们落成**结构性**判据，让否定由类型与状态机承载。

为什么是 `nai-media-core`（media 层）：§8.5 说的是**输出侧**的声音通路——keeper 混音、TTS 轨静音、收音路径唯一，这些是媒体许可与媒体时序的邻居。`crates/nai-media-core/src/timeline/audio.rs` 已经在管音频块的**内容**元数据（采样率、样本数、EOF）；本轮的 `mixing.rs` 管的是**通路**（哪条轨在出声、哪条麦在收音）。两者正交，合并会让 media 层同时承担两种职责。

## What Changes

- **接管是一条有副作用的操作（`AudioMixer::takeover`）**：它**同时**暂停自动、把 TTS 轨推到 `MUTE`、清空自动队列并（可选）接入真人轨，然后返回一份 `TakeoverReport`——把「清掉了几条」当成**报告值**而不是断言。重复接管以 `CONFLICT("already in takeover")` 拒绝：一次接管只对应一条收音路径。接管要求一个非空**事实版本**（`INVALID_ARGUMENT`），因为恢复时要拿它复核。

- **两条轨永不相加（`AudioMixer::render`）**：`render()` 从**模式**推导被静音的那条轨（`Auto` → 真人轨静音；`Takeover` → TTS 轨静音），所以「两轨同时可听」这个状态**没有构造点**。这不是「接管时记得把 TTS 置零」，而是置零这件事根本不由调用方决定。

- **唯一收音路径是注册表的性质（`CaptureRegistry`）**：`acquire` 在已有活跃路径时以 `CONFLICT("capture path already held")` 拒绝——**不同 owner、不同设备也一样拒绝**，那正是 OBS 与伴侣重复收音的原始场景。租约带**代次**，`release` 只接受当前代次，旧租约以 `STALE_EPOCH` 拒绝，所以一次迟到的释放不会掐掉新的持有者。麦克风以**稳定设备标识**（`DeviceId`，不透明字符串）表示；`observe_default` 把系统默认设备的变化报成 `Changed { from, to }` 并**记住**，绝不静默跟随——跟着默认设备走会把直播挪到没人测过的设备上。

- **回声测试是接管的前置（`EchoState`）**：接管要求 `Passed` 且**未过期**（TTL 5 分钟）。未跑/运行中/失败以 `CAPABILITY_NOT_READY` 拒绝，过期以 `EVIDENCE_EXPIRED` 拒绝，`Passed` 的时刻晚于当前时刻（未来）也**不接受**。旧的通过不是「现在没回声」的证据。

- **恢复自动是决策，不是事件（`resume_auto` / `on_talent_silence`）**：`resume_auto` 每次复核三件——授权、**接管时快照的事实版本**、仍在有效期内的**值守租约**，任一不满足即 `PERMISSION_DENIED`。而 `on_talent_silence` 只**返回**当前模式，**没有**回到 `resume_auto` 的代码路径；「真人讲完就恢复」在 API 上不可表达。

- **可构造的审计（`audit_mix`）**：对**上报快照值**（`MixSnapshot`）的纯函数，检出六类状态：`both_tracks_audible`、`takeover_with_tts_audible`、`auto_with_talent_audible`、`takeover_without_capture_path`、`takeover_without_valid_echo_test`、`multiple_live_capture_paths`。快照是普通值，所以每条违规都能在测试里直接构造——审计不会被「类型已经保证了，所以规则不可达」架空。

## Capabilities
### New Capabilities
- `talent-takeover`：接管的四件副作用与报告、两轨互斥的渲染不变式、唯一收音路径注册表与代次租约、稳定设备标识与默认设备变化观察、回声测试前置与 TTL、恢复自动的三项复核、混音审计。覆盖 registry **B05**（W11）。

### Modified Capabilities
- 无。既有 `media-basics`（媒体时序内核）与 `output-watchdog-process`（keeper 看门狗）**不改**：本变更在其之上回答「真人接管时，哪条轨出声、哪条麦在收音、什么时候允许回到自动」。

## Impact

- 代码：`crates/nai-media-core/src/` 新增 `mixing.rs`；`tests/` 新增 `mixing.rs`（18 项）；`lib.rs` 增 `pub mod mixing;` 与再导出。**不新增 crate，不改 workspace 依赖**（维持 18 包与依赖方向；media 层仍不依赖 provider/插件，仍无 HTTP 客户端）。
- 规格：新增 capability `talent-takeover`；新增 `docs/mixing.md`、ADR 0030、报告 `docs/reports/2026-10-10-w28-talent-takeover.md`。
- 依赖：**零新增**。仅用 `nai_contracts::v1::ErrorCode` 与 `std`。
- registry：B05 补证据并按实现 `pending → implemented`（`automation: manual`，真实声卡、真实 OBS/伴侣与真实回声环境未接入，**不标 `tested`**）。
- 明确不做：不打开任何设备、不做真实回声测试（不采 PCM、不算回声抑制系数）；不做真实 OS 混音器接线；不做 keeper 进程内的装配（那是 runtime 接线）。

## Non-Goals

- 不用混音控制语义替代真机验收：本轮证明的是「两轨不可同时出声、收音路径唯一、恢复不被事件触发」这些**结构性判据**，不宣称任何真实声卡上无回声。
- 不把回声测试做成内部自动流程：`EchoState` 由调用方驱动，本模块只**要求**它通过且新鲜。
- 不引入第二条恢复路径：没有「talent 停止 N 秒后自动 resume」的定时器或事件钩子。
