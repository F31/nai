## ADDED Requirements
### Requirement: Assisted outline work preserves position without inventing playback completion
桌面 SHALL 支持已发布单商品提纲的单轮人工辅助操作，最多16段；每段显式生成、审批、播放，MUST NOT 自动批准、自动重试或将设备提交当作硬件完成。前进 SHALL 要求匹配本段候选的本机提交观测及操作员确认听完。
#### Scenario: Another candidate or a file preview finishes
- **WHEN** 非本段候选完成提交或仅完成文件预览
- **THEN** 本段不得允许确认前进
#### Scenario: A segment is confirmed
- **WHEN** 本段候选已提交本机且操作员确认听完
- **THEN** 只前进一段，最后一段后结束，不自动请求下一段或写硬件ACK
### Requirement: Assisted interjection return requires explicit revalidation
插问 SHALL 先请求静音并保留段落位置，丢弃本段候选关联。恢复 SHALL 重读同一发布修订/快照及实时会话，要求操作员逐项核对商品、价格、活动、素材和授权；未知不得确认。断连或发布 SHALL 使本轮失效。恢复 MUST 重新生成，不复用未播文本。
#### Scenario: Snapshot changes or a face is unknown
- **WHEN** 快照或草稿修订改变，或任一核对未完成
- **THEN** 拒绝恢复，无Provider请求
#### Scenario: An old generation returns after interruption
- **WHEN** 插问后旧生成请求才返回
- **THEN** 不得把其候选关联到保留位置或允许前进
