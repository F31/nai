# live-question-return Specification

## Purpose
Define a bounded, source-bound FAQ interruption of automatic narration. Pause at a verified sentence boundary, submit the separately approved answer through the public media path, and resume the first unspoken script sentence only after trusted completion. Preserve explicit failure and cancellation outcomes without replaying uncertain audio.

## Requirements

### Requirement: 插问必须在句边界暂停并绑定来源
系统 SHALL 仅接受当前发布源、当前问题池的有效FAQ候选，显式批准其原文，在当前句边界暂停并持久保存未播游标，同一时刻最多一个插问。
#### Scenario: 重复或陈旧请求
- **WHEN** 插问ID已使用、原问题取消、资料或epoch已变化
- **THEN** 拒绝准备或播出，不替换原队列和费用记录

### Requirement: 回答提交与原稿归位分别记录
系统 SHALL 对实际回答执行TTS与公共守门，只有可信提交完成才记录已回答并返回原未播游标，不将回答计入原稿进度。
#### Scenario: 回答失败或取消
- **WHEN** 合成、授权、媒体提交失败或操作员取消
- **THEN** 不自动重试、不冒充完成，保留明确暂停和恢复入口
