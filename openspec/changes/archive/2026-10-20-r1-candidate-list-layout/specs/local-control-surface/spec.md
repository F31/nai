## ADDED Requirements
### Requirement: Full width question and batch candidate review
桌面 SHALL 将提问与候选列表各占整行，发送按钮位于输入右侧；主操作仅播放/静音/接管；候选支持悬浮全文及列表右上集中批量审批。
#### Scenario: Batch approval confirmation
- **WHEN** 操作者选中候选并点击审批
- **THEN** 展示固定候选全文集合，经显式确认后逐条复检并提交，失败停止且如实显示部分成功，不自动播放或重试
#### Scenario: Cancel or stale candidate during batch
- **WHEN** 会话换代、候选过期/变化、断开或静音撤销当前操作
- **THEN** 不发送后续审批；已提交项不得被描述为回滚
#### Scenario: Read full candidate text
- **WHEN** 鼠标进入候选行或键盘聚焦全文入口
- **THEN** 显示完整内容浮层，离开并失焦后隐藏；确认窗口始终提供全文
