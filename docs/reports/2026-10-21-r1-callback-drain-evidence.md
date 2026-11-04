# 本机输出尾部回调证据

日期：2026-10-21。规格 r1-callback-drain-evidence，决策 [ADR 0142](../adr/0142-callback-tail-observation.md)。

## 实现

- 先扩展media.proto并完整生成Rust/TS/C++契约，再修改消费者。
- keeper不再用复制完成后固定等待220ms产生submitted；改由下一次有效真实回调越过预计尾部，且原期限、epoch、撤权与错误信号仍有效时，产生本cue累计帧的观测。
- Pending只有一个，复用原有有界packet队列。DrainFence只否决，不具备新授权能力；P0路径不取数据库/模型/业务锁。
- 宿主两个成功出口均要求描述符摘要匹配、完整复制/排空计数与独立预期重采样帧数一致、窗口提交和单stream，证据随原playback.pb落盘。
- actual_local_playback_completed仍false；不写PlaybackAck，不自动推进节目、不保存已完成句。该诊断不代表硬件实际播放或扬声器/OBS验收。

## 验证

仅验证改动和直接关联范围。所有Provider使用本地HTTP fixture；设备测试为显式临时测试素材与授权，不读用户凭据/库、不调用付费API或替换安装。

| 范围 | 实测结果 |
|---|---|
| Windows callback队列 | 最终5/5：无后续回调/过早回调不补证；mute、仅epoch变化、到期、错误、缺时序、空缓冲拒绝；双声道frame计数及cue隔离 |
| WSL callback队列 | 4/4（第五项双声道随后在Windows执行，未声称WSL同次5项） |
| WSL宿主监督 | 2/2：硬件ACK拒绝、诊断结构限制、精确摘要/完整计数/观测标志；含protobuf Rust往返 |
| Windows原生keeper | 显式ignored设备用例1/1，六cue序列含4秒输出、短句、取消、换代、篡改恢复与EOF待机，始终单stream |
| Windows原生engine | 显式ignored集成1/1，实际设备提交证据落盘及静音取消；未产生PlaybackAck |
| 契约 | descriptor 3/3，包含TS可选计数缺失/0/正值往返、字段9/10/11与编译描述符对应；contracts TS build通过 |
| 静态检查 | Linux/Windows改动3包clippy --all-targets -D warnings通过，fmt通过 |
| 生成/消费者 | cargo xtask schema完整生成，schema --check无漂移，desktop typecheck通过 |
| 注册表 | 57/57；本轮不改变任何验收状态 |
| OpenSpec | 已归档2026-10-21-r1-callback-drain-evidence；严格主规格44/44、归档185/185通过 |

Windows Cargo增量缓存存在硬链接失败转复制提示，非业务失败。WSL并行命令出现构建目录等待锁，未据此跳过测试。未运行全仓full、全前端UI或重做未受影响的Provider测试。

Windows真实keeper用例已执行，观察到4秒cue的192000帧与短句24000帧完整回调排空，摘要逐cue匹配；cancel未报告完整观测，换代/帧损坏恢复仍只打开一条stream，EOF待机不重播。

Windows真实engine→keeper→设备用例已执行：24000帧回调证据由宿主核对并写入playback.pb；静音路径返回STALE_EPOCH，保持待机，不产生硬件ACK。该用例包含真实设备/窗口，但素材与Provider为测试fixture，不能称作真实付费直播/OBS录制验收。

## 仍未完成

CPAL的WASAPI输出时间戳是预计值（已核对锁定依赖源码），本轮没有实际设备播放位置生产者。因此硬件/后端可核验完成ACK、可恢复节目检查点、自动连续讲品/插问归位、五面重检、真人麦克风和OBS实录仍未完成，不将其标为外部阻塞或已验收。安装版未更新，registry不升级。
