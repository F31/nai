# 真人接管音轨与回声控制（W11 / B05）

- 代码：`crates/nai-media-core/src/mixing.rs`（media 层）
- 测试：`crates/nai-media-core/tests/mixing.rs`
- 决策：[ADR 0030](adr/0030-talent-takeover-boundary.md)；判据：`tests/registry.yaml` B05（V3.11 §8.5、W11）

本模块把「接管」这件事的**通路语义**落成可执行判据：哪条轨在出声、哪条麦克风在收音、什么时候允许回到自动。
它不打开任何设备、不采 PCM、不做回声抑制——真实声卡、真实 OBS/伴侣环境与真实回声测试是设备里程碑。

## 接管的四件副作用

`AudioMixer::takeover(TakeoverRequest)` 一次做完四件事，并返回 `TakeoverReport`：

| 副作用 | 落点 |
| --- | --- |
| 暂停自动 | `mode → Takeover` |
| 数字人静音 | TTS 轨增益 → `MUTE` |
| 清空队列 | 返回并累计**实际**清空条数（`cleared_utterances`） |
| 可选接入真人轨 | `talent_gain` 记入 `TakeoverReport::talent` |

前置条件两条：**回声测试新鲜通过**、**事实版本非空**（恢复时要拿它复核）。已处于接管时再次接管是 `CONFLICT`——一次接管只对应一条收音路径。

## 两条轨永不相加

`AudioMixer::render()` 从**模式**推导被静音的那条轨：

- `Auto` → `{ tts: tts_gain, talent: MUTE }`
- `Takeover` → `{ tts: MUTE, talent: talent_gain }`

因此 `{ tts: FULL, talent: FULL }` **没有构造点**。这不是「接管时记得把 TTS 置零」的约定，而是置零这件事不由调用方决定。
`talent_gain` 允许为 `MUTE`，对应规格里「可选真人音轨接入」——只暂停、不接入是合法形态。

## 唯一收音路径与代次租约

`CaptureRegistry` 强制「只允许一条有效收音路径」：

- `acquire(path)`：已有活跃路径即 `CONFLICT("capture path already held")`。**不看 owner，也不看 device**——不同进程、不同设备同样是「第二条路径」，这正是 OBS 与伴侣重复收音造成回声的场景。
- `release(&lease)`：只接受**当前代次**。旧代次的租约以 `STALE_EPOCH` 拒绝，所以一次迟到的释放不会掐掉新持有者。
- `DeviceId` 是**稳定**的不透明标识；`observe_default(device)` 把系统默认设备的变化报成 `Changed { from, to }` 并记住，**绝不静默跟随**——跟着默认设备走会把直播挪到没人测过的设备上。

## 回声测试是接管的前置

`EchoState` 由调用方驱动（`start` / `pass(utc_ms)` / `fail`），`AudioMixer` 只**要求**它是新鲜的 `Passed`：

| 状态 | 接管结果 |
| --- | --- |
| `Unrun` / `Running` / `Failed` | `CAPABILITY_NOT_READY` |
| `Passed` 但超出 `ECHO_TEST_TTL_MS`（5 分钟） | `EVIDENCE_EXPIRED` |
| `Passed` 且未过期 | 允许，`TakeoverReport::echo_accepted = true` |

`valid_at` 同时拒绝「过期」与「未来时刻」：通过时刻晚于当前时刻说明时钟或调用顺序出错，不应被当作证据。旧的通过不是「现在没回声」的证据。

## 恢复自动是决策，不是事件

- `resume_auto(&proof, now)` 每次复核三件：`authorized`、事实版本与**接管时快照**一致且非空、值守租约**未到期**。任一不满足即 `PERMISSION_DENIED`，模式保持接管。
- `on_talent_silence(&self) -> MixMode` 是**只读查询**，只返回当前模式。真人讲话结束**没有**通向恢复的代码路径。

## 混音审计

`audit_mix(&MixSnapshot)` 是对**上报快照值**的纯函数（`MixSnapshot` 是普通值），报出**全部**违规并带稳定标记，顺序固定：

| 标记 | 含义 |
| --- | --- |
| `both_tracks_audible` | 两条轨同时可听（被相加） |
| `takeover_with_tts_audible` | 接管时 TTS 未静音 |
| `auto_with_talent_audible` | 自动时真人有声 |
| `takeover_without_capture_path` | 接管却没有收音路径 |
| `takeover_without_valid_echo_test` | 接管却没有有效回声测试 |
| `multiple_live_capture_paths` | 多于一条活跃收音路径 |

把审计写成对**公开快照**的纯函数，是为了让每条规则都能在测试里直接构造出来——否则「类型已经保证了」会让规则变成永远不触发的死代码。

## 与 `timeline/audio.rs` 的职责区分

`timeline/audio.rs` 管音频块的**内容**元数据（采样率、格式、样本数、EOF 校验）；`mixing.rs` 管**通路**（哪条轨出声、哪条麦收音）。
两者正交：一个块可以在接管时被丢弃（通路不允许），而其内容本身仍然合法。

## 未做边界

- **不打开设备、不采 PCM、不做回声抑制**：`EchoState` 由调用方驱动，本模块只要求它新鲜。
- **不做 keeper 进程内装配**：把 `AudioMixer` / `CaptureRegistry` 接进真实输出通路是 runtime 接线。
- **不引入第二条恢复路径**：没有「真人停止 N 秒后自动 resume」的定时器或事件钩子。
- **真机验收未做**：出手声、无回声、真实设备默认变化需要真实声卡与真实 OBS/伴侣环境，registry 保持 `implemented` 而不标 `tested`。
