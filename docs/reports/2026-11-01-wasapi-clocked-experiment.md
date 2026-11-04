# 隔离同流 WASAPI 实验后端

## 结果

音频适配器的 `clocked-experiment` 默认关闭。同一 `AudioClient` 写入并读取 `IAudioClock`。未激活时写入静音，已播放保持 0，设备位置仍可公布。门控撤销后写入函数不再被调用。错误流代次让 `observe_verified` 失败并撤销栅栏。`stop_within` 超时保留线程所有权，不报告硬件静音。

Windows 默认输出上的静音流：未开门控时已播放和提交都是 0；激活后已播放超过 2400 且不超过提交；撤销后 `stop_within(500ms)` 成功，之后 150ms 已播放不再增加。`CPAL` 的 `open_controlled` 仍在，该后端不授予输出授权。

`cargo test -p nai-audio-cpal --lib --features clocked-experiment --offline` 19 项通过，含新增的未激活静音写入和错误代次撤销。`cargo test -p nai-audio-cpal --test clocked_experiment --features clocked-experiment --offline -- --ignored silent_same_stream_clock_stops_on_gate_revocation --nocapture` 1 项通过，用时 1.25 秒。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.12 已按这次验收勾选。1.3.10 整项没有勾选。

## 没有改变的路径

没有 OBS 实录，没有口型评分，没有平台开播。没有把 MuseTalk、960×540 研究帧或 Python 路径接入公共窗口。没有替换 `open_controlled`。没有授予播放许可。Drop 仍可能无界 join。164 项验收仍全部 pending。
