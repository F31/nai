## Why
讲品进度目前由界面保存，引擎不知道固定段落或本轮调用上限。需要先形成真实引擎生产者，才能接入持续讲品和插问归位。

## What Changes
- 增加绑定当前知识快照和 epoch 的单轮四段人工辅助节目 API。
- 引擎复用 PitchRunner 顺序和冷却规则，每段只接受一次生成，无自动重试；继续使用现有持久预算。
- 只在匹配候选已提交本机设备后允许操作员确认听完，不产生硬件完成 ACK。
- 静音、接管、知识切换或期限到期后旧节目不可继续，不伪造五面重检通过。

## Capabilities
### New Capabilities
无。
### Modified Capabilities
- `basic-selling-loop`: 引擎有限人工辅助节目生产者。

## Impact
OpenAPI、control-api 与 engine；本轮不替换桌面人工辅助面板，不变更安装和真实凭据。B07 保持原验收状态。
