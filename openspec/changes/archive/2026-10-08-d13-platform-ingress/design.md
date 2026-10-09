# Design

## Context
V3.5 §10.2/§33 D13；既有 scheduler 使用有界事件队列，宿主已提供身份/握手机制，但平台 SDK/凭据不在仓库。

## Decisions
- 先补 Protobuf，再 cargo xtask schema 生成。新增 optional epoch 区分缺失与合法 0；connection_generation 从 1 递增，0 无效。旧发送端可解码但缺字段不能进入官方入口。
- nai-kernel::platform 不依赖平台 SDK。PlatformLink 固定 session/epoch/platform/app_scope，准入需要可信 root 的 authenticated 判断、PlatformCapabilities 和验证过的 room_source_ref。布尔值是可信调用方决定，不是网络请求自报权限。
- 准入存储一份权限快照，限制输入大小/键数；权限有效期限转换成单调 Instant，本地墙钟回退不延长授权。重连需要新 revision，拒绝隐式继承；启动已连接链接失败不改变状态。
- 官方事件检查来源、精确作用域、epoch、连接代次、room_source_ref、authority_ref、权限细分键、大小、payload_version、event TTL 和最大时钟偏差。使用 CallContext deadline/cancellation。
- 不建立第二个队列或去重器。调用者在 enqueue 前和消费后都验证；集成测试接入 EventIngress。断线/权限撤销通过显式 disconnect 清除快照，不等待队列。输出租约/自动权限继续由既有 Session/MediaGate 管理，连接成功不恢复它们。
- 权限变更要求先 disconnect，再经 root 提供新快照准入；旧队列事件因代次不同拒绝。官方事件本身不是可信撤权控制命令，宿主状态通知必须走可信 disconnect 路径。

## Risks / Trade-offs
- root 伪造身份/房间来源不能由字符串检查发现 → 不将此逻辑测试当作 SDK/账户认证；root 接线须后续实际认证。
- 去重窗口可能误合并 → 沿用 scheduler，不制造消息 ID；无可靠 ID 的降级策略尚待具体 SDK 设计。
- 停播/清理实际宿主进程尚未接线 → 平台断线不冒充硬件静音 ACK；媒体独立租约仍兜底。

## Verification
kernel 集成测试覆盖断线/重连、缺权限/unknown、不匹配房间/来源/epoch/代次、时间偏差、TTL、取消/过期、队列旧事件消费。执行生成/完整 contracts suite、portable 与前端检查。
