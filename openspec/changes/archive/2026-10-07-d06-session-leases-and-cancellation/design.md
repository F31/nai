# 设计

复用 SessionSnapshot/PlaybackLease/Approval/PerformanceCue/OutputAck；不新增消息，生成文件不手改。
既有 sha256-canonical-v1 候选哈希算法移至 contracts 并由 storage 重导出，媒体直接重算规范化 Protobuf 对象（BTreeMap 保证编码顺序稳定），拒绝修改文本后沿用旧哈希。算法与持久化哈希保持兼容，复用已锁定 sha2。
kernel 仅依赖 contracts。Session 由 composition root 持有，trusted actor 与 preflight 结果通过参数注入，不能取自模型或插件自报。状态转换显式校验，任务令牌包含共享 epoch 与独立 AtomicBool，忙循环无法强杀但迟到结果可丢弃。

media-core 仅依赖 contracts。专用 EmergencyHandle 使用原子撤销计数，媒体线程每次消费检查；不等待普通锁。MediaGate 独占其有界元数据队列，P0 在下一次 poll/消费清空。静音句柄立即阻止后续消费，不宣称清除了已提交硬件缓冲。租约用收到时 UTC 剩余期限转换为 Instant，并设最大 600ms，续租不能复活过期/撤销租约；必须显式新 epoch 的 arm。

播放授权来自可信 root 的 cue/approval/current facts/guard 结果，不能仅信任租约自报。AutoSupervised 另需有效人工值守期限；心跳不授予值守。guard、审批、事实校验均 fail-closed。输出完成只接受匹配 output/utterance 的实际 ACK。

storage 新增撤销 checkpoint API，CAS 匹配旧 revision/epoch，同事务保存新 epoch、撤销审批、取消未完成 utterances、审计。kernel 不依赖 storage/media；调用顺序由测试展示：emergency.mute → session.revoke → storage checkpoint → 协作取消结果丢弃。存储崩溃恢复复用 D03。

验证：cargo xtask test --suite session；portable 全量 fmt/clippy/tests；Windows GNU 编译（非执行）；OpenSpec strict。故障测试涵盖单调过期、持有普通锁时 P0、迟到数据、重试/容量、恢复、跨会话、审批/事实变更、实际 ACK、checkpoint CAS。真实 PCM/设备/Windows 独立进程强杀为 D11/D18 未开始，无环境的原生验收为 Blocked。
