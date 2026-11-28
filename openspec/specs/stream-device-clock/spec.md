# stream-device-clock Specification

## Purpose
为V5.6.1分句流式语音提供经过传输认证、身份/范围/状态校验和有限新鲜度约束的实际设备源样本观察，并通过有界出口连接帧调度；不以元数据替代帧来源认证或公共音画验收。

## Requirements

### Requirement: 认证流式设备观察
流式音频执行器 SHALL 以实际设备源样本提供带epoch、设备代次、观察序号、状态及年龄的签名时钟。根 MUST 拒绝缺失、旧版本、身份/样本不符、倒退和过期观察，不能用已提交样本推算播放。

#### Scenario: 设备轮询停止但控制仍应答
- **WHEN** 回执仍到达而真实设备观察超过200ms
- **THEN** 根 MUST 拒绝该观察并停止当前装配，不刷新为新的运行时钟

#### Scenario: 准备与结束
- **WHEN** 设备尚处准备或已完成/停止
- **THEN** 时钟 MUST 分别为Unavailable或Stopped，不授运行帧调度权限

### Requirement: 有界时钟观察出口
根 SHALL 只通过有界非阻塞通道发布已校验时钟，且未来帧调度与音频共享epoch及设备代次。消费者与生产帧仍需独立身份和新鲜度校验。

#### Scenario: 消费者迟滞
- **WHEN** Running观察通道已满
- **THEN** 系统 SHALL 允许丢弃中间观察，不阻塞音频续租或补造样本；终态不能伪报交付成功

#### Scenario: 旧帧
- **WHEN** 观察对应的epoch或设备代次与帧调度器不符
- **THEN** 调度 MUST 拒绝旧帧或旧时钟并进入安全状态
