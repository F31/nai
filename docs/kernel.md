# Kernel：注册、闭包与绑定

```bash
cargo xtask test --suite kernel
cargo xtask check --profile portable
```

suite 实际测试 nai-kernel/nai-plugin-sdk；kernel只依赖contracts。
实现入口为 `Registry`、`CallContext`、`Limits`，SDK入口为 `parse_manifest`。

## 输入及可信边界

1. SDK 验证 `schemas/plugin/manifest.schema.json`、kernel semver range、OS/arch、正式能力。
   64KiB输入上限、deadline/cancellation；输出生成 PluginRegistration。
2. 可信 root 提供独立 Admission；它必须引用已核验artifact/manifest/host/review/资源/许可。
   这不是插件自报可信，普通插件不能获得Registry owner管理接口。
3. register 检查身份/协议/目标/声明并拒绝循环，初始仍是Validated，没有自动健康或权限。
4. D05宿主提供有来源的健康观测；权限由认证的root按session/plugin更新，未知默认拒绝。

本轮没有真实插件、宿主、验签器或认证档案，因此空生产Registry仍为NotReady。
inproc准入必须是root已知编译/审核实现，不加载任何通用DLL。

## 场景计划

`schemas/config/binding-plan.schema.json` 为配置；调用内核使用生成 BindingPlan。
它包含 session、mode、required/optional capabilities、显式主备selection与policy_scope。

- 普通路由不隐式依赖知识/LLM；按请求和声明形成闭包。
- 外发的output/tts必需guard，avatar.render必需profile/guard；不自动降成未授权音频/字幕。
- 缓存路线可以不请求在线tts，但仍通过guard/output。
- 自动模式附加平台/知识能力及独立权限、监督期限、预算材料。真实来源由后续任务实现。
- 唯一provider可默认选，多provider必须明确选择；不按哪个“当前能响应”擅自切换。
- 同域显式备用成功为Degraded；缺失必需为NotReady，可选缺失为Degraded并列原因。

`prepare`只计算结果，不启动任何未选或选中插件。依赖顺序确定，失败候选的暂存闭包被丢弃。

## 冻结与复检

Ready/Degraded可 `freeze`，生成scope/version/artifact/manifest/host齐全快照；同session重复freeze拒绝。
被引用包的换版/移除须先结束使用并由可信root release；更新健康/权限和撤销准入可立即生效。
`recheck`对原快照和原选择复检，篡改、跨session、过期或撤销均失败，不重写成备用。
备用身份列表只是policy准入候选，使用仍需重新闭包/健康/权限检查；不是旧租约延续。

CapabilityView 未选实例不会被标Active。Ready是D04逻辑结果，不能当成Session Running、
输出设备可用、素材许可已实测或Guard/租约/平台最终授权。
D06还必须复检epoch、审批hash、事实和媒体lease。

## 使用约束

- 每个启动实例提供不同boot_id，不从插件请求读取owner/principal/当前时钟。
- registry不是资源预算治理器；结构Limits不能替代GPU/API/output认证。
- registry方法不得让未认证插件自行admit、更新权限或release另一个session。
- D04不创建进程/网络，不宣称P0或Native沙箱已实现；这些进入D05/D06。

## D10 智能体/守门评估

`nai-kernel::evaluate` 提供纯逻辑的可移植评估，仅依赖 contracts/prost/serde_json；
实际智能体/守门插件在插件层实现，内核守门不因插件缺失被绕过。

- `agent::evaluate_decision`：Observation→Decision 路由；speak 候选必须完整且 epoch/期限匹配；
  风险由受控规则/关键词重算，模型自报低风险不是最终；无效候选降级 ask_human/stay_silent，不编造内容。
- `guard::guard`：INPUT（注入/私密属性/越权）、EVIDENCE（权限版本/来源版本/有效期）、
  OUTPUT（金额单位/承诺/引用必须在本场检索）、PREPLAY（审批/事实/epoch/权限/期限复检）。
  重写限次（初值1），URL/脚本单次消毒重写，超限 block；缺失收费审核不会跳过基础守门。
- `skill::validate_descriptor/validate_tool_plan`：高风险不注册；观众可见需 risk_2 与 output.viewer 权限；
  工具计划步骤/时长有界，参数只允许受控资源 ID，任意 URL/SQL/路径拒绝；结果视为不可信数据。
- `memory::working_memory/session_summary/viewer_record_allowed`：工作记忆只保留已播完内容；
  场次摘要在主题/已回答/承诺/恢复点做确定性去重；观众长期记忆默认关闭，需作用域/用途/授权/保留/删除。
- `show::evaluate_show`：时长/打断政策有界；敏感场景要求证据绑定；checkpoint 恢复必须显式匹配场景版本。

示例：`cargo test -p nai-kernel` 覆盖决策降级、四阶段守门、技能注册拒绝、记忆未播排除与节目恢复。

## W57 记忆读写面（V3.9 §42.2–§42.6；验收 H03/H04）

在 D10 的评估函数之上补齐四块，**既有四个函数与 `MemoryPolicy` 一字不改**；纯逻辑，无 I/O、无存储、无新增依赖。

- **写入（H03）**：`WriteSource`／`ClaimKind`／`MemoryCandidate`／`admit_write`。只有**已批准且已播完**可写为事实；
  工具返回、外部智能体返回、评论原文、模型草稿一律**不得**写成事实或承诺（其自称「已批准·只读」无效，不按 `claim` 放行）；
  作用域、来源标识、时间是**必填项**；权威事实只落在语义记忆且须带 §6 的发布引用。
- **读取（H04）**：`MemoryScope`／`MemoryQuery`／`ScopedRecord`／`MemoryReadOutcome`／`read_records`。
  顺序固定为「**作用域 → 时效 → 撤销 → 权限 → 近似顶替 → 配额**」；过期、被撤销、权限不足与顶替条目
  **一个都不进上下文**，且每一条都以名字出现在 `refused` 里（**拒绝不静默**）。超配额按
  「**场次 > 工作 > 观众长期记忆**」裁剪，**被裁的条目同样具名**。观众长期记忆默认关闭，
  读取需授权引用 + 用途记录，缺任一项以「无记忆」继续、不写画像、不以推断补齐。
- **遗忘（§42.4）**：`ForgetTrigger`（TTL／事实失效／权限撤销／用户请求）／`PurgeState`／`ForgetOutcome`／`forget`。
  四类触发共用同一条链：**先阻断检索与读取、再异步清除副本**；宣称不得超出实际——未清完不得宣称擦除、
  有第三方留存不得宣称擦除、受阻态必须给出留存说明。
- **四操作（§42.6）**：`MemoryOperation` 只接受 `write`/`read`/`forget`/`scopes`，其余是 `unknown_operation`，
  不静默当作未知操作。

`MemoryRejection` 收录 **23** 个具名拒绝，`label()` 互不相同；`tests/memory.rs`（**21 项**）断言
「构造出的拒绝集合**恰为** `MemoryRejection::ALL`」（既证每条可构造，也防未登记的名字），并含正向不变量
（合规候选零拒绝）与**顺序断言**（同时越作用域又过期的记录报 `out_of_scope` ⇒「先作用域后时效」不是注释）。

**边界（如实）**：H03/H04 **保持 `pending`**——判据描述的是**运行时行为**，而真实记忆子系统（持久化、控制面接线、
Pro 跨会话）未建；本轮交付的是判定面。§42.5 的跨会话边界（授权/审批/租约/设备身份不跨场继承）**本轮未覆盖**。

