# 报告：engine-text-to-playback-ack（文本 → 语音 → 2D → 播放完成 ACK）

- 日期：2026-10-10
- 变更：`openspec/changes/engine-text-to-playback-ack/`
- 基线：V3.6 §1.4（Slice-0）、§21.3/§27（媒体时轴与 ACK）；ADR 0023（演练链路边界）
- 结论：Slice-0 演练链路的**可移植软件闭环已从「文本」延到「播放完成」并实测**；
  **不是**真实 TTS、**不是**设备/硬件播放 ACK、**不是** Windows 认证，帧只在内存

## 背景

上一轮（`engine-manual-event-pipeline`）把编排落在 `apps/engine`，但链路**止于文本**：没有 TTS 段、
没有像素、没有播放完成判据。而 `media-basics-timeline` 主规格早已要求「仅 `committed &&
actual_local_playback_completed && played_sample_count = 样本总数` 才算 Completed」，
`nai-media-core` 也已有 `AckTracker`/`UtteranceTimeline`，`nai-avatar-2d` 也已能产出真实 RGBA 帧——
但**只有单元测试**，没有任何一处把它们串成真实进程链路。结果是「提交被误当完成」「帧总数靠回读自证」
这类错误不会被任何工程判据拦住。ADR 0022 已把本轮命名为 `engine-text-to-playback-ack`。

## 实现

### 1. 语音接缝与 rehearsal 桩（`crates/nai-provider-core/src/speech.rs`，新增）

- `SpeechRequest`（request/provider/session/utterance 标识、`generation_epoch`、文本、目标采样率、
  voice 绑定、剩余预算、取消标志）与 `SpeechResponse`（PCM、采样率、模型标识、usage）。
- 请求校验：拒空文本/空 voice、采样率不在 `8000..=48000`、超长文本（`MAX_SPEECH_CHARS = 4096`）；
  已取消 → Cancelled；`remaining_budget_ms == 0` → DeadlineExceeded。
- 响应校验：采样率必须与请求一致、长度有界（`MAX_SPEECH_SAMPLES = 240_000`）、样本有限且 `|s| <= 1.0`。
- `synthesize_with_budget(...)`：reserve → dispatch → 合成 → 校验响应 → `usage_charge` → `finish`，
  与 `generate_with_budget` 同形；已派发请求返回 `Already`（Conflict）而**不重发**。
  既有 HTTP 适配器与 `UsageGateway` 行为未变。
- `RehearsalSpeechSynthesizer`（`model_ref = "rehearsal-speech-deterministic-v1"`）：样本总数
  `= clamp(字符数 × 60ms, 400ms, 5000ms) × sample_rate / 1000`，**只由文本与采样率决定**，因此
  「预期样本总数」可以在合成之前独立算出；波形由文本 FNV-1a 种子经 xorshift 生成（幅度 0.8）。
- 5 项单测。

### 2. 演练播放通道（`crates/nai-media-core/src/playback.rs`，新增）

- `frame_boundary(seq, rate, fps_num, fps_den)` 与软件渲染器**同一套整数算术**（含截断），
  `frame_count(samples, …)` 由样本总数推出帧总数：帧总数是**声明值**，不是回读值。
- `PlaybackLane`：`push_audio` 计入音频水位、`push_frame(seq)` 计入视频水位（1 基、严格连续、不超声明帧数）；
  `commit` 只在双水位达到时为真，且**提交不等于播放完成**。
- `acknowledge(&OutputAck)` 绑定 `(output_id, utterance_id, 预期样本总数)`：未提交即声明进度/完成 →
  `InvalidAck`；身份或样本数不符 → 拒绝；`Cancelled` 之后任何 ACK 都不能再完成。
- `RehearsalSink`（`OUTPUT_KIND = "rehearsal-sink"`）+ `rehearse(...)` 驱动
  `complete|partial|invalid|cancelled` 四个**正常结局**（非故障注入），返回
  `SinkReport { accepted, rejected, completed }`。帧总数上界 `MAX_FRAMES = 4096`。
- 8 项单测（`timeline/ack.rs` 另加 `AckTracker::for_utterance` 以便无代表块构造）。

### 3. engine 链路延伸（`apps/engine/src/pipeline.rs`）

- 脚本新增可选 `render` 段：`manifest`/`config`/恰三条 `assets`/至少一项 `licenses`/`voice_ref`/
  `output_id`/`playback`；相对路径按**脚本自身目录**解析；任一缺失或非法在驱动任何会话状态前拒绝，
  **不代填默认资产、voice 或输出身份**。
- 运行序：装载校验脚本 → 建检索索引 → **一次性准入 2D 渲染闭包**（失败即整轮失败，不退化成事件级失败）
  → 值守/作用域 → 逐事件：检索 → 决策 → 守门四阶段 → 文本生成。
- 给出 `render` 时的后半段：守门若改写过候选（候选哈希被清空）→ `guard_superseded`，**不发声**；
  → 复检值守/epoch → 经语音接缝合成为有界 PCM → 按帧边界切有界 `AudioBlock` 流驱动 2D 渲染核心产出
  **真实 RGBA 帧**（同时核验渲染器的 `frame_seq` 连续，1 基化后计入水位）→ 双水位提交 →
  `rehearse(...)` 应用声明的播放场景。
- 落库 kind：`stage == "played"` → `DecisionKind::PlaybackAck`；`guard_*` → `Guard`；
  已产出文本 → `Synthesis`（**保持已归档文本链路的 kind 不变**）；其余 → `Observation`。
- 摘要新增 `speech`/`renderer`/`playback_sink`/`playback_scenario`/`real_tts`/`frame_slot`/
  `keeper_consume`/`delivery`/`played`，`playback_ack` 改为「每个产出文本的事件都走到了经核验的播放完成」，
  并恒标 `devices=false`、`output_available=false`。
- `--inject speech` 与 USAGE 更新；**未给出 `render` 时该注入被拒绝**（无法生效的注入不静默忽略）。

## 验证（本轮实际执行）

| 检查 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -D warnings` | 通过 |
| `cargo xtask check-deps` | 17 包依赖边界通过 |
| `cargo xtask check --registry` | 通过 |
| `cargo test -p nai-provider-core --offline` | lib 11 passed（text 6 + **speech 5** + usage 等）+ `tests/provider.rs` 10 passed |
| `cargo test -p nai-media-core --offline` | lib 28 passed（含 **playback 8**）+ timeline/frame_slots/gate 15 passed |
| `cargo test -p nai-engine --test pipeline --offline` | **13 passed**（原 7 + 本轮 6） |
| `cargo test -p nai-avatar-2d --offline` | 9 passed（未改动） |
| `cargo test --workspace --locked --offline --no-fail-fast` | **317 passed / 13 failed**（全为既有 Windows 平台差异，见下） |
| `cargo xtask check --profile portable` | **本机无法整体通过**（两个独立原因，见下）——已按其四步等价逐条单独执行 |
| `cargo xtask schema --check` / `test --suite contracts` | **未执行**（本机 node 无法 spawn 子进程，EBUSY） |
| `pnpm typecheck` / `test:engine-smoke` | **未执行**（本机无 pnpm，且 node spawn 不可用） |

### 本轮新增的集成测试（`apps/engine/tests/pipeline.rs`，真实 engine 子进程）

1. `an_approved_event_plays_through_the_lane_to_a_verified_acknowledgement`：完整完成 → `stage=played`、
   `played=1`、`playback_ack=true`、`delivery=memory`，决策日志出现**恰好一条** `PlaybackAck`
   （payload 记 `playback=completed`、`delivery=memory`、语音模型标识），哈希链校验与两遍回放均通过；
   并**独立核验**「产生的帧数 == 由声明样本总数与 8kHz/25fps 推出的帧数」——即帧总数先于任何帧就已声明。
2. `a_playback_that_did_not_complete_never_logs_an_acknowledgement`：`partial`/`invalid`/`cancelled`
   三种正常结局分别落 `playback_incomplete`/`playback_invalid`/`playback_cancelled`，
   `playback_ack=false`、`played=0`、**无任何** `PlaybackAck` 条目，kind 保持 `Synthesis`。
3. `injected_speech_failure_halts_in_order_after_the_text_stage`：`--inject speech` → 退出 2、
   stderr 含 `synthesis failed`、有 critical `mute` 记录、**无**该事件的 pipeline 条目。
4. `a_speech_injection_without_a_render_closure_is_refused`：无 `render` 段时该注入被显式拒绝。
5. `a_render_closure_that_cannot_be_admitted_fails_the_run_not_an_event`：manifest 缺失 →
   `avatar manifest unreadable`；许可不符 → `avatar license not granted by root`（都不默认）。
6. `the_render_lane_does_not_change_a_script_that_declares_none`：无 `render` 时 `delivery=none`、
   `playback_ack=false`、`audio_samples=0`、`frames=0`、`playback=none`，kind 仍为 `Synthesis`。

### 为什么 `cargo xtask check --profile portable` 在本机跑不通（如实记录）

两个**互相独立**的原因，均非本轮代码缺陷：

1. **包装命令会重链自己**。该命令先跑 `cargo clippy --workspace --all-targets`，再跑
   `cargo test --workspace --locked`。实测：clippy 之后 `cargo test --workspace` 会重新链接
   `deps/nai_xtask.exe`（`Compiling nai-xtask`）。而此时正在运行的就是这个文件，Windows 锁定运行中的
   可执行文件，链接器报 `LNK1104: 无法打开文件 ...\nai_xtask.exe`（exit 101）。Linux 允许覆盖运行中的
   可执行文件，所以这是 Windows 宿主特有的包装器缺陷。复现：
   ```bash
   cargo clippy --workspace --all-targets --locked -- -D warnings   # 通过
   cargo test --workspace --locked --no-run                          # => Compiling nai-xtask（随后自我锁定）
   ```
2. **其内层 `cargo test --workspace --locked` 本身就会失败**：它包含 13 项既有 Windows 平台差异失败
   （见下），因此即便链接成功，包装命令也会以非零退出。

因此本轮按该命令的**等价四步**逐条单独执行并记录（fmt / clippy / check-deps+registry / workspace 测试）。
上一轮报告把 `check --profile portable` 记为「通过」、同时又把 workspace 测试记为「296 passed / 14 failed」，
两者互相矛盾；本轮按实测如实记录，不沿用该结论。

### `cargo test --workspace` 的 13 项失败（全部既有，均不在本轮改动文件内）

- `apps/output-keeper/tests/faults.rs`（6）：命名管道/FIFO（`ERROR_PIPE_BUSY`）。
- `apps/engine/tests/engine.rs`（3）：`/proc/<pid>` 探测 keeper 子进程、`0600` 权限位断言。
- `crates/nai-storage/src/tests*.rs`（4）：`fs::rename` 覆盖语义与备份/遍历断言。

本轮新增测试（provider speech 5、media playback 8、engine pipeline 集成 6）全部通过。

## 诚实边界（如实）

- `delivery = "memory"`：帧只存在于内存，**不**写帧槽位、**不**经 keeper、**不**涉设备。
- 演练播放槽 `rehearsal-sink` 是**软件替身**：它不产生任何输出、不证明任何硬件行为。
  `DecisionKind::PlaybackAck` 只表示「一个本地播放替身报告它播完了这一句」，
  **不是**设备/硬件认证；摘要恒标 `devices=false`、`output_available=false`。
- 真实 TTS 未接（`real_tts=false`）；真实 Provider 付费调用仍未完成（账户无余额，见 D08）。
- 帧总数上界 4096；更长的一句话必须由调用方分块，不得声明成无界区间。

## 验收状态

`tests/registry.yaml`：R07 补「`PlaybackAck` 条目进入哈希链」证据、A01 补「文本→播放完成」链路证据，
**均保持 implemented 不升级**——该 ACK 来自演练软件替身，真实 TTS 与设备/硬件播放 ACK 生产端仍缺。

## 归档

`openspec/changes/archive/2026-10-10-engine-text-to-playback-ack/`；主规格 `runtime-composition`（+3）、
`media-basics-timeline`（+2）、`api-provider-transport`（+2）增量生效。
`openspec validate --all --strict`：24 个主规格通过；`validate --archived`：27 项归档通过。ADR 0023。
