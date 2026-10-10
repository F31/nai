# ADR 0023：文本→语音→2D→播放完成的演练链路边界——接缝在 provider 层、播放语义在 media 层、编排留在 app 层

- 日期：2026-10-10
- 状态：Accepted
- 基线：V3.6 §1.4（Slice-0）、§21.3/§27（媒体时轴与 ACK）、§17.2；承接 ADR 0013（媒体时轴）、ADR 0022（编排边界）、ADR 0019/0020（装配根与引擎）
- 变更：`openspec/changes/engine-text-to-playback-ack/`

## 背景

ADR 0022 把 Slice-0 的编排落在 `apps/engine`，但链路**止于文本**：没有 TTS 段、没有像素、没有播放完成
判据。`media-basics-timeline` 主规格早已要求「仅 `committed && actual_local_playback_completed &&
played_sample_count = 样本总数` 才算 Completed」，`nai-media-core` 也已有 `AckTracker`/`UtteranceTimeline`/
`nai-avatar-2d` 的真实像素生产能力，却**只有单元测试**：没有任何一处把它们串成真实进程链路。
结果是「提交被误当完成」「帧总数靠回读自证」这类错误无法被工程判据拦住。

## 决策

**不新建核心 crate**；三段职责分别落在既有层，编排仍在 app 层。

1. **语音接缝在 provider 层**（`nai-provider-core::speech`）。理由与 ADR 0022 的文本接缝相同：TTS 是
   provider 职责，`media` 层按 `check-deps` **禁止**依赖 provider，因此接缝若放在 media 层会制造
   反向依赖或迫使 app 层自造第二套预算/账本。接缝与 `TextGenerator` 同纪律（reserve→dispatch→
   usage_charge→finish），既有 HTTP 适配器行为不变。
2. **播放语义在 media 层**（`nai-media-core::playback`）。`PlaybackLane` 复用 `UtteranceTimeline`（双水位）
   与 `AckTracker`（完成判定），把「提交」与「播放完成」在同一句上分开记账。帧边界用与软件渲染器
   **相同的整数算术**，所以声明的帧总数与产生的帧数是**构造一致**，而不是运行后回读。
3. **编排在 app 层**（`apps/engine/src/pipeline.rs`）。可选 `render` 段由可信 owner 显式写出完整闭包
   （manifest/config/三条资产/许可/voice/输出身份/播放场景），相对路径按脚本目录解析；缺失即拒绝，
   **不代填默认资产、voice 或输出身份**。
4. **预期样本总数必须可独立算出**。`RehearsalSpeechSynthesizer::sample_count(text, rate)` 只由文本与
   采样率决定（`clamp(字符数×60ms, 400ms, 5000ms)`），因此「完整播放」的 ACK 能对着一个**先于音频**
   就已知的数去核验。这是本轮判据成立的关键：如果样本总数只能在合成后回读，「完成」就是自证。
5. **守门改写过候选文本时必须停声**。四阶段守门会清空被改写候选的 `candidate_hash`（守门不得签发
   未审批对象）。因此编排在发声前比对「当前候选哈希 == 已审批对象哈希」，不等则记 `guard_superseded`
   并停止——被改写的内容**不发声**。
6. **帧只存在于内存**。`delivery = "memory"`：不写帧槽位、不经 keeper、不涉设备。演练播放槽是软件
   替身（`OUTPUT_KIND = "rehearsal-sink"`），它不产生任何输出、不证明硬件行为。

## 诚实边界

- 只有「已提交 + `actual_local_playback_completed` + `played_sample_count == 预期样本总数`」的 ACK 才记
  `DecisionKind::PlaybackAck`；`partial`/`invalid`/`cancelled` 三个**正常结局**（非故障注入）分别记
  `playback_incomplete`/`playback_invalid`/`playback_cancelled`，kind 保持 `Synthesis`（文本与音频确已
  生成、但未播放），且**不**声称观众已听到。
- 摘要恒标 `devices: false`、`output_available: false`、`real_tts: false`、`frame_slot: false`、
  `keeper_consume: false`。摘要的 `playback_ack` 只表示「每个产出文本的事件都走到了经核验的播放
  完成」，**不是**设备/硬件认证。
- `--inject speech` 需要脚本给出 `render` 段：无法生效的注入被拒绝，而不是静默忽略。语音阶段失败走
  既有固定顺序「P0 静音 → 撤权（epoch 递增）→ critical 日志 → 有界退出」，不产生成功结果行。
- 帧总数上界 `MAX_FRAMES = 4096`；更长的一句话必须由调用方分块，不得声明成无界区间。

## 后果

- `media-basics-timeline` 的提交/完成规则第一次取得**真实进程链路判据**；`runtime-composition` 的
  「手工事件 → 决策日志」链路延伸到播放完成，且 `render` 缺失时链路行为**不变**（既有归档测试逐条
  保持通过，`DecisionKind::Synthesis` 仍是文本链路的 kind）。
- 依赖方向不变：`check-deps` 通过（17 包）；`apps/engine` 新增 `nai-plugin-sdk` 与 `nai-avatar-2d`
  两个 app→plugin 依赖，符合「plugin 不得依赖 plugin、kernel 不得依赖 runtime」的既有规则。
- 残余缺口（不得掩盖）：真实 TTS 与真实 Provider 调用、真实设备/窗口/音频输出与**设备** PlaybackAck、
  帧槽位/keeper 生产接线、Windows 平台认证、protected IPC/信号停机。`nai-media-core` 的渲染帧仍只在
  内存，软件渲染不替代 Windows 帧率与设备认证。
