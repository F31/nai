# W28 实测报告：真人接管音轨与回声控制（B05 / W11）

- 日期：2026-10-10
- 变更：`openspec/changes/w28-talent-takeover/`
- 决策：[ADR 0030](../adr/0030-talent-takeover-boundary.md)；指南：[docs/mixing.md](../mixing.md)
- 判据：`tests/registry.yaml` B05

## 结论

B05 此前是 registry 里少数**真的没写**的代码缺口（notes：「混音/回声未实现（W11）」）。本轮把 §8.5 的四句话落成
`crates/nai-media-core/src/mixing.rs` 里的结构性判据，**零新增依赖**（media 层仍只依赖 contracts/prost/sha2，仍无 HTTP 客户端），
18 包与依赖方向不变，全部在 portable 下可测。

registry B05 由 `pending` 升为 `implemented`，**不标 `tested`**：`automation: manual` 的真机验收（出声、无回声、真实设备默认变化）需要真实声卡与真实 OBS/伴侣环境，未接入。

本轮的关键取舍是：把四条**否定式**要求（两轨不相加、第二条路径不出现、讲话结束不恢复、接管前必有回声测试）写成「类型里没有这个状态」，而不是「调用方记得检查」。

## 目标行为

| 判据（§8.5） | 实现 | 关键设计 |
| --- | --- | --- |
| 接管 = 暂停自动 + 静音 + 清队列 + 可选接入 | `AudioMixer::takeover` | 返回 `TakeoverReport`，清空条数是**报告值**；重复接管 `CONFLICT` |
| 真人麦独立于 TTS 轨；接管时 TTS 轨静音 | `AudioMixer::render` | 静音轨由**模式推导**，`{tts>0, talent>0}` 没有构造点 |
| 设备稳定标识；监控默认设备变化 | `DeviceId`、`CaptureRegistry::observe_default` | 变化报成 `Changed{from,to}` 并记住，**不静默跟随** |
| 只允许一条有效收音路径 | `CaptureRegistry::acquire` | **不看 owner/device** 一律拒第二条；`release` 只收当前代次 |
| 接管前做回声测试 | `EchoState` + `takeover` 门控 | 未跑/运行中/失败 → `CAPABILITY_NOT_READY`；过期 → `EVIDENCE_EXPIRED`；未来时刻无效 |
| 恢复自动必须明确操作 | `AudioMixer::resume_auto` | 复核授权 + 事实版本 + 值守租约 |
| 不因真人讲话结束而自动恢复 | `AudioMixer::on_talent_silence` | **只读**查询，没有通向恢复的路径 |

## 实测数字

- `cargo test -p nai-media-core --test mixing`：**18 passed / 0 failed**
  - 接管 6：初始自动态、两轨不可同时可听、接管四件副作用与报告、重复接管冲突、空事实版本被拒、队列清空计数。
  - 回声 2：三种未通过状态与 TTL 边界、过期与未来时刻无效。
  - 恢复 4：三项复核与各自失败、已自动时冲突、讲话结束不恢复、恢复后 TTS 复位且真人轨静音。
  - 路径与设备 3：唯一路径与代次租约（含迟到释放 `STALE_EPOCH`）、释放未持有路径被拒、默认设备三态。
  - 校验与审计 3：标识与增益校验、队列有界且仅自动、审计干净快照与每条违规的稳定标记（含四规则同触发的固定顺序）。
- `cargo clippy -p nai-media-core --all-targets --locked -- -D warnings`：通过（零告警）。
- `cargo xtask check --profile portable`：见下「门控」。

## 逐项验收映射（B05）

| 条款 | 覆盖 | 位置 |
| --- | --- | --- |
| 真人接管：出声 | 接管接入真人轨，`render()` 在接管时输出真人轨增益 | `AudioMixer::takeover` / `render` |
| 真人接管：无回声 | 唯一收音路径 + 接管前回声测试通过（+ TTL） | `CaptureRegistry::acquire`、`takeover` |
| 真人接管：恢复自动需复核 | 授权 + 事实版本 + 值守租约三项复核，且讲话结束不恢复 | `resume_auto` / `on_talent_silence` |
| §8.5 设备稳定标识与默认变化 | `DeviceId` 不透明标识；`observe_default` 报变化 | `CaptureRegistry::observe_default` |

## 门控

- `cargo xtask check --profile portable`（`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=4`）：**EXIT=0**，75 个测试目标（58 `Running` + 17 `Doc-tests`；较 W27 的 74 多 1，即本轮新增的 `tests/mixing.rs`），0 失败。
- `cargo test -p nai-media-core --test mixing`：**18 passed / 0 failed**。
- `cargo clippy -p nai-media-core --all-targets --locked -- -D warnings`：通过（零告警）；`cargo fmt --all` 已跑。
- `cargo xtask check-deps`：18 包，依赖方向不变（`nai-media-core` 的 layer 为 `media`，新增模块未引入任何新依赖，仍只依赖 contracts/prost/sha2，仍无 HTTP 客户端）。
- `cargo xtask check --registry`：通过（B05 证据与状态一致）。
- `openspec validate w28-talent-takeover --strict --no-interactive`：`Change 'w28-talent-takeover' is valid`。

> 注：本轮门控先遇到一次宿主的 LNK1104（Windows 链接器无法打开 `debug/deps/` 下**新生成**的 `nai_avatar_2d-*.exe`，与代码无关），且首次启动时编译输入仍在变动。处置同既有惯例：预链重试循环收敛（第 2 次 `rc=0`）后再跑完整门控，一次通过。该现象属宿主事实，不写进 portable 检查。

## 未覆盖边界

- **真实声卡与真实回声环境未接入**：不打开设备、不采 PCM、不算回声抑制系数，`EchoState` 由调用方驱动。
- **keeper 进程内装配未做**：把 `AudioMixer` / `CaptureRegistry` 接进真实输出通路属 runtime 接线。
- **不引入第二条恢复路径**：没有定时器或事件钩子能通向 `resume_auto`。
- **TTL 是常量 5 分钟**：长句间隔超过 TTL 需重测回声，这是刻意取严。
