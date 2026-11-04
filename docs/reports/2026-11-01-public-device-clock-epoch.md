# 观测代次必须等于租约代次

## 结果

同一条静音实验流在租约代次 1 下读到已播放 171、提交 1200、设备位置 15032、频率 384000、流代次 1。按这个租约代次，接纳规则通过。同一条读数改成代次 2 被拒绝，代次 0 也被拒绝。代次 0 不能靠“至少为 1”的旧条件蒙混，批准代次本身为 0 同样拒绝。

被忽略的 `same_stream_clock_rejects_a_different_lease_epoch` 本次显式运行通过，用时 0.16 秒。契约谓词 `bound_observation_stays_within_submission` 通过。宿主设备时钟测试 2 项通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.10 和 1.3.11 都没有勾选。

## 没有改变的路径

公共 CPAL 播放仍只使用提交游标，不读 `IAudioClock`，状态里继续不填设备时钟。宿主只记住最近一次发给 keeper 的执行租约代次；还没发过租约时代次是 0，因此带时钟的状态过不了。没有装配公共输出，没有拔设备，没有把 cue 区间写进观测。没有真实公共音画或 OBS。没有跑 MuseTalk，没有重绑定位缓存。
