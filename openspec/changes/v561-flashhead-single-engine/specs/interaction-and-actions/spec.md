# 互动与动作

## ADDED Requirements

### Requirement: 通用事件入口

系统 SHALL 提供本地 `POST /api/events` 接收弹幕/礼物 JSON；SHALL 永远返回 200 + `{queued, reason}`；SHALL NOT 内置平台鉴权、Cookie、签名或自动开播。

#### Scenario: 弹幕入队
- **WHEN** 合法弹幕到达
- **THEN** 系统过滤/去重/限流后入队，并返回 queued=true 或明确 reason

#### Scenario: 外部事件注入
- **WHEN** 事件文本包含改变权限、系统指令或越界动作
- **THEN** 系统将其作为不可信数据处理，不改变配置/权限/输出策略

### Requirement: 内容过滤与队列

系统 SHALL 对输入/输出执行红线、屏蔽词、提示词注入与动作黑名单过滤；SHALL 对队列设置容量、过期、同人/同文本合并、每分钟上限；命中丢弃 SHALL 记录。

#### Scenario: 红线命中
- **WHEN** 弹幕/回答命中红线
- **THEN** 整条丢弃或替换并记录，不送 TTS/画面

#### Scenario: 洪峰
- **WHEN** 事件超过每分钟上限或队列容量
- **THEN** 丢弃/合并并计数，不阻塞停止/接管

### Requirement: 预录动作片段

系统 SHALL 使用授权预录动作片段（如挥手、点头、摇头、微笑、比心、鼓掌、鞠躬、思考）；每个片段 SHALL 有哈希、时长、入点/出点、循环性、许可与变体；SHALL NOT 在主线依赖 LiveAct 生成动作。

#### Scenario: 片段触发
- **WHEN** 用户点击动作或关键词命中
- **THEN** 系统在 ≤300ms 内淡入插播片段，播完回实时流

#### Scenario: 片段缺失
- **WHEN** 请求的动作无授权片段
- **THEN** 系统拒绝并显示可用动作，不生成/伪造

### Requirement: 动作调度

系统 SHALL 对动作执行白名单、优先级、冷却、队列上限与停止优先；优先级 SHALL 为停止/接管 > 手动 > 关键词 > 待机；同动作 SHALL 有冷却。

#### Scenario: 动作冷却
- **WHEN** 同动作在冷却期内再次触发
- **THEN** 系统拒绝或延迟并记录，不刷屏

#### Scenario: 停止
- **WHEN** 用户停止/接管
- **THEN** 当前片段/排队动作立即取消，旧 epoch 动作不得执行

### Requirement: 礼物感谢（可选）

系统 SHALL 可选地对礼物事件生成简短感谢；SHALL 有每分钟上限、合并与模板兜底；SHALL NOT 阻塞问答/停止。

#### Scenario: 礼物洪峰
- **WHEN** 短时间大量礼物
- **THEN** 系统合并/限流，使用模板兜底，不无限生成

#### Scenario: 停止中
- **WHEN** 用户停止
- **THEN** 未播出的感谢被取消，不补播

### Requirement: 不做 LLM 动作标签与任意动作

系统 SHALL 默认关闭 LLM 动作标签；SHALL NOT 从自由文本生成任意动作、舞蹈或全身动作；如后续启用，必须新增规格与验收。

#### Scenario: 模型输出动作标签
- **WHEN** 模型返回未在白名单的动作
- **THEN** 系统忽略/拒绝并记录，不执行
