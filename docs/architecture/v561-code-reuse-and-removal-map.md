# V5.6.1 代码复用与删除映射

日期：2026-11-04。依据：ADR 0193、`openspec/changes/v561-flashhead-single-engine/`、`docs/reports/2026-11-04-v56-review-and-recommendations.md`。

## 原则

- 复用：与 V5.6.1 单引擎、Qwen3-TTS、预录片段、OBS 输出直接相关的 engine/UI/SQLite/输出门/安全帧。
- 替换：视觉 worker 从 MuseTalk+LivePortrait/双引擎切换改为 FlashHead 单 worker。
- 保留但不进默认构建：仍有未提交在研代码、但被旧 xtask/contracts 引用的旧线。
- 删除：低耦合、无活动引用、无未提交在研价值的旧模块。
- 不向前兼容：旧数据库/配置/协议/ACC/INV 不导入 V5.6.1。

## 当前保留（V5.6.1 活跃线）

| 资产 | 处置 | 说明 |
|---|---|---|
| `apps/workbench-engine` | 保留 | 会话/HTTP/组合根；后续接 Qwen/FlashHead 适配 |
| `apps/workbench-desktop` | 保留 | Tauri 2 + React 工作台壳 |
| `apps/public-output-keeper` | 保留 | 输出门/设备时钟/安全帧基础 |
| `apps/output-keeper` | 保留 | 本地音频/播放在研代码；不进 V5.6.1 默认 UI |
| `apps/audition-worker` | 保留（可选） | 未跟踪在研；不阻塞主线 |
| `apps/human-mic-worker` | 保留（可选） | 未跟踪在研；不阻塞主线 |
| `adapters/storage-sqlite` | 保留 | 新 V5.6.1 最小 schema 落点 |
| `adapters/llm-openai` | 保留 | LLM/TTS 适配基础 |
| `adapters/audio-cpal` | 保留 | 音频设备/时钟 |
| `crates/nai-contracts` | 保留 | 只保留 V5.6.1 需要的消息 |
| `crates/nai-runtime-core` | 保留 | 会话/执行基础 |
| `crates/nai-media-core` | 保留 | 输出门/时钟/安全帧/共享内存 |
| `crates/nai-avatar-files` | 保留 | 肖像/资产导入与哈希 |
| `crates/nai-workbench-host` | 保留 | 工作台宿主/IPC |
| `crates/nai-live` | 保留 | 直播会话基础 |
| `crates/nai-kernel` | 保留 | 必要守卫；不继承 62 INV |
| `crates/nai-plugin-sdk` | 保留 | 当前 UI/2D 降级仍依赖 |
| `plugins/avatar-2d` | 保留 | 降级/示例形象 |
| `xtask` | 保留 | 构建/依赖/契约工具；旧 suite 后续收敛 |

## 已删除（第一批）

| 资产 | 状态 | 原因 |
|---|---|---|
| `apps/render-worker` | 已删除 | 旧渲染入口；V5.6.1 由 avatar worker 替代 |
| `plugins/platform-unavailable` | 已删除 | 平台 connector 移出主线 |
| `crates/nai-plugin-hosts` | 已删除 | 无活动消费者；插件宿主不在 V5.6.1 |
| `crates/nai-runtime` | 已删除 | 旧装配/预算/升级树；仅旧 engine 使用 |

## 已删除（第二批）

| 资产 | 状态 | 原因 |
|---|---|---|
| `apps/engine` | 已删除 | V5.6.1 使用 workbench-engine |
| `crates/nai-budget-core` | 已删除 | 预算/审批旧线 |
| `crates/nai-provider-core` | 已删除 | 旧 provider 抽象 |
| `crates/nai-security` | 已删除 | 旧安全/凭据抽象 |
| `crates/nai-knowledge-core` | 已删除 | 旧知识/RAG 线 |
| `crates/nai-storage` | 已删除 | 旧 workspace/role/approval/ledger 存储 |
| `crates/nai-upgrade-core` | 已删除 | 旧升级线 |
| `crates/nai-control-api` | 已删除 | 旧控制 API |
| `crates/nai-control-ipc` | 已删除 | 旧控制 IPC |

workspace 从 26 个包降到 18 个包；`cargo xtask check-deps` 通过。

## 剩余旧代码（下一步）

1. `adapters/storage-sqlite` 仍保留 V4.8 workspace/role/approval/budget 模块与测试；V5.6.1 新 `v561` 模块已加入，但旧 `Store` 仍被 workbench-engine 使用，需按实际消费者收敛。
2. `scripts/contracts/*` 仍有 V4.8 锚点/registry 脚本；不在 V5.6.1 `workbench` 门控中运行，后续清理或归档。
3. `apps/audition-worker`、`apps/human-mic-worker` 为未跟踪可选 worker，暂保留；确认无关后删除。
4. 旧 `tests/registry.yaml` 保留只读归档，不参与 V5.6.1 门控。
5. 每步运行 `cargo check --locked`、`cargo xtask check-deps`、`cargo xtask check --profile workbench`、`node scripts/check-v561-registry.mjs` 并记录报告。

## V5.6.1 新增/替换

| 目标 | 说明 |
|---|---|
| `nai-avatar-worker`（新） | Python/PyTorch，FlashHead Lite；命名管道 + 共享内存 |
| Qwen3-TTS 适配 | 1.7B CustomVoice 默认；24k WAV 非流式；句级预取/缓存 |
| 预录动作片段库 | `assets/clips/` + 元数据/哈希/许可 |
| 本机预览/OBS | 复用输出门，新增 720p/25fps 预览 |
| 单数据根/最小 schema | `config.json` + `nai.db` + 10 表 |
| V5.6.1 ACC 注册表 | `tests/v561/registry.yaml` + 检查器 |

## 禁止

- 不把 LiveAct 18B、双引擎切换、热备、过渡切换重新引入主线。
- 不把旧 164 ACC/62 INV 作为 V5.6.1 通过依据。
- 不删除用户资产/许可记录；旧资产只手动导入。
- 不在默认构建中重新加入旧 legacy crates，除非新增 ADR/规格。


## 2026-11-04 第三批：退役 MuseTalk 实验

- 删除 MuseTalk 专用脚本、测试与 `research_clock_bridge` 示例及专用 Cargo 入口；删除独立 MuseTalk 源码/权重与旧实验模型副本。
- `clocked-experiment` 音频适配器及公共输出仍有实际消费者，暂不能按旧名称直接删除；需 V5.6.1 输出替换后再移除或重构。
- 工作台 engine 的 `foundation` 和 SQLite `Store` 仍直接依赖旧工作区/预算/审批模块；这是未完成的架构缺口，须让 V5.6.1 新核心先接替调用后删除，并同步替换 UI 及契约，不能视为已完成。
- 保留研究报告、用户视频/语音、凭据、数据和可能复用的 PyTorch 工具链；不迁移旧数据库或旧许可。

## 2026-11-28 第四批：`nai-kernel` 收缩为「必要守卫」

判据：能力目录已冻结为 3 个 `workbench.*`（`workbench.foundation/inspect/prepare`），V4.8 判定面的每一件被审对象都已不可请求；再叠加「排除 tests 后是否还有生产者」逐模块核实。全部文件删除前均在 `HEAD` 内，可按提交回滚。

### 删除

| 资产 | 规模 | 消费者 | 原因 |
|---|---|---|---|
| `src/evaluate/`（agent/guard/harness/memory/ramp/selling/show/skill/assembly） | 4,879 行 | 仅 `assemble` | 62 INV 判定面；对象为 `agent.decide@1`/`guard.check@1`/`knowledge.retrieve@1`/`live.program@1` 等已不在 `CAPABILITY_IDS` 的能力 |
| `src/assemble.rs` | 311 行 | 无 | 只构造 `evaluate::assembly` 的计划，判定面删后无输入 |
| `src/registry.rs` + `src/registry/tests.rs` | 1,531 行 | 无（`Registry`/`CallContext` 在真实源码域零引用） | V4.8 插件准入/绑定内核，语义含 `budget_admission_ref`、`platform.permissions`、`avatar.render→avatar.profile` 硬闸；能力名校验的唯一落点改为 `nai-plugin-sdk` 对 `CAPABILITY_IDS` 的校验 |
| `src/bridge.rs`、`src/platform.rs` | 900 行 | 仅 `evaluate` | 平台 connector / SDK 存在性桥，平台线已移出主线 |
| `src/demo.rs`、`src/device.rs`、`src/realistic.rs`、`src/multiplex.rs`、`src/scheduler.rs`、`src/monitoring.rs` | 3,171 行 | 无 | 零生产者；`realistic` 属 V4.8 写实研究线 |
| `tests/`：assemble、assembly、bridge、demo、device、evaluate、harness、memory、monitoring、multiplex、platform、ramp、realistic、scheduler、selling | 7,283 行 | — | 随被测模块一并退役 |

### 保留（V5.6.1 有真实调用方）

| 模块 | 调用方 |
|---|---|
| `output_owner` | `workbench-engine`（v561_tasks/audio/stream_audio/audition_control）、`public-output-keeper`、`llm-openai` 测试 |
| `obs`、`obs_dest`、`obs_observe` | `workbench-desktop`（窗口观测/目标） |
| `golden`、`metrics`、`review` | `cargo xtask check --golden`（依赖 `golden` 的 `Finding`；`metrics`/`review` 为其比例/延迟样本） |
| `session`、`duty` | `crates/nai-media-core/tests/gate.rs`（会话生命周期与指令账本） |

### 验证

`cargo check --workspace --all-targets --locked` 通过（6.7 s）；`cargo test -p nai-kernel` 105 用例全绿（原 16 红）；`cargo test -p nai-media-core` 全绿；`cargo xtask check-deps` 18 包边界通过。

### 未做与本批相关但未决的

- `xtask check --golden` 与其 `tests/rag-golden`（仅剩 README，无用例）：知识/RAG 线已删（`nai-knowledge-core`），该工具目前没有负载，待定「保留为格式校验」或「随线下线」。
- `docs/runtime.md`、`docs/scheduler.md`、`docs/storage.md` 整体描述已删的 `nai-runtime`/`apps/engine`，并引用已退役的 `--profile portable` 与不存在的 `test --suite scheduler`；需整体归档而非逐行订正。

