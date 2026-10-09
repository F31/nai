# D10 智能体/守门评估设计

## 位置与边界

新增 `nai-kernel/src/evaluate/`（kernel 层，仅依赖 contracts/prost），纯逻辑无 I/O：
`agent.rs` 决策路由与候选校验、`guard.rs` 四阶段守门与有限重写、`skill.rs` 描述符/工具计划、
`memory.rs` 工作/场次/观众规则、`show.rs` 节目与恢复。实际守门插件在 D10 验证为独立实现，
内核守门不因插件缺失被绕过。零值不授予 PASS；风险由受控规则计算。

## agent

`evaluate_decision(obs, proposed, policy, now_ms) -> Decision`：
- 校验 context/session/workspace、remaining_budget；动作集合白名单。
- SPEAK：候选 display/speech 非空且有界、risk 按规则重算、epoch 与 context 匹配、
  expires 在未来、policy 要求证据时 evidence_refs 非空、policy_version 已设置；失败降级 ask_human。
- STAY_SILENT/ASK_HUMAN：候选可为空；DEFER 需 reason。REQUEST_SKILL/REQUEST_SCENE_CHANGE：tool_plan 有界。
- 无 proposed 时按触发事件回退路由：有安全模板才 SPEAK，否则 stay_silent/ask_human，不编造内容。
- 注入集：少量标记（SQL/脚本/外链/私密索取）用于 INPUT 测试与检测。

## guard

`guard(request, policy, now) -> GuardResult` 对指定 stage：
- INPUT：注入/私密属性/越权场景；命中 block/escalate。
- EVIDENCE：每条证据 permission_revision 与权限快照 revision 一致、revision 非空或 original_verifiable、
  与候选 fact_versions 无冲突；缺失可核验版本按未知短 TTL 处理，不伪造版本。
- OUTPUT：金额/单位格式、承诺词表、引用必须出现在本次检索证据集合、外部 URL/代码拒绝。
- PREPLAY：审批未撤销未过期、事实版本匹配、epoch 匹配、权限快照未过期。
- REWRITE：提供 rewritten_candidate 与 rule_version；重写计数在 policy.max_rewrites 内，
  超限 block。基础守门不因缺失收费审核 API 被跳过。

## skill

`validate_descriptor(skill) -> Result`：名称/参数/结果 Schema 引用有界且非空、permissions 非空、
side_effect 分类 ∈ {read_only, notify, viewer_visible, high_risk}、risk 与 side_effect 一致
（viewer_visible→risk_2，high_risk 拒绝注册）、timeout/concurrency 有界、幂等声明。
`validate_tool_plan(plan, limits) -> Result`：步骤≤max_steps、预计时长≤max_ms，超限 ask_human；
参数 JSON 只允许受控资源 ID（无任意 URL/SQL/路径字段）；返回视为不可信。

## memory

`working_memory(records, max) -> Vec`：只保留已完成 utterance；未播完不写事实。
`session_summary(records) -> Summary`：确定性提取已讲主题/已回答问题/已批准承诺/恢复点字段。
`viewer_record_allowed(request, policy, now) -> Result`：默认关闭；需 scope/purpose/allowed_fields/
authorization_ref/retention/delete；禁止跨平台 identity、昵称当唯一 id、敏感画像字段。

## show

`evaluate_show(request, policy, now) -> Result<ShowPlan>`：scene/version/segment 非空，时长有界，
interrupt_policy ∈ {finish_segment, stop, cut}，敏感场景要求 evidence_binding；checkpoint 恢复需
checkpoint 与场景版本显式匹配，completed_utterance_ids 仅引用已实际完成句段。

## 验证范围

单元测试覆盖：决策降级/风险重算/回退、四阶段各命中与重写限次、技能注册拒绝与工具计划上限、
记忆未播排除与观众关闭、节目敏感绑定与 checkpoint。运行 knowledge/portable/契约/前端检查。
真实 LLM 守门插件、节目实际执行与记忆持久化消费不在本轮伪造。

## 2026-10-08 实施结果

`nai-kernel/src/evaluate/`（agent/guard/skill/memory/show）已实现，纯逻辑仅依赖 contracts/prost/serde_json。
agent 对无效候选降级 ask_human 并重算风险；guard 四阶段各命中/pass，重写限次 1；
skill 拒绝高风险/观众可见缺 permission 与任意 URL/SQL/路径；memory 未播内容不写事实、观众记忆默认关闭；
show 敏感场景要求证据绑定且 checkpoint 恢复显式版本化。新增集成测试覆盖完整决策→四阶段守门流水线
与 URL 单次重写后第二个 pass。kernel 测试 42 项、evaluate 集成 2 项、全 workspace 178 项通过。
实际智能体/守门插件由插件层实现，本轮不伪造；真实平台事件与媒体执行仍待 D11/D13。
