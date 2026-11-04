# 公共窗口按已播放样本选帧

## 结果

Windows 公共输出执行不再用 CPAL `open_controlled` 写声卡。准备阶段打开同一条受控 WASAPI 流，先写静音并确认设备位置为零，执行租约接受后才放开句内 PCM。探测端公布流代次、设备频率、原始位置和已播放源样本。状态在流代次和频率都有效时填写 `device_clock`。

画面用 `PlayedFrameBoard` 按已播放源样本选择当前帧。提交游标只保留原义，不作为选帧输入。时钟尚未确认时不选帧。字幕是否可见仍按提交游标。停止、提交完成、取消和租约拒绝都会撤选帧器并撤实验栅栏。

`cargo test -p nai-media-core --lib played_frame --offline` 13 项通过，含 `offer_windows` 从已播放位置排队窗口。`cargo clippy -p nai-media-core -p nai-audio-cpal -p nai-public-output-keeper -p nai-workbench-host --offline -- -D warnings` 通过。`openspec validate v49-realistic-avatar --strict` 在改动后返回 “Change 'v49-realistic-avatar' is valid”。1.3.10.1 已按这次接线勾选。1.3.10 整项和 1.3.11 没有勾选。

## 没有改变的路径

没有跑 keeper 的真实设备 `clocked_execution` 套件，没有公共音画或 OBS 实录，没有拔设备。没有把 MuseTalk、960×540 研究帧或 Python 路径接入公共窗口。没有改协议字段，没有授予播放许可。164 项验收仍全部 pending。

随后设备流失败停播记在 [公共设备流失败停播](2026-11-01-public-device-lost.md)。
