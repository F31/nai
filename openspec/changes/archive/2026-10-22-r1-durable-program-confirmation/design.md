## Context
IAudioClock已提供音频尾部证据，但minifb更新不等于完整音画实际完成。既有cue种类为critical，可承载明确subtype的节目人工确认，无需扩大数据库kind或升级schema。
## Goals / Non-Goals
Goals: 报告防替换、持久成功后前进、重启可只读看近期记录、可解释失败。
Non-Goals: 机器已完成句、场次记忆、自动恢复/自动下一段、硬件/OBS认证。
## Decisions
- 播放成功结果保留报告SHA256；确认仅使用本轮精确候选的原生submitted记录、独立预期样本数，复读≤16KiB报告，不接受旧报告或换包。
- 确认复检批准对象/事实版本、有效期、epoch/快照/期限，critical cue payload带event=program_human_confirmation，含操作者/节目/段落/报告摘要/音频帧/窗口提交诊断；playback_ack永为false。
- 先验证可前进，再critical提交，最后复检epoch后前进；提交后的撤权/冲突使节目暂停，记录保留但不授予恢复权限。数据库失败不前进，无自动重试。
- 历史读取仅扫描当前工作区最近256条决策，返回最新匹配记录及哈希链引用；不是完整归档检索。重启absent可显示历史；不重建runner/候选或预算。
- OpenAPI添加可选last_confirmation保持旧响应可读；桌面严格解析新记录并只读展示，异常不当作成功。
## Risks / Trade-offs
人工确认是操作员声明，不是机器音画完成或自动检查点。记录仍在已有审计库中持久保存，有限近期窗口可能不包含较老确认，界面不据此断言历史不存在。Windows实际设备集成用明确本地协议fixture验证，不调用真实Provider；自动测试不冒充人耳/OBS证据。
