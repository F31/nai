## ADDED Requirements
### Requirement: Product fact validity reaches retrieval and playback without implicit freshness
商品事实 SHALL 支持成对生效/失效时间，from严格小于until，保存/读取/发布/热发布 SHALL 保留UTC毫秒与闭开区间语义。旧数据缺失 SHALL 表示未声明，MUST NOT 自动赋永久有效或机器Fresh。人工辅助节目载入、生成及归位 SHALL 拒绝未声明、未生效和过期事实；普通手工问答保持既有准入与时间过滤。
#### Scenario: A legacy draft has no dates
- **WHEN** 读取未声明有效期的草稿
- **THEN** 明示未知，允许编辑保存；节目不调用Provider，要求补充时间
#### Scenario: Invalid or inactive windows
- **WHEN** 区间缺半边/倒置/越界，或已发布事实尚未生效/已过期
- **THEN** 非法配置拒绝保存，合法但非活动的事实不参与检索，不据此调用LLM
### Requirement: Evidence expiry bounds candidates and in-flight generation
生成候选和调用deadline SHALL 不晚于所有引用证据最早失效时间。过期生成结果不得进入待审批队列，已批准候选仍须在合成/播放复检有效期；媒体租约不得超过该期限。MUST NOT 为失败自动重新计费或把设备提交表示为硬件完成。
#### Scenario: Evidence expires during a model request
- **WHEN** 已派发LLM调用尚未结束时事实失效
- **THEN** 有界取消，保留真实用量结算，不产生可批准候选、不自动重发
#### Scenario: Approval outlives its fact
- **WHEN** 候选批准后引用事实到期
- **THEN** 新的语音合成/播放拒绝，不因审批原期限未到而继续
