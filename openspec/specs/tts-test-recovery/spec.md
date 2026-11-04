# tts-test-recovery Specification

## Purpose
TBD - created by archiving change v48-05-tts-test-recovery. Update Purpose after archive.

## Requirements

### Requirement: 测试等待有界且未知提交不得重发
系统 SHALL 明确显示测试准备、提交和合成阶段，并在原生回执超时后恢复页面操作；未知提交 MUST 保持原任务身份，只能核对或取消，不自动创建新付费请求。
#### Scenario: 停止或提交悬挂
- **WHEN** 停止试听或测试提交没有在期限内返回
- **THEN** 页面显示具体等待阶段；停止超时不派发测试，提交超时按原ID核对且不重发
#### Scenario: 迟到回执或页面离开
- **WHEN** 原回执迟到或用户已离开测试
- **THEN** 迟到结果不创建新任务，离开后的前置等待不能继续派发；成功音频须手动试听
