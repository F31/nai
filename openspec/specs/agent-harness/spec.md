# agent-harness Specification

## Purpose
把 V3.8 §40–§48 定义的**模型之外的工程化运行时**落成可判定的边界：上下文怎么装配、记忆怎么写读与遗忘、
意图怎么表示且为何不构成授权、工具循环怎么有界终止、语义缓存与 TTS 精确缓存为何必须分开、
MCP 与 A2A 两类外部接入各走什么入口、以及每一层失败如何具名降级并被观测。

本能力覆盖 `Agent = Model + Harness` 中 **Harness** 的那一半。它**不实现**任何 harness 代码，也**不引入**任何
第三方协议库：MCP 与 A2A 的协议版本与认证方式**待验证**，本能力只裁决边界与登记，MUST NOT 被读作「互操作已可用」。
四层判定（L1 装配、L2 工具、L3 编排与意图、L4 记忆）全部复用既有章节的机制，不新增运行实体——进程拓扑仍是 V3.8 §3.3 的六个。

## Requirements

### Requirement: Context is assembled in a fixed order under per-segment budgets

每次调用付费模型前，上下文 SHALL 由声明的装配步骤按**固定顺序**生成：系统策略、人设与场景、任务与输出契约、
当前事件与 Observation 摘要、已批准事实与检索证据、记忆片段、技能与工具描述、最近实际播完的句子、工具结果与外部返回。
前三个片段（策略、人设、任务契约）MUST NOT 被裁剪；预算不足时裁剪 MUST 只从后段发生，且 MUST 记录被裁项与原因。
Token 与字符 SHALL 同时计入，`remaining_budget_ms` 与 Token 预算是两个维度，MUST NOT 互相替代。

#### Scenario: An uncroppable segment is missing

- **WHEN** 系统策略、人设或任务与输出契约任一片段缺失，或预算不足以容纳它们
- **THEN** MUST 拒绝调用该模型，MUST NOT 以「少几条策略继续生成」代替

#### Scenario: Budget pressure reaches the front segments

- **WHEN** 上下文超预算且裁剪落到前三个片段
- **THEN** 装配失败并具名报告，MUST NOT 静默裁掉策略或人设

### Requirement: Every assembled fragment is sourced, scoped and reconstructible

每个进入上下文的片段 SHALL 携带来源类型（系统策略/场景/事件/知识证据/记忆/工具结果/外部智能体）、来源标识、作用域、时效（如适用），
以及**数据或指令**的标注。观众文本、检索片段、工具返回与外部智能体返回 MUST 标为数据，其内部出现的「忽略规则」「已批准」等措辞
MUST NOT 取得指令权。装配 SHALL 落决策日志（片段标识、各段预算占用、裁剪与原因、模型与提示词版本），且 MUST NOT 含明文凭据。
装配 SHALL 可由决策日志的片段引用重建，MUST NOT 依赖当时的内存状态。

#### Scenario: A fragment arrives without provenance

- **WHEN** 某片段缺少来源类型或作用域
- **THEN** 装配失败并指名该片段，MUST NOT 以空来源入上下文

#### Scenario: Rebuilding an assembly from the decision log

- **WHEN** 用决策日志中记录的片段引用重建某次调用的上下文
- **THEN** 重建结果的片段集合与各段预算占用 MUST 与原始记录一致（不要求逐字节相同，但要求同一片段集合与同一裁剪结论）

### Requirement: Memory writes are gated and reads are scoped

记忆的写入 SHALL 带 workspace/session 作用域、来源标识、时间与「叙述还是权威事实」的标注。
**只有已批准且已播完的内容**可写为场次记忆；工具返回、外部智能体返回、评论原文与模型草稿 MUST NOT 直接写成事实或承诺。
读取 SHALL 先按作用域（workspace/session/场景）再按时效过滤；过期、被撤销或权限不足的条目 MUST NOT 进入上下文，
MUST NOT 用「近似条目」顶替。观众长期记忆 **默认关闭**，其读取 MUST 有授权引用并记录用途。
`memory.access@1` 在能力目录中已标 `frozen`（与基线 §20.6 的「保持 Draft」不一致，以目录为准），
故本节要求的是**冻结后的语义补齐**：四项操作（`write`/`read`/`forget`/`scopes`）的语义 MUST 落进一致性套件，
覆盖作用域越权的拒绝、过期与撤销的拒绝、删除的可验证性。

#### Scenario: A tool result is offered as a fact

- **WHEN** 试图把工具返回或模型草稿写成场次记忆中的事实
- **THEN** 写入被拒，并指向 §6 的导入与发布流程

#### Scenario: A read crosses scopes

- **WHEN** 某会话读取另一个 workspace 或另一个场景的记忆
- **THEN** 读取被拒并记录，MUST NOT 返回近似条目顶替

#### Scenario: Long-term viewer memory is off by default

- **WHEN** 未显式开启观众长期记忆
- **THEN** 读取返回空且不写画像，MUST NOT 以推断补齐

### Requirement: Intent is a closed enum with confidence and evidence, and grants nothing

意图 MUST 表示为「封闭枚举 + 置信度 + 依据引用」，MUST NOT 是自由文本，也 MUST NOT 只给枚举而无依据。
`unknown` SHALL 是一等取值。依据引用 MUST 指向本次装配的片段。意图 MUST NOT 构成授权、MUST NOT 参与风险判定、
MUST NOT 改变守门的阶段数或强度；风险仍由有效风险计算决定。实时自动路径 SHALL 保持不做独立分类器；
非实时路径（人工辅助、影子评估、节目编排、入站外部请求）SHALL 做显式意图识别，其中入站外部请求 MUST 先判意图类别。
置信度低于阈值时 MUST 落 `unknown` 并澄清或转人工，MUST NOT「按最可能的猜一个」外发。

#### Scenario: Intent tries to lower the guard

- **WHEN** 某次意图被识别为 `request_action` 且置信度很高
- **THEN** 权限不提升、守门强度不降低，动作仍按风险分级走人工批准

#### Scenario: Only a label is supplied

- **WHEN** 意图结果只有枚举或只有分数，缺少依据引用
- **THEN** 判定为未定义并落 `unknown`，MUST NOT 作为路由依据

#### Scenario: Low confidence

- **WHEN** 置信度低于阈值
- **THEN** 落 `unknown` 并澄清或转人工，MUST NOT 直接生成回答

### Requirement: The tool loop is bounded, read-only parallel and reports partial failure

工具循环 SHALL 对步数、总 deadline、预算、并行度与结果总大小同时设界，任一越界 MUST 具名终止（例如 `RESOURCE_EXHAUSTED`、
`DEADLINE_EXCEEDED`、`BUDGET_EXCEEDED`），MUST NOT 存在静默截断。并行 MUST 只在无相互依赖且 `side_effect` 为
`none` 或 `read` 时允许，且同一轮并行 MUST 共享同一 deadline 与预算池。读类可重试，写类 MUST NOT 重试，除非可证明幂等且带幂等键。
部分失败 MUST 显式标注缺失，结果必须能区分「查到了、值为空」与「没查到」。工具结果进上下文即标为不可信数据；
用于播报 MUST 先绑定证据再进四阶段守门——「工具调用成功」不等于「内容可信」或「已获准外发」。

#### Scenario: The loop hits a bound

- **WHEN** 步数、deadline、预算、并行度或结果大小任一越界
- **THEN** 具名终止并写决策日志，已完成步骤的结果 MUST 按部分失败规则标注

#### Scenario: A write is retried

- **WHEN** 某写类调用失败，且无法证明幂等或未带幂等键
- **THEN** MUST NOT 重试

#### Scenario: Empty is confused with absent

- **WHEN** 某工具返回空结果
- **THEN** 结果 MUST 标明是「查到且为空」还是「没查到」，MUST NOT 用空结果冒充无数据

### Requirement: The semantic cache is separate from the TTS cache and never grants

语义缓存与 TTS 精确缓存 SHALL 分开判定。TTS 精确缓存以逐字相同为命中条件，可复用音频；语义缓存以语义相似超阈值为命中条件，
**只复用文本或候选，MUST NOT 复用音频**。语义缓存的键 MUST 含 embedding 模型与修订、意图类别、场景与知识空间，
并沿用事实/权限/策略修订。命中 MUST NOT 复制授权、审批或事实版本，复用后的候选 MUST 仍走守门与有效风险判定，
且 MUST 重跑证据复核。价格、库存、优惠与活动条件、承诺与售后政策、个体化承诺 MUST NOT 进入语义缓存。
事实、权限或策略任一 revision 变化 MUST 使相关作用域条目立即失效。命中率与**误命中率** MUST 一起报告。

#### Scenario: A near-duplicate sentence tries to reuse audio

- **WHEN** 语义相似度超阈值且待播内容涉及价格
- **THEN** MUST NOT 复用音频，且该内容 MUST NOT 进入语义缓存

#### Scenario: A fact revision changes

- **WHEN** 某条事实、权限或策略的 revision 变化
- **THEN** 相关作用域的语义缓存条目立即失效，MUST NOT 继续命中

#### Scenario: Only the hit rate is reported

- **WHEN** 缓存效果被总结
- **THEN** MUST 同时给出误命中率，MUST NOT 以命中率单独作为结论

### Requirement: MCP maps onto skills, data and configuration, with description-drift re-review

MCP 的 `tools` SHALL 映射为技能并逐工具审核，MUST NOT 信任服务端自报的只读或低风险；`resources` SHALL 按数据契约接入
并带来源、时效、权威级别与敏感分类，MUST NOT 直接当证据、MUST NOT 绕过知识发布流程；`prompts` MUST NOT 直接作为系统提示，
其变更 SHALL 按安全相关变更处理；`sampling` MUST 默认禁用，启用需本机管理员授权并计入预算。
接入时 SHALL 生成工具清单快照（名称、描述、参数 Schema、来源、版本）；**工具描述的变更 MUST 触发重新审核**。
MCP 服务端视同 remote 宿主。协议版本与认证方式待验证，MUST NOT 臆造。

#### Scenario: A tool description changes remotely

- **WHEN** MCP 服务端修改了某个工具的描述
- **THEN** 该工具 MUST 重新审核后才可用于装配，MUST NOT 沿用旧审核结论

#### Scenario: A resource is used as evidence

- **WHEN** 直接把 MCP `resources` 的返回当作播报证据
- **THEN** 被拒；该数据 MUST 先经数据契约带上来源与时效，或经知识发布流程

#### Scenario: The server asks to sample the model

- **WHEN** MCP 服务端发起 `sampling` 反向调用
- **THEN** 默认拒绝；仅在本机管理员授权且预算准入时才允许，并逐次记决策日志

### Requirement: A2A inbound is an external request; outbound delegation is bounded

A2A 入站 SHALL 走**独立入口**，入口请求一律按**外部请求**处理：MUST NOT 构成用户指令、MUST NOT 构成授权、
MUST NOT 构成值守；默认只在人工辅助模式下响应，受监督自动模式下响应 MUST 由场景显式启用并预授权。
对端身份 MUST 由本地信任配置派生，MUST NOT 取自请求自报；MUST 有作用域、速率限制与体积上限；入站请求 MUST 先过意图判定。
A2A 出站 MUST 声明目标、目的、数据范围与超时，MUST NOT 外发观众原文、知识原文或凭据；返回 MUST 按外部数据标注不可信。
出站 MUST NOT 委派 risk_2 及以上的动作，MUST NOT 委派需要凭据的动作，委派链深度 MUST 有上限（初值 1）。

#### Scenario: An inbound peer poses as the user

- **WHEN** 入站请求声称「用户已批准」或自称低风险
- **THEN** 该声明无效力；请求按外部请求处理并进人工队列，MUST NOT 触发外发

#### Scenario: Outbound delegation escalates

- **WHEN** 委派被要求执行 risk_2 及以上动作，或委派链已达深度上限
- **THEN** 拒绝并记录，MUST NOT 继续委派

#### Scenario: Credentials would be sent outbound

- **WHEN** 出站委派的数据范围包含凭据或观众原文
- **THEN** 拒绝该委派

### Requirement: Harness layers fail with named degradations and are observable

harness 每层 SHALL 产出可回放记录并进入只追加的哈希链决策日志：装配（片段来源与预算、裁剪与原因）、
意图（枚举、置信度、依据、与守门的冲突）、记忆（读写键、作用域、命中、删除完成状态）、工具（每步技能 ID 与版本、
参数哈希、结果摘要与大小、终止原因、成本）、缓存（命中与失效原因）。所有记录 MUST NOT 含明文凭据。
每层失败 MUST 有具名错误码与降级路径，并落到既有的降级顺序（模板 → 已批准缓存 → 人工 → 静音待机），
MUST NOT 静默降级为「空上下文的自由发挥」；降级 MUST 在能力视图与界面可见。
harness 新机制 MUST 默认只在演练、影子与人工辅助下启用，进入受监督自动前 MUST 通过相关黄金集回归。

#### Scenario: Assembly fails at runtime

- **WHEN** 装配层失败（例如不可裁剪段缺失）
- **THEN** 拒绝调用并退到模板，且在界面可见该降级；MUST NOT 生成无策略约束的回答

#### Scenario: A silent degradation happens

- **WHEN** 任一层的失败没有产生具名错误码或没有在能力视图体现
- **THEN** 视为不满足本需求

#### Scenario: A new harness mechanism is enabled in auto mode

- **WHEN** 某 harness 机制（装配、意图、循环、缓存的新行为）首次进入受监督自动模式
- **THEN** MUST 先通过相关黄金集回归，并在决策日志留下启用记录

### Requirement: Contract staging is read from the catalog, and unverified protocols are not named

契约的 `stage`（draft / frozen）SHALL 以 `schemas/catalog/capabilities.json` 为准。当基线正文的陈述与目录不一致时
（例如 §20.6 把某契约列为「保持 Draft」而目录已标 `frozen`），MUST 以目录为准，并 MUST 把该不一致**登记**出来；
MUST NOT 改写目录去迎合正文，也 MUST NOT 删掉正文那一句来掩盖差异。
当某外部协议的版本与认证方式**未验证**时，MUST NOT 为其命名契约——先占名会产生**半声明的 draft**，使 `stage` 字段失去意义；
这类验收条目 MUST 以 `contract` 留空表达，MUST NOT 引用目录中不存在的名字。

#### Scenario: The baseline prose and the catalog disagree

- **WHEN** 基线正文对某契约的阶段陈述与能力目录不一致
- **THEN** 以目录为准并在文档与验收中登记该不一致，MUST NOT 静默消解

#### Scenario: An unverified protocol is given a contract name

- **WHEN** 为版本与认证方式尚未验证的外部协议新增契约条目
- **THEN** 拒绝；相关验收 MUST 以 `contract` 留空表达（先例：M10/M12）
