## ADDED Requirements
### Requirement: Human segment confirmation is durable and bound to playback evidence
人工确认 SHALL 复核本轮精确候选的原生播放报告摘要、独立预期设备帧数、已观测设备尾部、有效审批/事实与当前epoch快照。确认 SHALL 先critical持久保存具名人工复核记录再推进，失败不前进、不重试。该记录 MUST NOT 产生PlaybackAck、机器已完成句、场次记忆或恢复授权。
#### Scenario: Report replacement or storage failure
- **WHEN** 报告缺失/被替换/缺进度，或critical写入失败
- **THEN** 拒绝确认，原段落不前进且不调用Provider。
#### Scenario: Revocation during confirmation
- **WHEN** 持久提交后epoch或权限改变
- **THEN** 保留审计记录但暂停节目，不发下一段，不恢复旧授权。
### Requirement: Recent confirmations survive restart as read-only history
节目响应 SHALL 可展示当前工作区最近256条审计内最新人工复核记录及哈希链引用；重启 SHALL 默认无活跃节目，历史不重建runner/审批/候选或消费生成预算。
#### Scenario: Restart with a saved confirmation
- **WHEN** 同工作区重启引擎并读取节目
- **THEN** 活跃节目为absent，近期人工记录可见，读取不生成语音或自动前进。
