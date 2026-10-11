## MODIFIED Requirements

### Requirement: Bounded scoped approval listing

审批列表 SHALL 限制 workspace/session/epoch 作用域并按 created 顺序有界返回待审批候选；已撤销、已过期、已批准候选 MUST 被过滤，越界请求 MUST 用明确原因拒绝而不是隐式全量返回。列表项 MUST 携带候选哈希、审批引用（如有）、期限、证据计数与候选的**展示文本**；展示文本供操作员核对将要播报的内容，MUST NOT 被当作身份（身份仍是候选哈希）。列表 MUST NOT 返回候选的其余载荷（语音文本、证据明细、性能参数等）。

#### Scenario: Stale or approved candidates hidden
- **WHEN** 候选已批准、已过期或已撤销
- **THEN** 不再出现在待审批列表，且不能作为 approve/reject 的成功输入

#### Scenario: Limit exceeded
- **WHEN** 请求的 limit 超过服务上限
- **THEN** 返回可读拒绝，不得伪造空列表成功

#### Scenario: Candidate admission reaches capacity
- **WHEN** 当前 workspace/session/epoch 已有 200 个有效待审批候选而又入库新候选
- **THEN** 返回 ResourceExhausted 且不插入候选；HTTP 满页提示列表已达上限

#### Scenario: Candidate payload beyond display text
- **WHEN** 审查列表载荷
- **THEN** 每项含候选哈希、展示文本、期限与证据计数，且不含语音文本、证据明细或性能参数

## ADDED Requirements

### Requirement: Displayed candidate text is display-only and absence is named

候选的展示文本 SHALL 随候选入库保存（来自生成侧的最终可展示文本），并随审批列表返回。展示文本
MUST 只用于**显示**：批准与拒绝的身份 MUST 仍是候选哈希，界面 MUST NOT 用展示文本构造或替换命令身份。
当载荷**未**给出展示文本时（例如旧库补列前的行），界面 MUST 具名说明该缺失，MUST NOT 把哈希当作文本展示，
也 MUST NOT 以空串冒充「内容为空」。

#### Scenario: Payload carries the display text
- **WHEN** 审批项带有非空展示文本
- **THEN** 界面显示该文本，且批准命令仍以该候选的哈希为身份提交

#### Scenario: Payload omits the display text
- **WHEN** 审批项缺少展示文本
- **THEN** 界面报出具名的缺失说明，不显示哈希为文本，也不显示空内容

#### Scenario: Hash is never substituted for text
- **WHEN** 审查呈现逻辑
- **THEN** 不存在把 `id`（候选哈希）渲染成展示文本的路径
