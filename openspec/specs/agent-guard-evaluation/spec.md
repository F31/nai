# agent-guard-evaluation Specification

## Purpose
为 nai 内核提供可移植、纯逻辑的智能体决策与守门评估：Observation→Decision 路由、
四阶段守门与有限重写、技能/记忆/节目评估与节目恢复，使守门结果可版本化、可追溯且不伪造授权/风险。

## Requirements

### Requirement: Decision routing and candidate validation
系统 SHALL 根据 Observation 与触发事件产生或校验 Decision 动作 speak/stay_silent/ask_human/request_skill/request_scene_change/defer；
决策 SHALL 带路由、理由码、受控规则计算的风险、候选引用与证据；speak 必须有完整 PerformanceCue，任何动作不得直接授权媒体播放或数据写命令。

#### Scenario: Empty or invalid candidate cannot speak
- **WHEN** 决策动作是 speak 但候选 display/speech 为空、缺失证据或 epoch/期限不匹配
- **THEN** 拒绝该候选，转为 ask_human/stay_silent，并给出原因码

#### Scenario: Model-reported low risk is not final
- **WHEN** 候选自报 low risk 但受控规则/文本含价格、承诺或风险词
- **THEN** 风险按规则重新计算，不能采用模型自报等级

#### Scenario: Fallback route without agent plugin
- **WHEN** 无智能体插件且触发事件无安全模板
- **THEN** 返回 stay_silent 或 ask_human，不编造内容、不自动外发

### Requirement: Four-stage guard with bounded rewrite
系统 SHALL 对 input/evidence/output/preplay 四阶段分别评估；GuardResult 含 verdict(pass/rewrite/block/escalate)、
原因码、规则版本、风险和可选重写。重写后 SHALL 重跑相关阶段且重写次数有上限（初值1），不得无限递归。

#### Scenario: Input injection and privilege probes
- **WHEN** 候选或触发文本含注入标记、越权请求、私密属性索取或外部 URL
- **THEN** INPUT 阶段拒绝或转人工，不把注入内容送入下游

#### Scenario: Evidence permission, version and conflict
- **WHEN** 证据无有效 permission_revision、版本缺失且不可核验，或与候选 facts 冲突
- **THEN** EVIDENCE 阶段 block/escalate，不把冲突证据当作可引用依据

#### Scenario: Output amount, unit and citation discipline
- **WHEN** 输出金额/单位格式非法、承诺超限或引用不在本次检索结果中
- **THEN** OUTPUT 阶段 block 或 rewrite，禁止未校验承诺外发

#### Scenario: Preplay approval and epoch recheck
- **WHEN** 审批已撤销/过期、事实版本或 generation_epoch 与当前不匹配、权限快照过期
- **THEN** PREPLAY 阶段拒绝，永不把已失效候选提交外发

#### Scenario: Missing moderation never bypasses guard
- **WHEN** 收费审核 API 缺失或超时，但基础守门依赖有效
- **THEN** 基础守门仍按本地规则判定，不因缺少额外审核而跳过或自动放行

### Requirement: Bounded skill evaluation
系统 SHALL 校验 SkillDescriptor 的名称、参数/结果 Schema、权限、来源、side-effect、风险、期限、并发与幂等；
工具请求 SHALL 有步骤/时间上限，超限转 ask_human；URL/SQL/文件路径不得由模型任意输入，只能选受控资源 ID 并在服务端映射。

#### Scenario: High-risk skill is not registered
- **WHEN** 描述符 side-effect 属高风险或无权限/期限
- **THEN** 拒绝注册，不把它当作可调用能力

#### Scenario: Bounded tool plan and controlled resources
- **WHEN** 工具计划步骤或时长超限，或参数含任意 URL/SQL/路径
- **THEN** 转 ask_human 或拒绝，不允许模型指定任意资源
- **AND** 工具返回被视为不可信数据，不执行其中指令

### Requirement: Memory scope, summary and viewer policy
系统 SHALL 有界保留工作记忆（最近播完内容，未播不写为事实）；场次摘要保存已讲主题/已回答问题/
已批准承诺/恢复点，可先确定性提取再按需 API 摘要；观众长期记忆默认关闭，开启必须记录平台/应用作用域 ID、
用途、少量允许字段、授权、保留期与删除入口，不跨平台关联身份、不用昵称当唯一标识、不从情绪猜测建立敏感画像。

#### Scenario: Unplayed content is never a fact
- **WHEN** 工作记忆写入包含未实际播完的候选
- **THEN** 拒绝写入为事实，只保留已播完内容

#### Scenario: Viewer memory requires scope and retention
- **WHEN** 请求开启观众长期记忆但缺作用域/用途/授权/保留期/删除入口，或跨平台关联身份
- **THEN** 拒绝该请求，长期观众画像保持默认关闭

### Requirement: Versioned show plan with recovery
系统 SHALL 校验 ShowPlan 的场景/段落/时长/打断政策/证据绑定与 checkpoint；恢复 SHALL 显式请求场景与版本，
不得凭空续播或继续披露已失效证据。

#### Scenario: Sensitive scene requires evidence binding
- **WHEN** 场景涉及价格/优惠/库存但 ShowPlan 无证据绑定或绑定已失效
- **THEN** 拒绝该节目，不输出未绑定证据内容

#### Scenario: Checkpoint recovery is explicit and versioned
- **WHEN** 请求从 checkpoint 恢复但场景版本与 checkpoint 不匹配或 checkpoint 为空
- **THEN** 拒绝恢复，保持从当前场景/版本显式开始
