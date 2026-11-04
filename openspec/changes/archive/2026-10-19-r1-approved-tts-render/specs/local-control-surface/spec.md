## ADDED Requirements

### Requirement: Approved candidates produce a local speech-and-frame preview
引擎 SHALL 允许操作者对仍有效的已批准候选发起一次有界预览：读取该候选的最终文本，经配置的 API 合成语音，解码为有界 PCM，产生真实 RGBA 帧并双水位提交，把可试听音频与帧摘要写入本机数据根。预览 MUST 绑定候选哈希与当前 epoch，MUST 在任何 Provider 调用前复核审批、证据、TTL 与权限；失败、取消、过期或换代 MUST 不产生预览产物，也不得使用请求方提供的替代文本。预览成功 MUST NOT 被报告为设备播放完成或 PlaybackAck。
#### Scenario: Approved candidate preview
- **WHEN** 候选仍有效且操作者发起预览
- **THEN** TTS 请求使用该候选最终文本与哈希，产物记录真实样本数、帧数与帧哈希，界面明确标注未播放
#### Scenario: Unapproved, expired or stale candidate
- **WHEN** 候选未批准、审批过期、证据失效或会话已换代
- **THEN** 不发起 TTS/渲染，返回具名拒绝且不写产物
#### Scenario: Revocation during synthesis
- **WHEN** 合成或渲染期间发生静音/接管或停机
- **THEN** 在途任务以具名失效结束，不提交帧、不写预览产物
#### Scenario: Repeated preview command
- **WHEN** 同一命令重试
- **THEN** 返回原 ACK，不重复调用 TTS 或重复计费
