# 静音设备上按已播放样本出画

## 结果

在 Windows 默认输出设备上跑了一次公共执行。PCM 是 24000 个静音样本，画面是内置 2D。同一条受控 WASAPI 流填写设备时钟。`PlayedFrameBoard` 按已播放源样本选帧，再交给 `PublicClipRenderer` 出画。

最后一帧：源样本 8000，帧号 11，已播放 8601，提交 9840。源样本不超过已播放。提交窗口超前为真，选帧没有跟提交游标走。流代次 1，设备频率 384000 Hz。

`cargo test -p nai-public-output-keeper --test played_av --offline -- --ignored --nocapture` 1 项通过，用时 1.44 秒。`openspec validate v49-realistic-avatar --strict` 在测量前返回 “Change 'v49-realistic-avatar' is valid”。1.3.10.3 已按这次测量勾选。1.3.10 整项和 1.3.11 没有勾选。

## 没有改变的路径

夹具哈希不是输出授权。没有 OBS 实录，没有口型评分，没有平台开播。没有把 MuseTalk、960×540 研究帧或 Python 路径接入公共窗口。没有授予播放许可。164 项验收仍全部 pending。
