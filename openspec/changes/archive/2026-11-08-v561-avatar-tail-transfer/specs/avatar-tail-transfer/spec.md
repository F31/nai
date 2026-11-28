## ADDED Requirements
### Requirement: 真实生成及补尾计量
worker SHALL 报告同一任务实际生成、保留、传输与补尾计数，Rust MUST 核对其与配置、源样本及已消费像素一致。
#### Scenario: 缺失或伪造统计
- **WHEN** Complete缺少统计或生成/保留/补尾等式不成立
- **THEN** owner MUST 拒绝成功，不将缺失值当0或授实时通过
### Requirement: 只传输有效像素前缀
backend SHALL 在完整输出验证后只转换和传回最终所需前缀，不修改模型输入、采样或上下文。
#### Scenario: 尾块多余帧
- **WHEN** 模型输出帧数超过剩余有效帧
- **THEN** 传输计数 MUST 等于保留帧，前缀像素与完整转换后截取的结果相同
#### Scenario: GPU仍计算尾块
- **WHEN** 多余尾帧仍由GPU生成
- **THEN** 报告 MUST 保留该计算成本，不把传输减少称为持续实时通过
