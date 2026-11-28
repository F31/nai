## ADDED Requirements

### Requirement: 连续滤波且仅EOS补尾
重采样器 SHALL 保留跨块滤波状态并按模型固定几何输出块，只有上游校验EOS后才flush和补尾，源样本与补零分开计数。

#### Scenario: 任意网络分片
- **WHEN** 同一PCM以不同大小的合法分片到达
- **THEN** 有效特征样本数量与既有整段算法一致，数值误差在明确测试阈值内。

#### Scenario: 失败和重复结束
- **WHEN** 取消、超限、格式错误或重复EOS发生
- **THEN** 终结组件，不再输出或复用旧滤波状态。
