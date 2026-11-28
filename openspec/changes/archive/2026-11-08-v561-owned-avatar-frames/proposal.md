## Why
常驻worker仍自行启动诊断接收器，Rust owner只收到完成计数，不能直接消费真实帧或在自身控制范围内停止画面。
## What Changes
- worker私有协议升级为2，签名覆盖映射身份及FrameRef；Rust验证后通过固定只读helper取像素，摘要复检并回执槽位。
- 移除常驻worker配置中的独立receiver入口；每帧经有界owner回调消费，取消拒绝后续帧。
- 提供全句有界预准备音画预演，Rust设备实际样本驱动画面；明确不是持续流式实时直播。
## Impact
变更既有worker契约及全部当前消费者，无旧协议兼容。公共正式许可/映射ACL认证/长稳验收不升级。
