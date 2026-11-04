# Design

## Context

审批后候选文本保存在 `utterances.display_text`，`approval_valid` 已复核 epoch、TTL、证据与审批权限。Provider 的 TTS 适配器已流式读取音频并执行 reserve/dispatch/finish，但只把字节计数返回，未把 PCM 交给媒体层。2D 渲染与双水位提交已在 engine 的演练路径存在。

## Goals / Non-Goals

交付：已批准候选 → 真实 TTS → 有界 PCM → 真实 RGBA 帧 → 本地 WAV/帧摘要。暂不播放、不接设备、不产生 PlaybackAck，也不声称真实账户或浏览器验收。

## Decisions

1. 预览是显式操作者动作，绑定候选哈希。控制命令复用 `SessionCommand` 的 epoch/revision/seq/TTL 与幂等账本；仅 `approval.write` 权限，因为动作对象是已批准候选，不扩张 operator 权限面。
2. 每次预览前调用 `read_approval` 与 `approval_valid`，要求候选仍有效；读取 `display_text` 作为**唯一** TTS 文本，不复用请求方文本。合成请求的 `approved_segment_hash` 取候选哈希。
3. Provider 适配器新增返回音频字节的方法；现有只计数的 `tts_stream` 保持签名。两种协议共用各自私有核心，避免两份 HTTP/预算逻辑。字节上限沿用现有 64 MiB 传输界，engine 在解码前按 5s/48kHz 再拒绝过大音频。
4. engine 复用既有 `SpeechRequest`、`PlaybackLane`、`frame_boundary`、`frame_count` 与 Avatar 流；把演练路径的帧循环抽成共享函数，避免第二条帧算术。帧在内存中产生并计 SHA-256，不写帧槽、不经 keeper。
5. 预览产物写入 `<data-root>/operator-artifacts/`：`<hash>-<epoch>.wav` 与 `.frames.json`（尺寸、帧数、每帧哈希、样本数）。文件名只由候选哈希/epoch 构造，不含文本。写入有界并原子完成：先写临时文件再 rename。
6. 任务容量 1，状态历史 100。静音/接管通过共享 Session 的 epoch 与停机标志使在途任务失效；TTS 等待期间仍不持有 Composition/Store 锁。失败不删除已发生计费的账本条目（reserve/dispatch/finish 语义不变）。
7. 桌面只在批准成功后提供预览；轮询状态，显示失败码或真实样本/帧/产物引用。预览成功不是播放完成，界面必须明说。

## Risks / Trade-offs

本地 WAV 可能被误读为「已播出」；界面与状态字段显式标注 `playback_ack=false`。TTS 字节在适配器内累积（既有 64 MiB 界），本轮不再为早停改造流式协议。帧只写摘要，不提供浏览器帧显示（媒体不经控制总线）。

## Migration

无数据库迁移；新增可选 `render` 场景段，旧 operator 配置仍可生成候选，只是预览返回 `CAPABILITY_NOT_READY`。

## Verification

进程级测试用本地 HTTP 协议测试服务返回真实 s16 PCM，验证：批准前不可预览、批准后可预览、TTS 请求携带批准文本与哈希、WAV/帧摘要字节与哈希、静音取消、过期/换代拒绝、重复命令不重复计费。另测桌面预览状态与迟到响应抑制。
