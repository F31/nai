## ADDED Requirements

### Requirement: 独立常驻模型
worker SHALL 在显式启动时加载一次FlashHead Lite与参考形象，后续任务复用模型，保持单活动渲染及有界队列，不能每句重启模型或静默重试失败任务。

#### Scenario: 连续两个任务
- **WHEN** 同一worker依次处理两个合法任务
- **THEN** 模型加载次数 MUST 为1，任务结果分别绑定任务ID/epoch，运动状态和音频历史不得串入下一任务

### Requirement: 同源PCM与迟到丢弃
worker MUST 在GPU调用前核对PCM字节摘要、样本数、格式及参考形象身份；控制与媒体分离，停止或取消后不得发布旧任务的新帧。

#### Scenario: 输入变化
- **WHEN** 文件实际摘要或长度不符
- **THEN** worker MUST 拒绝任务，不调用渲染、不宣称完成

#### Scenario: GPU执行中取消
- **WHEN** 控制取消在GPU执行期间到达
- **THEN** worker SHALL 立即撤销输出资格，迟到块丢弃，保持busy直到真正退出；不能把控制回执当GPU抢占完成
