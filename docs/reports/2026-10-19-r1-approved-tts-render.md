# 已批准候选的 TTS 与 2D 本机预览

日期：2026-10-19。变更：`openspec/changes/r1-approved-tts-render/`。

## 本轮交付的用户路径

操作者批准候选后，可在桌面点击「生成语音与 2D 预览（不播放）」：

1. 引擎读取该候选的原始 `PerformanceCue`（迁移 0013 新增的 `utterances.cue_blob`），重算哈希并
   要求等于候选身份；`approval_valid` 复核审批、epoch、TTL、证据与权限。
2. 用**同一份 cue 的 `speech_text`** 调用配置的 API TTS，预算走持久化
   `reserve → dispatch → finish`；请求带 `approved_segment_hash`。
3. 将返回的 s16 PCM 解码为有界 f32（≤ 5s/48kHz），用现有 2D 渲染与双水位提交产生真实 RGBA 帧。
4. 写出 `operator-artifacts/<hash>-<epoch>.wav` 与 `.frames.json`（尺寸、帧数、每帧 SHA-256、
   音频 SHA-256、`playback_ack=false`），桌面显示真实样本数、帧数、产物引用与音频摘要。

**这不是播放**：没有设备输出、没有 PlaybackAck、没有帧槽或 keeper 消费。操作者需要自行打开 WAV
试听；帧只以摘要形式提供，尚无窗口显示。

## 关键实现

- `migrations/sqlite/0013-utterance-cue.sql`：`utterances.cue_blob`（可空）。
  `record_candidate` 写入校验后的 cue，`read_candidate_cue` 读回并重算 `candidate_hash`。
  这闭合了 ADR 0045 留下的残余风险——此前只有 `display_text`，无法证明待合成文本就是被哈希批准的
  那个对象；旧行没有 cue 时返回 `CAPABILITY_NOT_READY`，不用文本猜一个 cue。
- `nai-provider-core`：OpenAI/ElevenLabs 适配器新增返回音频字节的 `tts_pcm`，与既有 `tts_stream`
  共用各自私有的 HTTP/预算核心；现有只计数路径签名不变。`SpeechClient::tts_pcm` 暴露同一预算语义。
- `apps/engine`：把演练路径的帧循环抽成 `RenderLane::render_audio`，演练与预览共用同一帧边界、帧计数、
  推送/提交与帧序检查；渲染器仍检查 cue 的 session/epoch/expiry/hash。
- `nai-control-api`：新增 `PreviewSurface`，接线
  `POST/GET /sessions/{id}/previews`；命令复用 `SessionCommand` 的 epoch/revision/seq/TTL 与幂等账本，
  权限为 `approval.write` / `session.read`。状态历史 100，任务容量 1。
- 桌面：批准成功后保留候选哈希与文本，提供显式预览按钮；轮询状态，失败/超时/换代/断连的迟到响应
  不恢复旧结果，不自动重发；成功时明确显示 `playback_ack=false`。
- `schemas/openapi/control.yaml` 与目录登记同步；`schemas/operator/manual-session.schema.json`
  增加可选 `render` 段与 `character_limit`。

## 验证

在 Linux/WSL 本机：

```bash
cargo test -p nai-engine --test engine operator_flow -- --nocapture   # 7 passed
cargo test -p nai-provider-core                                      # 8 + 10 + 8 passed
cargo test -p nai-storage --lib                                      # 96 passed
cargo test -p nai-control-api                                        # 50 passed
cargo xtask schema --check                                           # no drift
cargo xtask check --profile full                                     # EXIT=0
node scripts/frontend-gate.mjs                                       # 6 steps, UI 89/89
```

`check --profile full` 现测：**84 个 Running + 18 个 Doc-tests = 102 个测试目标，
1195 passed / 0 failed**；Node **432 项通过 / 0 失败**（UI 89 含本轮 5 项，desktop 5）。
变更归档后复跑同一命令仍为 **EXIT=0**，计数逐项相同；归档位置
`openspec/changes/archive/2026-10-19-r1-approved-tts-render/`。

新增进程级测试覆盖：

- 未批准候选预览返回 403 且不调用 TTS；批准后 TTS 请求的 `input` 恰为最终文本，`voice` 与
  `response_format=pcm` 正确。
- 渲染状态包含 4000 样本/13 帧；WAV 头为 mono/8kHz/16-bit，长度 = 44 + PCM；帧摘要 13 个哈希、
  2×1、`playback_ack=false`；决策链无 `playback_ack` 且校验通过。
- 重复相同预览命令返回原 ACK，不产生第二次 TTS；无 `render` 配置返回 503；静音后旧 epoch 预览
  不再调用 TTS。
- TTS 阻塞时静音使在途预览以 `STALE_EPOCH` 结束，释放后仍无产物。

## 边界与未做

- 仍使用**本地 HTTP 协议测试服务**，不是真实付费 TTS 账户；但请求经过真实
  `ProviderProfile → ProviderClient → 适配器 → 预算网关`，返回真实 s16 字节。
- **无设备播放、无 PlaybackAck、无帧槽/keeper 消费、无窗口显示**；WAV 是试听产物，不能被读作
  「已经播出」。Windows 音频/窗口仍为 Blocked。
- TTS 适配器沿用 64 MiB 传输界；engine 在解码前拒绝 > 240k 样本，但字节已经下载。早停流式上限
  是后续改进，不在本轮。
- 预览依赖 `render` 场景段与本机 2D 素材；没有合法素材时返回 `CAPABILITY_NOT_READY`，不填默认形象。
- `operator` 的 provider 协议仍只有一个，TTS 与 LLM 共用 OpenAI-compatible 配置；ElevenLabs 路径
  已暴露字节接口但未通过 operator 配置选择。
- 没有浏览器 E2E；UI 测试用 `fetch` 替身与纯逻辑。验收状态未提升，O02/O05 仍 pending。

## 下一步

接**输出端**：Windows 音频/窗口或 keeper 的设备适配，让批准 cue 的 PCM/帧真正输出，并让
PlaybackAck 来自输出侧；在此之前，任何「已播放/已看到」的说法都不成立。
