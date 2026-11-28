# 单设备跨句执行器：原生三段与撤销故障通过，Root装配待接

## 实现

stream_audio升级wire3，拒绝旧wire1/2。增加OpenScript、SegmentBegin、SegmentSeal以及MAC覆盖的有界逐句进度。OpenScript一次创建Buffer、AudioLease和WASAPI Device，句尾不再退出进程；只有整段最终Seal及实际播放完成才结束。

执行器消费上轮continuous_audio::Timeline，逐句真实字节/样本/hash封口后将可信边界交给PCM Buffer。设备短读仅在整段最终尾部或已验证句尾可等待；普通句内缺样本仍报错。等待写入的静音设备帧不提交源Timeline，因此源played_samples不会凭墙钟或静音增长。逐句完成依据实际设备时钟，最多返回当前/未来2项，发送回执后退休完成项；脚本/句身份floor仍保留。

控制认证、命令递增、原始PCM独立stdin、300ms续租/最高500ms安全上界、200ms设备时钟新鲜性不变。停止回执表示软件撤销，不冒充硬件瞬时静音。未放宽45秒业务期限。

## 原生与定向证据

候选程序：E:/projects/nai/target-windows/v561-clock-build/continuous-device.exe。

测试使用明确标注的零音量PCM夹具，不是模型语音或口型质量证据，0次模型调用、无录音或推流。

- continuous_three_segments_share_device_and_boundary_wait_excludes_source通过：单PID48728、device_generation=1，3个独立身份/封口/设备完成回执，实际14400源样本播放完成。每句后保持约80ms续租等待，源played固定在句尾；三段始终同一设备实例。全局最终封口完成，控制和原始媒体均真实跨进程。
- continuous_stop_and_unsealed_future_fail_closed通过：显式Stop报告Stopped；前句未封口即提交未来句终止进程。
- forged_mac_version_and_epoch_are_rejected_before_device通过：伪MAC、旧wire1/2、零epoch均拒绝。
- lease_loss_and_partial_pcm_cannot_hold_control_open通过：续租失联和半包媒体不能卡住控制路径。
- PCM6项（含可信句尾允许等待/句内短读仍失败）、跨句内核6项、设备时钟3项、回执签名1项通过；新增segments字段篡改同样签名失败。TS/Python互通1项通过。
- 执行器定向构建与clippy通过。322消息/21proto生成及无漂移检查通过，OpenSpec严格校验通过。协议注释变更曾触发漂移，已正规重新生成，不手改产物。

## 构建限制与交付范围

cargo test/rustc的集成测试目标会连带重建引擎其他bin，运行中v561_live_control.exe受到Windows文件锁，另有一次编译子进程0xc0000409；因此整条集成构建命令未成功，不能写成完整构建通过。所需具名测试产物已实际生成，并直接运行上述4项原生测试全部通过；日志保留在target-windows/v561-clock-build/continuous-fault-build.log。未强杀用户工作台或反复要求关闭。

Root的v561_stream_audio与脚本音画协调器尚未装配OpenScript。当前工作台仍为此前交付的逐句设备版本；不能声称界面已使用单设备或约4秒空窗已消除。新旧wire不兼容，维护更新必须同时部署匹配的Root与执行器，禁止只替换当前音频程序。C03b1执行器完成，C03b2 Root装配及C04视频准备/C05统一更新未完成，OpenSpec不归档，34ACC/M0不变。

## 下一步

实现Root持有整段共享设备会话，逐句时钟观察映射回各自epoch、源偏移及Scheduler，保留实际完成回执后才能退休；整段停止/EOF撤销当前与未来句。用真实已有业务语音验证三句同窗口/同设备，再接下一句GPU提前生成。该Root装配是未实现工作，不标为外部Blocked。
