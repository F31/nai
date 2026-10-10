## Decisions
续接 ADR 0022 的编排位置：本轮仍不新建核心 crate。语音接缝放 `nai-provider-core`（provider 层，与 `text` 接缝同址），播放完成语义放 `nai-media-core`（media 层，与 `timeline::ack` 同址），链路编排留 `apps/engine`（app 层）。
语音接缝：`SpeechSynthesizer` 与 `TextGenerator` 同纪律——请求携带 session/utterance/epoch/文本/采样率/预算/取消，响应携带**有界** PCM、采样率、usage 与模型标识；接线经 `synthesize_with_budget`，先 reserve 再 dispatch 再 finish，已派发的请求不得重发，usage 走既有 `usage_charge`（单位限既有词表，语音用 `audio_seconds`）。
预期样本总数可预先算出：确定性语音桩的样本数只由 `文本字符数` 与 `采样率` 决定（有上下界），因此「完整播放 ACK」要匹配的样本总数是一个**可核验的已知量**，而不是运行期观测值——这是本链路能否诚实判定完成的前提。
2D 帧：复用 `nai-avatar-2d`（`apps/render-worker` 已装配的同一核心），以**已审批的候选对象**（`PerformanceCue`）驱动 `stream`，把合成 PCM 按 `采样率 × fps_den / fps_num` 的样本窗口切成有界 `AudioBlock` 流，每块推入后按块产出帧并计视频水位。像素真实，但只存在于内存：**不写帧槽位、不经 keeper、不声称设备**；生产帧路径仍归 `apps/render-worker` + 帧槽位 + keeper。理由：帧槽位数据面在本机（Windows）是 `CapabilityNotReady`，把渲染核心嵌入演练是本仓库既有「演练显式、边界诚实」模式的自然延续（对照 `--media-drill`）。
完成判定：只有同一 `AckTracker` 的 `Committed` + `actual_local_playback_completed` + `played_sample_count == 预期样本总数` 才落 `DecisionKind::PlaybackAck`；只确认提交、样本数不足、身份不符、已取消一律不落该 kind，改落 `Observation` 并在 payload/摘要中如实标注 `playback` 状态与 `playback_ack=false`。提交（水位）与完成（ACK）在类型与日志上都分离，杜绝「提交即完成」。
演练场景由脚本显式声明（`render.playback` ∈ `complete|partial|invalid|cancelled`），与故障注入分开：注入（`--inject speech`）表示**阶段失败**并触发有序停机，场景表示**正常但未完成**的播放结局，二者不得混用。
`render` 段为可选：缺失时链路行为与上一轮完全一致（止于文本、`delivery=none`、`playback_ack=false`），这保证既有脚本与既有规格增量不被破坏；给出时必须是完整闭包（manifest/config/三份资产/许可/output_id），缺一即拒绝，不猜默认。
两遍回放（R07）继续成立：`EventOutcome::canonical()` 只纳入确定性字段（阶段、样本数、帧数、播放状态、生成文本、usage），**不纳入**像素与 PCM 字节，故 pass 2 重算结果与落库条目逐项一致。
诚实边界：无真实 TTS、无帧槽位、无 keeper 消费、无设备输出、无 Windows 认证；文档与报告必须与摘要字段同口径声明。
## Verification
真实 engine 进程（Windows 与本机工具链）：带 `render` 段的已审批手工事件走完「文本 → 语音 → 音频块流 → 真实帧 → 双水位提交 → 完整播放 ACK」，落 `PlaybackAck` 决策条目，哈希链校验通过、两遍回放逐项一致；`partial`/`invalid`/`cancelled` 场景均**不落** `PlaybackAck` 且摘要如实标注未完成；`--inject speech` 触发有序停机且无成功结果行。命令：workspace 测试、`fmt`、`clippy`、`check-deps`、`check --profile portable`、`check --registry`、`schema --check`、`test --suite contracts`。
