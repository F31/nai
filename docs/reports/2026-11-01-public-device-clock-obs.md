# 授权公共音画设备时钟与隔离 OBS

## 结果

操作员授权的本地 Qwen 单句走同一条受控 WASAPI 公共执行。时间线 16 行都带设备时钟：流代次 1，设备频率 384000 Hz，已播放样本从 203 到 35979，当前帧源样本最大 35200，不超过当时已播放样本。提交 36480，字幕出现过。执行理由是 `PUBLIC_EXECUTING`。隔离 OBS 只捕获标题为 `nai · 公共输出` 的窗口，留下 `nai-v49-device-clock-2026-11-01-23-34-42.mkv`。容器时长 59966 毫秒，画面 512×288 H.264，1799 个视频块，约 30 帧/秒，另有 48 kHz 立体声 AAC。1.3.10 整项按这次验收勾选。

正式公共画面后端选择已交付的 BuiltinAvatar / nai-media-core 内置几何 2D。依据是隔离研究三次 512 串行墙钟 47.66、47.97、48.75 毫秒/帧高于 32 毫秒、Python 不得进入 Windows 正式运行包、没有原生 ONNX、也没有已批准的外部服务适配。成本写在 [ADR 0192](../adr/0192-windows-public-visual-backend.md)。1.4 按该 ADR 勾选。该选择不是写实生产。

静音初始化曾把句级设备时钟发到宿主，而执行租约代次仍是 0，宿主报 `WB_PROTOCOL_ERROR`。keeper 在 `DEVICE_INITIALIZED_MUTED` 和 `DEVICE_LOST` 上不再发布该字段；宿主在静音初始化上拒绝出现该字段。`muted_init_rejects_a_sentence_clock_before_the_execution_lease` 通过。

`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。

## 本机 2026-11-01 授权执行

证据目录是 `E:\projects\nai\target-windows\v49-public-av-evidence-5`。授权文件 `local-qwen-av-once.json` 已消费。任务 `0160a8ab-21c6-457a-9d8b-7849c1e74be2`，工作区 `9ab262e1-9a97-4130-8571-68224e0e131e`。文本「容量为500毫升。」，模型 `qwen3-tts-1.7b-customvoice`，音色 `serena`。TTS 状态 5，`SYNTHESIZED_BILLING_UNKNOWN`，费用 unknown，`paid_api` 为 false。输出设备名 `Speaker (Realtek(R) Audio)`。驱动 pid 53484，keeper pid 57968。

时间线 `elapsed_ms` 从 59 到 1556。`window_frame` 最大 42。`source_sample_within_played` 为 true。`hardware_playback_completed` 为 unknown。驱动把 `obs_recording` 标成 `external_evidence_required`，OBS 文件由这次隔离实例另行留下。

隔离 OBS 是 E 盘便携树，pid 41284，`--portable --multi`，配置只写在 `target-windows\obs-isolated-v49`。场景来源 `nai-public-window-only`，窗口 `nai · 公共输出:minifb_window:nai-public-output-keeper.exe`，`method` 2（WGC），`capture_audio` 为 true。23:34:42.631 `GetSourceActive` 返回 `videoActive` true、`videoShowing` true。23:34:42.633 `StartRecord` 返回 code 100。23:34:43.652 写入 `run.request`。23:34:51.378 驱动写出时间线和 `local-result`。没有点开始直播。用户 OBS pid 44740 仍从 `D:\obs-studio\bin\64bit\obs64.exe` 运行，配置未改写。

录像 SHA-256 `fecaccd765de5219c7ddfb5723e540ea5b5619e58ab7f2ef6c7a303fb718967f`，131018 字节。Matroska 时长 59966 毫秒，时间刻度 1000000 纳秒。视频载荷 49356 字节，8 个关键帧；音频载荷 52791 字节。大于 200 字节的视频块时间戳是 0、567、700、6300、6367、7300、7700、7833 毫秒。其余多数 P 帧约 18 字节，对应窗口在句后继续待机。Windows Shell 时长列是 00:00:59。录像停止之后 websocket 读到 `outputActive` false、`outputBytes` 102184、`outputDuration` 0；容器自己的 Duration 是 59966。本轮没有解码像素核对字幕字形。

## 没有改变的路径

这次不是平台开播，不是口型评分，不是硬件静音证明，也不是写实样片。1.3.1 仍开放。MuseTalk 1.5 留在隔离研究。没有把 Python 放进正式运行包。真实挑战续租、认证 IPC、独立故障隔离和生产许可仍按 1.3.15 / 1.3.16 另行装配。撤销后已提交的操作系统缓冲区仍不能撤回。探测析构超时后 worker 仍可能跑到自己的停止。事件等待上限内仍可能跑完当前等待。164 项验收仍全部 pending。
