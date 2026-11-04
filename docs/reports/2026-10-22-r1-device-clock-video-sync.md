# 2026-10-22 实际设备时钟驱动画面

## 已实现

修复常驻画面跟随音频copied缓冲位置而可能超前的问题。画面现在由当前cue真实IAudioClock所覆盖的played帧数选取，使用有理帧率整数算术。先读played再读copied，避免并发前进导致假越界；回退、超出复制或独立预期总数、陈旧/伪造提交位置使每cue VideoCursor失效。

选帧与提交分离。像素读回/摘要校验后、窗口update前再次检查已观测取消、租约、音频健康和cue错误，成功update后才登记帧索引与实际音频位置。最终成功要求音频实际到尾且对应最终帧已提交；结束前再次检查撤权，取消/中性待机不携带成功尾帧。

先修改media.proto并生成Rust/TS/C++：LocalPlaybackStatus新增optional video_frame_index(15)、optional video_device_samples(16)和video_tail_submitted(17)。宿主两个成功出口统一比较独立预期帧数减一、完整设备帧数及提交标志；缺字段、错误索引、位置提前等均拒绝。节目人工确认继续以原报告SHA256绑定并复检这些字段，无新增用户步骤。见 [ADR0146](../adr/0146-device-clock-video-submission.md)。

**这关闭实际音频时基→画面选择→尾帧提交→宿主/人工确认复检的接线子闭环，不是完整机器音画完成。** 窗口update不证明每帧都显示、屏幕/OBS采集完成或人耳听到；忙碌时可能跳帧。本轮不宣称精确音素口型或同步误差认证。actual_local_playback_completed仍false，无PlaybackAck、机器已完成句、正式恢复检查点或自动讲品/插问归位。

## 验证

- media-core视频游标最终3/3：复制提前不推进、尾部选择未提交不成立、单调/边界/复制回退、失效不恢复、新cue不继承、有理帧率、重复位置、陈旧/伪造索引拒绝。
- 引擎lib20/20，含精确音频/描述符/视频尾部拒绝，以及人工确认报告替换/缺失/超限/错误epoch/不完整进度等既有边界。
- Windows真实keeper最终1/1：同PID/单stream多句、4秒及短片段、取消、换epoch、帧损坏后待机与新句恢复、EOF待机；完整提交检查最后索引与设备帧数，取消不携带尾帧字段。
- Windows真实engine设备集成2项分别1/1：人工确认报告防替换与持久一次；普通播放两句/单stream及播放中静音撤权。使用明确本地协议Provider与媒体夹具，无真实付费API调用。
- contracts suite通过：生成无漂移、Rust/TS/C++编译，Node契约44/44，25夹具/22消息类型双路径互通。新字段覆盖可选零索引及u64最大位置；这些是wire夹具，不是运行时成功报告。
- Linux及Windows的media-core/keeper/engine all-target clippy -D warnings、fmt通过。Windows构建通过。一次最终设备测试链接遇LNK1104，原命令重试通过；未放宽断言或新增业务重试。

只检查本轮改动及直接消费者，不执行全仓full，无新依赖/后台服务或真实付费模型调用，registry状态不升级。日志在`.nai-diagnostics/video-sync-{clock-final,engine-lib-final,device-final,program-device,engine-device,contracts,clippy,native-clippy,fmt}.log`。OpenSpec已归档为`2026-10-22-r1-device-clock-video-sync`，合并后的resident-local-output主规格严格校验通过。

## 安装更新

确认无nai活动进程后，仅同步更新原目录`D:\nai Engineering`的引擎和keeper；桌面壳保持原文件。更新前完整备份安装和用户目录至`target-windows/installation-backups/before-video-sync-20261022-064055`。8个用户文件更新前后数量与SHA256一致，未改凭据绑定或调用API。

引擎SHA256：`e3daac3beaeade9810d8c79ffc8b2b3d6a965aa9c23175cc751a630f04338a77`；keeper：`477b44738219985b15e25d2d3ac3e0bd256164eff4ab8521514148e7508836d7`。安装版GUI subsystem=2，`--smoke`退出0且报告`SHELL_UI_READY`。结果见`.nai-diagnostics/video-sync-install-result.json`。本轮为定向维护更新，未重新发布旧NSIS包。

下一主线：独立画面呈现/采集完成证据与实际录制、完整音画判据，随后正式节目检查点/五面复检、连续讲品与插问归位；真人音轨与OBS整场验收仍待完成。
