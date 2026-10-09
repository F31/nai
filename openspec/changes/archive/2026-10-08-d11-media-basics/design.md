# D11 媒体基础与样本时轴设计

## 位置与边界

新增 `nai-media-core/src/timeline/`（media 层，仅依赖 contracts，纯逻辑无设备/网络）：
`audio.rs` 音频块与 seq 校验、`clock.rs` 单调媒体时钟、`ring.rs` 有界槽位 PCM 缓冲、
`resample.rs` 有理样本映射、`timeline.rs` utterance 时轴与水位提交、`record.rs` 录制 sink 与 PTS、
`ack.rs` 播放 ACK 完成判定。Windows 独立窗口/音轨设备接入与宿主另属 D11 设备任务，如实 Blocked。

## audio

`AudioBlock`（session/utterance/epoch、seq、sample_rate、channels、format i16/f32、sample_start、
sample_count、end_of_stream）。`validate(&self, prev)` 校验：采样率/声道/格式有界且同句不变、
seq 单调 +1、count>0、不越界；end_of_stream 后禁止再写。时长 = sample_count / sample_rate 的有理表达。

## clock

`MediaClock` 由单调起点与 sample_rate 驱动：`tick(now)/probe(now)` 给出 sample_position（只增）。
`MediaClockState::insert_silence(ms)` / `advance(samples)` 推进位置，绝不停顿回退；有理 `pts(tb_num,tb_den)`。
提供 `map_cross_rate(model_rate, model_pos, output_rate)` 的精确比例映射，禁止直接比较异率 sample_start。

## ring

`Slot` 有界环形缓冲（max_slots、max_slot_bytes）：`push` 超限返回 `Capacity`，不静默丢弃、
不截断改写；`pop` 按先入先出；支持水位查询 `ready_bytes`。PCM 为原始字节 + 确定性元数据帧。

## timeline

`UtteranceTimeline`：累计 audio blocks 与 video frames；对每 utterance 校验跨率映射连续性；
`commit(audio_ready, video_ready)` 仅当获准首段音频与首批视频均达水位才返回 Commit；持续迟到
（超过 `persistent_late_ms`）返回 `Late`，由调用方取消当前句并静音/降级，不长时间有声无嘴动。

## record

`RecordingSink` trait：`push_audio(block, position)` 以单调媒体 clock 样本位置录制，PTS 单调；
句间时间间隔写确定静音补齐；测试实现为内存原始样本序列。录制默认不自动上传，
文件录制不能证明已连 Windows 设备。

## ack

`AckTracker`：对每个 utterance 记录 expected_samples、committed 与 completed 状态；
`acknowledge(ack)` 仅在 committed && actual_local_playback_completed && played == expected 时置 Completed；
不完整/部分 ACK 保持进行中，跨故障未知状态返回取消以规避重复播报。补充覆盖实际设备完成前的全部中介状态。

## 验证范围

单元测试覆盖：seq/格式变化拒绝、end_of_stream、时钟单调与静音、跨率映射、缓冲容量与水位、录制 PTS/静音补齐、ACK 完成判定；
集成测试模拟 16k TTS → 48k 输出时轴 → 水位 commit → 实际 ACK → 录制连续流。运行 portable/契约/前端检查。
Windows 设备窗口/音轨、虚拟摄像头/音频设备、录制文件与传播链路不在本轮伪造。

## 2026-10-08 实施结果

`nai-media-core::timeline`（audio/clock/ring/resample/record/ack + utterance 时轴）已实现，纯逻辑仅依赖 contracts。
audio 校验 seq 单调/格式不变/end_of_stream 终块；clock 位置只增、静音连续、跨率精确比例映射；ring 超限返回 Capacity 不静默丢弃；
timeline 音频+视频双水位才 commit，持续迟到返回 Late 由调用方取消/降级；recorder 按媒体时钟位置原子补静音、PTS 单调；
ack 仅在 committed+actual_local_playback_completed+样本数匹配时标 Completed，部分 ACK 保持 Playing。
新增 media-core 单元 20 项（timeline）+ 集成 1 项；kernel 42 项保持不变。全 workspace 199 项 Rust 测试通过。
Windows 独立窗口/音轨、虚拟摄像头/音频设备与录制文件装配仍是设备里程碑，如实 Blocked。