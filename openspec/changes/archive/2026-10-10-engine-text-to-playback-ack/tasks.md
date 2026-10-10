# Tasks

## 1. 规格与校验
- [x] 1.1 proposal/design/tasks/specs 完成并通过 `openspec validate engine-text-to-playback-ack --strict --no-interactive`。

## 2. Provider 语音接缝与桩
- [x] 2.1 `nai-provider-core` 新增 `SpeechSynthesizer` 接缝：请求含 session/utterance/epoch/文本/采样率/预算/取消；响应含**有界** PCM、采样率、usage、模型标识；预算/取消/usage 与 `TextGenerator` 同纪律，不改变既有 HTTP 适配器行为。
- [x] 2.2 确定性 rehearsal-only 语音桩：样本数只由文本与采样率决定（有上下界）、可复现、无网络无凭据。
- [x] 2.3 单元测试：样本数可预算、桩可复现、非有限/越界 PCM 被拒、取消与期限不进入合成器、已派发不得重发。

## 3. media-core 播放通道
- [x] 3.1 新增可移植播放通道：以 `UtteranceTimeline` 双水位提交、以 `AckTracker` 完成判定；提交与完成语义分离。
- [x] 3.2 ack 绑定 `(output_id, utterance_id)` 与预期样本总数：未提交保持 Pending、样本不足不完成、样本数不符被拒、已取消不再被完成。
- [x] 3.3 文档化的演练场景（complete/partial/invalid/cancelled）与单元测试覆盖（含音频水位满足但视频未达水位不得提交）。

## 4. engine 链路延伸
- [x] 4.1 pipeline 新增可选 `render` 段（manifest/config/assets/licenses/output_id/playback），路径相对脚本解析；缺失即拒绝，不猜默认。
- [x] 4.2 文本生成后经语音接缝合成 PCM，按样本窗口切有界 `AudioBlock` 流；每次对外步骤前复检 session/epoch/值守与候选哈希。
- [x] 4.3 以已审批候选对象驱动 2D 渲染核心产出真实 RGBA 帧并计视频水位；双水位提交后由演练播放槽发出 ACK。
- [x] 4.4 仅完整播放完成 ACK 落 `DecisionKind::PlaybackAck`；partial/invalid/cancelled 落 `Synthesis`（文本与音频确已生成、但未播放）且摘要如实标注 `playback_ack=false`。实施时把手写草稿里的 `Observation` 改为 `Synthesis`：该 kind 更准确，且是已归档文本链路（`stage=generated`）既有行为，改回 `Observation` 会无声改变上一轮已归档判据。
- [x] 4.5 `--inject speech` 与 USAGE/参数校验；失败走「先 P0 静音 → 撤权 → critical 日志 → 有界退出」，不产生成功结果行。
- [x] 4.6 摘要补齐真实字段（语音模型标识、采样数、帧数、播放状态、声明的输出身份），并保持无设备/无槽位/无 keeper 的如实声明。

## 5. 交付
- [x] 5.1 集成测试（真实 engine 进程）：完整完成落 `PlaybackAck` 且哈希链校验与两遍回放通过；partial/invalid/cancelled 不落 `PlaybackAck`；`--inject speech` 有序停机。
- [x] 5.2 ADR 0023、docs/engine.md、docs/media.md、docs/provider.md、tests/registry.yaml（R07/A01 备注，状态按判据判定）、docs/development-plan.md、AGENTS.md。
- [x] 5.3 匹配检查：workspace 测试、fmt/clippy、check-deps、portable、registry；schema/contracts 若因本机平台限制不可执行须如实记录。**`cargo xtask check --profile portable` 本机无法整体通过**（两个独立原因，详见报告），等价门控已逐条单独执行并记录。
- [x] 5.4 报告 docs/reports/2026-10-10-engine-text-to-playback-ack.md，归档并 `validate --archived`。
