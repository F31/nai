## Context

§8.5 的四句话都是**否定式**的：两轨不相加、收音路径不重复、恢复不由事件触发。否定式的需求有一个共同的失败模式——把「不会发生」写成「记得检查」。一个 `if tts_on && talent_on { warn }` 的检查会在下一次重构里被绕过，而一个**没有构造点**的状态不会。

因此本设计的核心不是「实现了混音」，而是「**让不该发生的状态在类型上不可表达**」，同时让「该被报告的」都成为报告值而不是断言。

## Goals / Non-Goals

- Goal：接管的四件副作用可观测（报告清空条数）；两轨互斥由 `render()` 从模式推导；收音路径唯一由注册表的 `acquire/release` 强加；回声测试与恢复复核都是**显式前置**；审计对上报快照可构造。
- Non-Goal：不打开设备、不采 PCM、不做真实回声抑制；不做 keeper 进程装配；不做真实 OS 混音器。

## Decisions

### 决策 1：落点选 `crates/nai-media-core`（media 层），不新建 crate

`xtask/src/deps.rs` 对 media 层的约束是「不得依赖 provider 层、不得依赖 `plugins/llm-*`、`plugins/tts-*`、`plugins/gateway-*`，且不得有 HTTP 客户端」。本模块只需要 `nai_contracts::v1::ErrorCode` 与 `std`，天然满足。

不放进 `crates/nai-kernel`：`multiplex.rs`（W27，多会话）在 kernel 是因为多会话是内核**会话语义**的延伸；而 §8.5 是**声音通路**语义，与 media 层已有的音频时序（`timeline/audio.rs`）是同一个领域的两个正交面。kernel 也**不许**依赖 media，所以放在 kernel 会迫使 media 侧无法复用。

不新开 crate：新 crate 会改动 18 包计数与 `xtask check-deps` 的基线，而本模块与 media 层共享 `MediaError` 的邻居语义；沿用既有 crate 是 W25/W26/W27 一致的取舍。

### 决策 2：`render()` 从模式推导静音轨，而不是接受两个增益

被否决的做法：`AudioMixer` 暴露 `set_tts_gain` / `set_talent_gain`，由调用方保证接管时 TTS 为零。那样「两轨不相加」就退化成一个调用约定。

采用的写法：`AudioMixer` 内部持有两轨增益，但 `render()` 按 `mode` **强制**把非当前轨写成 `Gain::MUTE`。于是 `RenderLevels { tts: FULL, talent: FULL }` 不可能从 `render()` 产出——不是「检查了两轨不同时非零」，而是「两轨同时非零没有表示」。

`takeover` 仍然记录调用方给的 `talent_gain`（可能是 `MUTE`，对应「只暂停、不接入」这个合法形态）。

### 决策 3：唯一收音路径用「登记表 + 代次租约」，不是「一个布尔」

被否决：`capture_active: bool` + 检查。布尔无法回答「当前持有者是谁」，也无法阻止**迟到的释放**掐掉新持有者。

采用：`CaptureRegistry` 持有 `Option<CaptureLease>`，`acquire` 在有活跃路径时一律 `CONFLICT`（**不看 owner 也不看 device**——OBS 与伴侣的差别正是「另一条路径」而不是「同一个进程」）。租约带自增 `generation`，`release` 只接受当前代次，旧代次 `STALE_EPOCH`。

### 决策 4：回声测试是调用方驱动的状态值，带 TTL 且拒绝未来时刻

被否决：把回声测试做成 `takeover` 内部的自动流程（先跑、再判）。那会让接管在设备上产生**不可预测的时延**，而接管恰恰是运营按下的紧急动作。

采用：`EchoState` 由调用方驱动（`start` / `pass` / `fail`），`takeover` 只**要求**它是新鲜的 `Passed`。`valid_at` 同时拒绝「过期」与「未来」：未来时刻说明时钟或调用顺序出错，不该被当成有效证据。

### 决策 5：恢复走 `resume_auto(proof)`，`on_talent_silence` 只是一次查询

被否决：给 `on_talent_silence` 一个「若接管超过 N 秒则自动恢复」的定时器，或让它返回一个 `ResumeHint`。任何让「事件」通向「恢复」的形状都会让「不因真人讲话结束而自动恢复」变成可绕过的。

采用：`on_talent_silence(&self) -> MixMode` **只读**，返回当前模式；恢复的唯一入口是 `resume_auto(&mut self, proof, now)`，它每次复核授权、事实版本、值守租约三件。

### 决策 6：审计对上报快照做纯函数，保证每条规则可构造

W25 的教训：当审计写在对内部状态的访问上时，多数违规**不可达**（类型已保证），规则成了死代码，测试只能靠「相信它不会触发」。

采用：`MixSnapshot` 是普通值（`MixMode` + 两个 `Gain` + `live_captures` + `echo_valid`），`audit_mix` 读它。六条规则全部可以在测试里直接构造出来（`tests/mixing.rs::audit_detects_each_violation_by_token`），所以规则是**活的**：即使未来有人绕过 `render()`，审计也能报出来。

## Risks / Trade-offs

- **审计与类型保证的重叠**：`both_tracks_audible` 在正常路径下不可达（决策 2）。保留它的价值在于「防未来绕过」，代价是当前它只被快照测试覆盖。这是刻意的，见决策 6。
- **TTL 是常量 5 分钟**：写死会限制长句间的接管。可接受——回声测试本就是接管**前**的动作，超过 5 分钟的间隔应重测。

## Migration Plan

无迁移。`mixing.rs` 是新增模块，不改任何既有模块的公开 API。keeper 进程内的装配（把 `CaptureRegistry` 与 `AudioMixer` 接进真实的输出通路）是后续 runtime 接线，本轮不做。

## Verification

- `cargo test -p nai-media-core --test mixing`（18 项）
- `cargo clippy -p nai-media-core --all-targets --locked -- -D warnings`
- `cargo xtask check-deps`（18 包与依赖方向不变）
- `cargo xtask check --profile portable`（全量留档，数字见报告）
- `cargo xtask check --registry`、`schema --check`、`openspec validate --all --strict --archived`

## Environment Boundary

本模块在 portable 与 Windows 上都不打开设备，因此两端行为一致。真实验收（出声、无回声、真实设备默认变化）需要真实声卡与真实 OBS/伴侣环境，属设备里程碑，本轮记为未完成而不是通过。
