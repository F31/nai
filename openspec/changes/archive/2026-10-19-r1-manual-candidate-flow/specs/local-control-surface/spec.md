## ADDED Requirements

### Requirement: Manual questions produce guarded approval candidates through the configured API
引擎 SHALL 通过已有手工事件命令接受操作者问题，在有界任务内实际检索已发布知识、装配上下文并调用配置的 API，将守门通过的最终文本保存为同一会话的待审批候选。凭据 MUST 仅由可信本机引用解析，预算 SHALL 使用持久化 reserve/dispatch/finish。事件状态与桌面 SHALL 区分准入、生成完成、拒绝与故障；准入 ACK 不代表生成、批准或播放。缺配置、无证据、外发禁止、取消、过期或换代 MUST NOT 使用桩替代或留下可批准旧候选。

#### Scenario: Operator generates and approves a question
- **WHEN** 操作者在桌面输入问题且配置、知识与 API 可用
- **THEN** 真实检索结果进入实际 API 请求，最终文本与候选哈希出现在现有审批列表，批准或拒绝作用于同一候选，未播放

#### Scenario: Repeated or overloaded requests
- **WHEN** 同一命令重试或已有生成任务占用容量
- **THEN** 重试不重复发起 API，超容量具名拒绝，状态历史保持有界

#### Scenario: Revocation or expiry while generating
- **WHEN** 网络返回前会话被静音/接管、epoch 改变或命令 TTL 到期
- **THEN** 响应不得写成有效待审批候选，事件状态给出具名失效原因

#### Scenario: Evidence must not leave the machine
- **WHEN** 检索缺失或命中文档被标为 local-only
- **THEN** 不向 Provider 发送问题与证据，也不生成候选或伪造答案
