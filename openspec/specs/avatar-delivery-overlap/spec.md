# avatar-delivery-overlap Specification

## Purpose
规定单GPU计算与单CPU帧交付的有界重叠、取消清理和签名计量，明确每个在途块的所有权、完成时机和墙钟约束，避免以阶段累计时间冒充端到端收益。

## Requirements

### Requirement: 单计算与有界交付重叠
worker SHALL 保持单GPU计算序列和最多一个在途CPU交付块，交付复检任务取消及精确帧释放；退出前完成交付线程清理。
#### Scenario: 下一块计算与旧块交付
- **WHEN** 前一块CPU帧仍在交付
- **THEN** 可计算下一块，但在物化下一块CPU帧之前必须等待旧交付完成。
#### Scenario: 交付失败或取消
- **WHEN** 交付失败或任务取消
- **THEN** 不发送成功完成，不在交付线程退出前关闭映射，不复用旧任务输出。

### Requirement: 重叠计量可核对
worker SHALL 在签名协议4中报告真实重叠区间累计，Rust复检overlap不超过compute及delivery，累计阶段减overlap受墙钟约束。
#### Scenario: 伪造重叠
- **WHEN** overlap超出任一参与阶段或扣除后仍超墙钟
- **THEN** Root拒绝完成事件。
