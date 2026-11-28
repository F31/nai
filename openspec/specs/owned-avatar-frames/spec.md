# owned-avatar-frames Specification

## Purpose
规定常驻形象worker的签名帧交付、Rust独立像素消费及有界设备时钟预演，区分短句准备证据与持续实时、映射权限和生产输出认证。

## Requirements

### Requirement: Rust拥有真实帧消费
worker SHALL 使用协议2签名通知真实帧；Rust MUST 核对任务、源身份、帧顺序/范围及实际像素摘要后消费并释放槽位，不以完成计数代替帧消费。
#### Scenario: 合法帧
- **WHEN** 同一活动任务产生合法帧
- **THEN** Rust SHALL 取得独立像素，消费后按精确槽代次回执
#### Scenario: 取消或篡改
- **WHEN** 源身份/帧摘要不符或父任务取消
- **THEN** Rust MUST 拒绝交付新像素，不将迟到结果视为完成

### Requirement: 有界预演据实际设备时钟显示
短句预演 SHALL 有界准备同源PCM和帧，显示位置由实际设备样本驱动。
#### Scenario: 停止或时钟过期
- **WHEN** 停止、任务取消或时钟证据过期
- **THEN** 画面 MUST 清空，不以墙钟继续播放旧帧
#### Scenario: 全句预准备
- **WHEN** 先生成完整短句再播出
- **THEN** 报告 MUST 保留准备耗时并标记非持续实时直播
