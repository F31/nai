## 1. 立项与骨架

- [x] 1.1 OpenSpec 变更 `w28-talent-takeover`（proposal/design/tasks + `talent-takeover` 增量），`validate --strict` 通过。
- [x] 1.2 落点确认：放进 `crates/nai-media-core`（media 层，`xtask/src/deps.rs` 只禁 provider 与 `plugins/llm-*|tts-*|gateway-*` 与 HTTP 客户端），不放进 kernel（kernel 不许依赖 media，且这是声音通路语义），不新开 crate——18 包与依赖方向不变；自带 `MixError{code, reason}`（稳定 token，与 `UpgradeError`/`MultiplexError` 同形）。

## 2. 接管与两轨互斥（B05）

- [x] 2.1 `Gain`（0..=1000 定点，`MUTE`/`FULL`，越界即 `INVALID_ARGUMENT`）、`MixMode`（`Auto | Takeover`）、`RenderLevels` 与 `any_audible`。
- [x] 2.2 `AudioMixer::new/Default`：自动模式、TTS 全量、真人轨静音、epoch 0。
- [x] 2.3 `render()` 按 `mode` **推导**静音轨——`Auto` 只出 TTS，`Takeover` 只出真人；「两轨同时可听」没有构造点（决策 2）。
- [x] 2.4 `enqueue_auto(count)`：仅自动模式（否则 `CONFLICT`）；零量 `INVALID_ARGUMENT`；上限 `MAX_AUTO_QUEUE = 256` 溢出 `RESOURCE_EXHAUSTED`。
- [x] 2.5 `takeover(request)`：已是接管即 `CONFLICT`；空事实版本 `INVALID_ARGUMENT`；回声不新鲜即拒；效果为暂停自动 + TTS 静音 + 清队列 + 接入真人轨，并返回 `TakeoverReport`（含实际清空条数）。推 epoch、累计 `cleared_total`。
- [x] 2.6 `resume_auto(proof, now)`：非接管即 `CONFLICT`；授权、事实版本（与接管快照一致且非空）、值守租约（未到期）三件任一不满足即 `PERMISSION_DENIED`；成功则回到自动并恢复 TTS、静音真人轨。
- [x] 2.7 `on_talent_silence(&self) -> MixMode`：**只读**，返回当前模式；真人讲话结束没有通向恢复的代码路径（决策 5）。

## 3. 唯一收音路径与设备标识（B05）

- [x] 3.1 `DeviceId`（稳定、不透明、校验非空与长度）、`CapturePath{owner, device}`、`CaptureLease{path, generation}`。
- [x] 3.2 `CaptureRegistry::acquire`：已有活跃路径即 `CONFLICT`（**不看 owner/device**）；代次自增；`release` 只接受当前代次，旧代次 `STALE_EPOCH`，无活跃路径 `CONFLICT`；`live`/`live_count` 查询。
- [x] 3.3 `observe_default(device)` → `First | Unchanged | Changed{from,to}`，并记住新值——变化被**报告**而非静默跟随（决策 3）。

## 4. 回声测试前置与审计（B05）

- [x] 4.1 `EchoState`（`Unrun | Running | Passed{at_utc_ms} | Failed`）与 `valid_at`：仅 `Passed` 且未超 `ECHO_TEST_TTL_MS`（5 分钟）且非未来时刻为有效。
- [x] 4.2 `takeover` 的回声门控：未跑/运行中/失败 → `CAPABILITY_NOT_READY`；过期 → `EVIDENCE_EXPIRED`；恰好 TTL 边界接受。
- [x] 4.3 `MixSnapshot`（普通值快照）与 `audit_mix`：检出 `both_tracks_audible`、`takeover_with_tts_audible`、`auto_with_talent_audible`、`takeover_without_capture_path`、`takeover_without_valid_echo_test`、`multiple_live_capture_paths`，报出**全部**且顺序固定（决策 6）。
- [x] 4.4 测试 `tests/mixing.rs`（18 项）：初始自动态、两轨不可同时可听、回声三种未通过状态与 TTL 边界与未来时刻、接管四件副作用与报告、重复接管冲突、空事实版本被拒、恢复三项复核与各自失败、已自动时恢复冲突、讲话结束不恢复、恢复后两轨复位、唯一路径与代次租约、迟到释放被拒、默认设备观察三态、标识与增益校验、队列有界且仅自动、干净快照审计无违规、每条违规的稳定标记与全触发顺序。

## 5. 规格、文档与提交

- [x] 5.1 新增 `docs/mixing.md`（接管四件副作用、两轨互斥的推导、唯一收音路径与代次租约、稳定设备标识与默认变化观察、回声测试 TTL、恢复三项复核、审计标记、与 `timeline/audio.rs` 的职责区分、未做边界）。
- [x] 5.2 新增 ADR 0030，记录落进 media 层而非 kernel/新 crate 的理由，并记录否决方案（两个增益 setter / `capture_active: bool` / 自动回声测试 / 讲话结束定时恢复 / 审计读内部状态）。
- [x] 5.3 报告 `docs/reports/2026-10-10-w28-talent-takeover.md`：实测数字、逐项验收映射（B05）、未覆盖边界与环境边界。
- [x] 5.4 `tests/registry.yaml`：B05 补证据并按实现 `pending → implemented`（**不标 `tested`**——真实声卡、真实 OBS/伴侣与真实回声环境中未接入）；`docs/development-plan.md` 与 `AGENTS.md` 记录本轮。`check --registry` 通过。
- [x] 5.5 归档 `w28-talent-takeover`（归档名 `2026-10-10-w28-talent-takeover`，新 capability `talent-takeover` 生效），跑 `validate --all --strict`（含 `--archived`）。
- [x] 5.6 全量 `cargo xtask check --profile portable` 通过并留档；提交 + 推送。

## 里程碑切分

本轮为单个可独立验证的切片：接管与两轨互斥（§2）、唯一收音路径（§3）、回声测试与审计（§4）共用同一个 `mixing.rs` 与同一个 capability——分开提交会让首次提交的测试目标不完整（例如「唯一路径」若无「接管」则无法验证接管前置）。文档、ADR、registry、归档与门控（§5）为纯声明变更，沿用同一次门控留档。
