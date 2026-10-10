# ADR 0030：真人接管音轨的边界——两轨互斥、唯一收音路径、恢复是决策不是事件

- 日期：2026-10-10
- 状态：Accepted
- 基线：V3.6 §8.5（真人接管音轨）、§30.4 B05、§31.4；承接 ADR 0023（演练播放通道）、ADR 0027（部署生命周期）、ADR 0029（多会话隔离）
- 变更：`openspec/changes/w28-talent-takeover/`
- 判据：`tests/registry.yaml` B05「真人接管音轨与回声」（W11）

## 背景

§8.5 用四句话定义接管：接管 = 暂停自动 + 数字人静音 + 清队列 + 可选真人音轨接入；真人麦克风由 keeper 混音输出，
**独立于 TTS 音轨**，接管时 TTS 轨静音，设备用稳定标识并监控默认设备变化；**只允许一条有效收音路径**，避免 OBS 与伴侣
重复收音造成回声，接管前做**回声测试**；恢复自动必须**明确操作**并复核授权、事实版本与值守租约，**不因真人讲话结束而自动恢复**。

这四句话的共同形状是**否定**：两轨不相加、第二条路径不出现、讲话结束不恢复。而 `tests/registry.yaml` 里 B05 是 `pending`，
notes 写着「混音/回声未实现（W11）」——这不是「机制在、缺设备证据」，而是真的没写。

四个问题必须先回答：放**哪个 crate**；「两轨不相加」靠**什么**成立；「唯一收音路径」靠**什么**成立；
「恢复不被事件触发」靠**什么**成立。

## 决策

### D1 放进 `crates/nai-media-core`（media 层），不放进 kernel，不新开 crate

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 放进 `nai-kernel`（与 `multiplex.rs` 并列） | 与多会话隔离同住 | **否决**：kernel **不许**依赖 media（`xtask/src/deps.rs` 的 `kernel => target.layer != "contracts"`），而这是声音通路语义，放 kernel 会让 media 侧无法复用；且 §8.5 与 kernel 的会话语义无交集。 |
| B. 新增 `nai-audio-mix-core` | 独立成包 | **否决**：改动 workspace 组成（18 包）与依赖方向，而本模块只依赖 `ErrorCode` 与 `std`。 |
| C. 放进 `nai-media-core` 的 `mixing.rs` | 与 `timeline/audio.rs` 并列 | **采用**：media 层的约束（不得依赖 provider 与 `plugins/llm-*|tts-*|gateway-*`、不得有 HTTP 客户端）天然满足；18 包与依赖方向不变；`xtask` 的 media 测试目标已覆盖该 crate。 |

代价：media 层只有 `MediaError`（媒体许可语义），不足以表达「冲突路径 / 证据过期 / 权限不足」这类接管特有失败。故自带
`MixError { code, reason }`，与 `UpgradeError` / `MultiplexError` 同形——分层约束下一处可接受的重复，报告仍可统一分类。

### D2 `render()` 从模式**推导**静音轨，不接受两个增益 setter

| 方案 | 结论 |
|---|---|
| 暴露 `set_tts_gain` / `set_talent_gain`，由调用方保证接管时 TTS 为零 | **否决**：把「两轨不相加」降级成一个调用约定，漏写不会让任何测试变红。 |
| 在 `render()` 里检查两轨是否同时非零并报警 | **否决**：报警说明状态**可以**存在；应当让状态没有构造点。 |
| `render()` 按 `mode` 强制把非当前轨写成 `MUTE` | **采用**：`{ tts: FULL, talent: FULL }` 表示不出来。 |

### D3 唯一收音路径用「登记表 + 代次租约」，不用布尔

| 方案 | 结论 |
|---|---|
| `capture_active: bool` + 检查 | **否决**：回答不了「当前持有者是谁」，也拦不住**迟到的释放**掐掉新持有者。 |
| 按 owner 去重（同进程幂等、跨进程拒绝） | **否决**：OBS 与伴侣正是**不同进程**，按 owner 去重会把最需要拦的场景放行。 |
| `Option<CaptureLease{path, generation}>`，`acquire` 一律拒绝第二条、`release` 校验代次 | **采用**：`CONFLICT` 不看 owner/device；旧代次 `STALE_EPOCH`。 |

### D4 回声测试由调用方驱动，带 TTL 且拒绝未来时刻

| 方案 | 结论 |
|---|---|
| `takeover` 内部先跑回声测试再判 | **否决**：接管是运营按下的紧急动作，内部测试会给它不可预测的时延；且测试需要真实设备，portable 下跑不动。 |
| 只判断「最近一次测试是通过」 | **否决**：旧的通过不是「现在没回声」的证据。 |
| `EchoState` 状态值 + `valid_at(now)` 要求新鲜且非未来 | **采用**：TTL 5 分钟；`Unrun/Running/Failed` → `CAPABILITY_NOT_READY`，过期 → `EVIDENCE_EXPIRED`。 |

### D5 恢复走 `resume_auto(proof)`，`on_talent_silence` 只查询

| 方案 | 结论 |
|---|---|
| `on_talent_silence` 触发「N 秒后自动恢复」的定时器 | **否决**：直接违反「不因真人讲话结束而自动恢复」。 |
| `on_talent_silence` 返回一个 `ResumeHint` 供调用方使用 | **否决**：任何让事件**通向**恢复的形状都会被下一次重构走成自动恢复。 |
| `on_talent_silence(&self) -> MixMode` 只读；恢复唯一入口是 `resume_auto` | **采用**：恢复每次复核授权 + 事实版本 + 值守租约三件。 |

### D6 审计对**上报快照**做纯函数

W25 的教训：审计写在对内部状态的访问上时，多数违规**不可达**（类型已保证），规则成了死代码。

| 方案 | 结论 |
|---|---|
| 审计读 `AudioMixer` 内部字段 | **否决**：规则不可构造，测试只能「相信它不会触发」。 |
| `MixSnapshot` 普通值 + `audit_mix` 纯函数 | **采用**：六条规则全部可在测试里直接构造，规则是活的（防未来绕过）。 |

## 后果

- 「两轨不相加」「唯一收音路径」「恢复不被事件触发」三条分别落成：从模式推导的渲染、登记表 + 代次租约、只读的 `on_talent_silence`。
- 代价：真实回声质量（回声抑制系数、真实设备的默认设备变化）不在本模块，需要设备里程碑。
- `MixError` 与 `MediaError` 并存：前者是接管通路语义，后者是媒体许可语义，两者错误码都映射到同一 `ErrorCode` 空间。

## 验证

- `cargo test -p nai-media-core --test mixing` = 18 passed。
- `cargo clippy -p nai-media-core --all-targets --locked -- -D warnings`。
- `cargo xtask check-deps`（18 包与依赖方向不变）、`cargo xtask check --registry`、`cargo xtask check --profile portable`。
- `openspec validate w28-talent-takeover --strict --no-interactive`。

## 未做边界

- 不打开设备、不采 PCM、不做回声抑制；`EchoState` 由调用方驱动。
- 不做 keeper 进程内的装配（runtime 接线）。
- `automation: manual` 的真机验收（出声、无回声、真实设备默认变化）属设备里程碑，registry 保持 `implemented` 而不标 `tested`。
