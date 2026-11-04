# 公共窗口按 cue 区间映射源样本

## 结果

公共执行把同一条受控 WASAPI 流上已绑定的第一段 cue 设备帧区间映射为句内源样本，再交给 `PlayedFrameBoard` 选帧。区间尚未成立、区间前和区间后的静音不推进该句。取消或 `DEVICE_LOST` 拒绝映射。映射核对当前租约代次。公共状态没有起点或终点字段，绑定留在 keeper 内部。

映射入口是 `map_public_cue`。区间数为 0 时返回 `Ok(None)`，不是 `Some(0)`。设备位置 38400、区间 `[4800, 28800)`、频率 384000 Hz 时映射为源样本 0；位置 134400 时映射为 6000；位置 230400 以及硬件残留位置 205120、区间 `[8832, 12192)` 都返回 `None`。空区间、零代次、零流代次和零频率在绑定时拒绝。代次不符在已绑定对象上拒绝。

keeper 在探测端失败时把门控设为 `DeviceLost` 并清空上次映射；执行已停止时设为 `Cancelled`。只有 `Ok(Some(played))` 才 `offer_windows` 和 `present`。`Ok(None)` 不把 0 交给选帧器。时钟观测里的已播放样本冻结为上次映射值。字幕仍按提交游标。

`cargo test -p nai-media-core --lib public_cue --offline` 7 项通过。`cargo test -p nai-audio-cpal --lib --features clocked-experiment --offline` 17 项通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.11 已按这次接线勾选。1.3.10 整项没有勾选。

## 没有改变的路径

没有重跑忽略的 WASAPI 硬件套件，没有 OBS 实录，没有口型评分，没有平台开播。没有把 MuseTalk、960×540 研究帧或 Python 路径接入公共窗口。没有改协议字段，没有授予播放许可。164 项验收仍全部 pending。
