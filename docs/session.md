# 会话、租约与取消

```bash
cargo xtask test --suite session
cargo xtask check --profile portable
```

## 接线顺序

可信 composition root 持有 `nai_kernel::session::Session`、独立媒体循环的
`nai_media_core::MediaGate` 与存储 owner。kernel 不依赖媒体或存储。

1. 鉴权后的 P0 路径首先调用 `EmergencyHandle::mute()`。此路径不等待普通队列、模型或数据库。
2. 调用 `Session::command`（Mute/Takeover/Stop）或可信内部 `revoke`，推进 epoch 并使旧 TaskToken 失效。
3. `Store::checkpoint_revocation` 用上次存储快照的 epoch/revision 做 CAS；冲突或期限失败保持静音，重新读取协调。
4. 媒体工作循环调用 `poll`/`consume` 清除待播元数据；Provider 协作检查 TaskToken，任何不可中断的迟到结果仍丢弃。

`executed` 的 CommandAck 仅表示会话状态改变，不能作为实际设备已停止或平台观众已听完的证明。
`PlaybackProgress::acknowledge` 要求匹配 output/utterance、committed、实际本地完成与预期样本数。
跨故障的进行中 progress 取消，不恢复为待重播任务。

## 会话机制

- 生命周期遵循 Created → Preparing → Ready/ReadyDegraded → Running → Draining → Stopped；Faulted 恢复须重新 Prepare。
- actor_allowed/preflight 是可信权限/准入结果，不能从模型或插件自报字段取得。
- `change_mode` 必须重新授权并撤销旧 epoch；切到 AutoSupervised 还需有效值守租约。媒体状态与 Running 生命周期分离。
- `TaskToken::cancel` 只取消一条；全场 revoke 递增 epoch，取消全部旧任务。有效性同时检查 deadline。
- `command` 验证 session/workspace/epoch/revision/TTL，支持 Prepare/Start/Stop/Mute/Takeover/ChangeMode。
  审批类命令由后续守门/控制接线实现。重复 ID/相同载荷返回原 ACK，不同载荷报冲突。
- 命令记录上限 256，指纹上限 4096 字节，保留期限 ≤300 秒；root 必须使去重期限覆盖允许重试窗口。
  容量满不提前淘汰。P0 即便状态命令拒绝，媒体信号也须先执行。
- 引擎重启时使用 `recover_after(saved, media.epoch_floor())`，协调更高 epoch，重新 Prepare。
  `recover(saved)` 仅用于无存活媒体的恢复。恢复不继承旧审批、绑定、自动模式。

## 播放门控

`MediaGate::arm` 只接受可信控制路径的更高 epoch、新租约；期限必须在收到时 UTC 的未来 600ms 内。
收到时将 TTL 转成 Instant；墙钟回退不能越过本地单调截止。约每 200ms 调用 renew，renew 不改变
允许句集合、审批或事实，不复活过期/撤销租约。重新输出须显式新 epoch 授权。

`Preplay` 在每次消费前从当前可信 guard/权限/审批/事实状态重建，不能重复使用过时检查结果。
检查对象包括 session、epoch、完整候选重算哈希、审批引用/作用域/TTL/撤销状态、事实版本、策略、模式、
值守与租约。AutoSupervised 需要有效人工值守期限；网络心跳不授予值守。模板/缓存也使用同一门控。

当前队列只保存有界 utterance 元数据，不保存 PCM，不绕过控制/媒体分离。
EmergencyHandle 在下次消费处立即拒绝旧授权，队列由媒体下一次 poll 清除；已提交硬件缓冲不由该库清除。

## 值守租约（自动模式）

`Session` 持有值守租约（`nai_kernel::duty`）。只有 `confirm_duty` 这一显式操作员路径能授予或续期；
`note_heartbeat` 仅观测活性，**不延长** expires_at，平台/网络重连也没有恢复路径。因此
「桌面有人值守时手机断线」不会自动判为无人，反之租约失效也不会被心跳掩盖。

`enforce_duty(now)` 落实 ADR 0015 勘误：到期即不再许可新自动回复（`auto_reply_permitted` 同时要求
Running、AutoSupervised 与有效租约）；自到期起 600ms（`DUTY_REVOKE_DELAY`）后撤销输出授权
（epoch 递增、媒体状态 Muted），重复调用幂等；只有操作员再次显式确认才恢复。切到 AutoSupervised
必须持有效值守租约，否则拒绝且模式不变。

成功执行 Mute/Takeover/Stop 后会清除旧值守租约并退出 AutoSupervised（转 Assisted，保留其他已有非自动模式）。
这些控制之后仅重新确认值守不会隐式恢复自动；还须显式切换模式并重新授权。
到期撤权若因 epoch/revision 耗尽失败，返回 HaltedPendingRevoke 并退出自动模式，重新确认不能在旧自动模式下继续。
`DutyEnforcement::Valid` 只是没有到期动作要执行，不是自动许可；必须检查 `auto_reply_permitted`。
`enforce_duty` 是由调用方驱动的检查，没有自身后台计时器。可信 root 仍须独立定时驱动并先撤销媒体，不能把函数测试当作真实设备600ms停声证明。

租约仅存在于内存，`recover`/`recover_after` 不继承；授予/到期/撤销应写决策日志（kind=duty_lease，
critical），但 kernel 不依赖 storage，接线待 composition root。

## 验收边界

本轮测试是 portable 状态机与门控逻辑。真实媒体进程/PCM/音轨/设备清缓冲、强杀引擎、Windows 静音热键
和 P95 静音指标尚未实现/未实测，属于 D11/D18。Windows GNU kernel/media 编译通过；SQLite 存储
交叉编译缺 `x86_64-w64-mingw32-gcc`，不视为 Windows 认证通过。
值守租约已实测的是逻辑与期限语义；真实设备的「600ms 内实际停声」需要在 D11/设备里程碑上测。
