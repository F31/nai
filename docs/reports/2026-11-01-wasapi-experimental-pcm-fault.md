# 隔离实验有界 PCM 与失败回滚

## 结果

默认关闭的同流 WASAPI 实验后端只接受 1 到 480000 个 24 kHz 单声道 PCM 样本。空向量和 480001 个样本在打开设备前返回 `CLOCK_PROBE_INVALID_PCM`。写入失败撤销一次性栅栏，旧栅栏不能再激活，也不能用短期租约重新打开。下一流代次必须用新栅栏和新游标，旧游标拒绝观测。24 kHz 单声道转 48 kHz 双声道浮点、连续 cue 区间、区间之间的静音、过期租约和写入/时钟故障都由既有单测覆盖。公共执行已经消费这条探测流；失败时具名 `DEVICE_LOST`。该接线不授予播放许可，也不替换 `CPAL` 的 `open_controlled`。

`cargo test -p nai-audio-cpal --lib --features clocked-experiment --offline` 21 项通过，含新增的有界 PCM 拒绝和失败后下一流代次。`cargo test -p nai-audio-cpal --test clocked_experiment --features clocked-experiment --offline -- invalid_pcm_is_rejected_before_opening_a_device` 1 项通过。`cargo clippy -p nai-audio-cpal --features clocked-experiment --all-targets --offline -- -D warnings` 通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.13 已按这次验收勾选。1.3.10 整项没有勾选。

## 没有改变的路径

没有 OBS 实录，没有口型评分，没有平台开播。没有把 MuseTalk、960×540 研究帧或 Python 路径接入公共窗口。没有替换 `open_controlled`。没有授予播放许可。Drop 仍可能无界 join。没有重跑静音设备硬件项。164 项验收仍全部 pending。
