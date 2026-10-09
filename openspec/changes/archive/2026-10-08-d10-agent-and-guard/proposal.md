# D10 智能体决策、四阶段守门与技能/记忆/节目评估

## Why
V3.5 第 21/23 章要求 Observation→Decision 路由、INPUT/EVIDENCE/OUTPUT/PREPLAY 四阶段守门、
有界技能/记忆/节目评估与节目恢复。内核目前只有注册/会话/调度，没有可移植的决策与守门评估逻辑。
需要可测试的纯评估内核，不被插件实现替换或绕过，且不伪造授权/事实/风险。

## What Changes
- nai-kernel 新增 evaluate：agent（决策路由与候选校验）、guard（四阶段守门与有限重写）、
  skill（描述符/工具计划校验）、memory（工作/场次/观众记忆规则）、show（节目/恢复校验）。
- 决策动作 speak/stay_silent/ask_human/request_skill/request_scene_change/defer；
  风险由受控规则/事实计算，模型自报低风险不是最终判定。
- INPUT 注入集与隐私/越权检查；EVIDENCE 权限/版本/冲突；OUTPUT 金额单位/引用/承诺限制；
  PREPLAY 审批/事实/epoch/授权复检。重写限次（初值1），不无限递归。
- 技能只允许受控资源 ID 与有界步骤；观众可见内容需 risk_2；高风险不注册。
- 记忆默认无观众长期画像；场次摘要可先确定性提取；未播内容不写为事实。
- 节目版本化 checkpoint 恢复，不能凭空续播；评估为纯内核逻辑，实际守门插件在 D10 验证。

## Capabilities
### New Capabilities
- `agent-guard-evaluation`: 决策路由、四阶段守门、技能/记忆/节目评估的纯内核实现。
