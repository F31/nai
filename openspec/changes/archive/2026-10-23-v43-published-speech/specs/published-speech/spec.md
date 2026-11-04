## ADDED Requirements
### Requirement: 单段业务语音明确批准
系统 SHALL 从有效发布单元的私有预演派生语音计划，并要求绑定文本、模型音色、source/session epoch、角色、目标与有效期的审批hash，事务持久化后才派发有预算TTS。只读计划不得发起API或写费用。
#### Scenario: 计划后内容或模型改变
- **WHEN** 批准时任一绑定已改变
- **THEN** 拒绝且不预留费用、不派发，不截断或替换文本
### Requirement: 语音持续复检与撤销
系统 SHALL 在派发、合成完成、授权及原生播放续租前重新核对当前发布事实、会话epoch、审批hash与模型；停止/撤销/过期后不恢复旧音频，不重试付费请求，不以Submitted冒充硬件完成。
#### Scenario: 播放中停止预演或撤销事实
- **WHEN** 本机停止或当前source失效
- **THEN** 优先停止或失联租约关闭输出，旧产物不能再次授权，费用历史保留
