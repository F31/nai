# agent-harness Specification

## Purpose
把 V3.11 §40–§48 定义的**模型之外的工程化运行时**落成可判定的边界：上下文怎么装配、记忆怎么写读与遗忘、
意图怎么表示且为何不构成授权、工具循环怎么有界终止、语义缓存与 TTS 精确缓存为何必须分开、
MCP 与 A2A 两类外部接入各走什么入口、以及每一层失败如何具名降级并被观测。

本能力覆盖 `Agent = Model + Harness` 中 **Harness** 的那一半。它**不实现**任何 harness 代码，也**不引入**任何
第三方协议库：MCP 与 A2A 的协议版本与认证方式**待验证**，本能力只裁决边界与登记，MUST NOT 被读作「互操作已可用」。
四层判定（L1 装配、L2 工具、L3 编排与意图、L4 记忆）全部复用既有章节的机制，不新增运行实体——进程拓扑仍是 V3.11 §3.3 的六个。

## Requirements

### Requirement: Context is assembled in a fixed order under per-segment budgets

每次调用付费模型前，上下文 SHALL 由声明的装配步骤按**固定顺序**生成：系统策略、人设与场景、任务与输出契约、
当前事件与 Observation 摘要、已批准事实与检索证据、记忆片段、技能与工具描述、最近实际播完的句子、工具结果与外部返回。
前三个片段（策略、人设、任务契约）MUST NOT 被裁剪；预算不足时裁剪 MUST 只从后段发生，且 MUST 记录被裁项与原因。
Token 与字符 SHALL 同时计入，`remaining_budget_ms` 与 Token 预算是两个维度，MUST NOT 互相替代。
预算 SHALL **按段**分配，每个片段 MUST 有自己的配额，前三个片段还 MUST 带固定下限；除 Token 与字符外，
**条数**是第三个可判定的维度。阈值的**取值**由策略给定，MUST NOT 硬编码在本需求里，MUST NOT 采用外部观察值。
未定义段名、顺序错乱、不可裁段缺失或被裁、裁剪无原因或无被裁项、裁剪不落在可裁序列的后缀、缺配额、任一维度超限，
MUST 各自具名拒绝并指名涉及的片段。

#### Scenario: An uncroppable segment is missing

- **WHEN** 系统策略、人设或任务与输出契约任一片段缺失，或预算不足以容纳它们
- **THEN** MUST 拒绝调用该模型，MUST NOT 以「少几条策略继续生成」代替

#### Scenario: Budget pressure reaches the front segments

- **WHEN** 上下文超预算且裁剪落到前三个片段
- **THEN** 装配失败并具名报告，MUST NOT 静默裁掉策略或人设

#### Scenario: Cropping skips a front croppable segment

- **WHEN** 超预算触发的裁剪落在「已批准事实与检索证据」，而更靠后的「技能与工具描述」与「最近实际播完的句子」完好
- **THEN** 具名拒绝——「裁剪只从后段发生」意味着被裁的必须是**可裁序列的一个后缀**；
  一个只检查「有裁剪记录且记了原因」的实现会放行，故不满足本需求

#### Scenario: A crop that records neither items nor reason

- **WHEN** 某段被标记为已裁剪，但原因空白；或原因非空而被裁项为空
- **THEN** 分别具名拒绝，MUST NOT 让「说不清裁了什么」被读作已记录

#### Scenario: One total cap standing in for per-segment budgets

- **WHEN** 各片段共享一个总上限而自己不带独立配额
- **THEN** 具名拒绝，MUST NOT 以「总量仍在范围内」放行——本需求要求的是**按段**分配

#### Scenario: Only one dimension is recorded

- **WHEN** 某段只记录了 Token 用量而未记字符用量（或反之）
- **THEN** 具名拒绝——Token 与字符是两个维度，MUST NOT 互相替代

### Requirement: Every assembled fragment is sourced, scoped and reconstructible

每个进入上下文的片段 SHALL 携带来源类型（系统策略/场景/事件/知识证据/记忆/工具结果/外部智能体）、来源标识、作用域、时效（如适用），
以及**数据或指令**的标注。观众文本、检索片段、工具返回与外部智能体返回 MUST 标为数据，其内部出现的「忽略规则」「已批准」等措辞
MUST NOT 取得指令权。装配 SHALL 落决策日志（片段标识、各段预算占用、裁剪与原因、模型与提示词版本），且 MUST NOT 含明文凭据。
装配 SHALL 可由决策日志的片段引用重建，MUST NOT 依赖当时的内存状态。
来源标识 MUST NOT 为空白，作用域 MUST 携带（「时效」原文自带「如适用」，故其缺失 MUST NOT 被读作违规）。
重建 MUST **只**消费日志：日志缺该片段、或该片段的正文不在日志里而只存在于当时的内存，MUST 分别具名拒绝；
被引用片段在日志中的顺序与记录中的进入顺序不一致时，MUST 具名拒绝。判定的是「能否只靠日志重建」；
落链与运行期重建由 `runtime-composition` 承担，本需求 MUST NOT 被读作「已落链」。

#### Scenario: A fragment arrives without provenance

- **WHEN** 某片段缺少来源类型或作用域
- **THEN** 装配失败并指名该片段，MUST NOT 以空来源入上下文

#### Scenario: Rebuilding an assembly from the decision log

- **WHEN** 用决策日志中记录的片段引用重建某次调用的上下文
- **THEN** 重建结果的片段集合与各段预算占用 MUST 与原始记录一致（不要求逐字节相同，但要求同一片段集合与同一裁剪结论）

#### Scenario: An untrusted fragment is marked as an instruction

- **WHEN** 观众文本、检索片段、工具返回或外部智能体返回中的任一类被标为「指令」
- **THEN** 具名拒绝；这**四个**来源的清单 MUST 被逐项钉住，一个只判「有没有标注」的实现不满足本需求

#### Scenario: A fragment whose content was never logged

- **WHEN** 重建时某被引用片段的正文不在日志里，只能从当时的活动内存取得
- **THEN** 具名拒绝——「不依赖当时的内存」是本需求的判据，MUST NOT 用当前内存补齐后放行

#### Scenario: The log's own order is what decides

- **WHEN** 日志中被引用片段的排列顺序与记录中的进入顺序不一致
- **THEN** 具名拒绝；一个先按记录的进入顺序去排序再比较的实现会让本断言恒真，故不满足本需求

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
**R1 首版入站只响应本机与本地信任配置中已登记的受信对端**；对第三方开放 MUST 作为一次**显式的版本决策**，
MUST NOT 因请求自称可信或自称已获授权而放行。
对端身份 MUST 由本地信任配置派生，MUST NOT 取自请求自报；MUST 有作用域、速率限制与体积上限；入站请求 MUST 先过意图判定。
A2A 出站 MUST 声明目标、目的、数据范围与超时，MUST NOT 外发观众原文、知识原文或凭据；返回 MUST 按外部数据标注不可信。
出站 MUST NOT 委派 risk_2 及以上的动作，MUST NOT 委派需要凭据的动作；**R1 首版委派链深度上限 MUST 为常量、
MUST NOT 提供配置面**。

#### Scenario: An inbound peer poses as the user

- **WHEN** 入站请求声称「用户已批准」或自称低风险
- **THEN** 该声明无效力；请求按外部请求处理并进人工队列，MUST NOT 触发外发

#### Scenario: A third party asks to be admitted

- **WHEN** 一个**未登记**在本地信任配置里的对端发起入站请求
- **THEN** 拒绝并**具名**（MUST NOT 以「无响应」代替拒绝）；开放第三方入站 MUST NOT 由请求内容自行开启

#### Scenario: Outbound delegation escalates

- **WHEN** 委派被要求执行 risk_2 及以上动作，或委派链已达深度上限
- **THEN** 拒绝并记录，MUST NOT 继续委派

#### Scenario: Delegation depth is asked to be configured

- **WHEN** R1 首版试图把委派链深度上限做成可配置项（配置面或运行时参数）
- **THEN** 该做法被拒（上限 MUST 为常量）；配置化 MUST 先经一次显式的版本决策

#### Scenario: Credentials would be sent outbound

- **WHEN** 出站委派的数据范围包含凭据或观众原文
- **THEN** 拒绝该委派

### Requirement: Harness layers fail with named degradations and are observable

harness 每层 SHALL 产出可回放记录并进入只追加的哈希链决策日志：装配（片段来源与预算、裁剪与原因）、
意图（枚举、置信度、依据、与守门的冲突）、记忆（读写键、作用域、命中、删除完成状态）、工具（每步技能 ID 与版本、
参数哈希、结果摘要与大小、终止原因、成本）、缓存（命中与失效原因）。所有记录 MUST NOT 含明文凭据。
每层失败 MUST 有具名错误码与降级路径，并落到既有的降级顺序（模板 → 已批准缓存 → 人工 → 静音待机），
MUST NOT 静默降级为「空上下文的自由发挥」；降级 MUST 在能力视图与界面可见。
harness 新机制 MUST 默认只在**受监督自动以外的阶段**启用（默认启用集合按爬坡**位置**判定，MUST NOT 按阶段名枚举），
进入受监督自动前 MUST 通过**相关**黄金集回归。

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

### Requirement: A memory write carries its source, and only played approved content becomes a fact

每次记忆写入 SHALL 声明**写入来源**（已批准且已播完／已批准未播出／工具返回／外部智能体返回／评论原文／模型草稿）、
作用域、来源标识、时间，以及「叙述还是权威事实」的标注。只有**已批准且已播完**的内容可写为事实；
工具返回、外部智能体返回、评论原文与模型草稿 MUST NOT 写成事实或承诺，MUST 被**具名拒绝**。
只有**语义记忆**可承载权威事实，且其 MUST 带 §6 的发布引用。缺作用域、来源标识或时间的写入 MUST 被具名拒绝。

#### Scenario: A tool result, peer return, comment or model draft is offered as a fact

- **WHEN** 试图把工具返回、外部智能体返回、评论原文或模型草稿写成场次记忆中的事实或承诺
- **THEN** 写入被具名拒绝，并指向 §6 的导入与发布流程

#### Scenario: An approved but unplayed candidate is offered as a fact

- **WHEN** 某候选已批准但**尚未播完**却被写为事实
- **THEN** 写入被具名拒绝，MUST NOT 落库

#### Scenario: An authoritative fact lacks a publication reference, or sits outside semantic memory

- **WHEN** 某记录标为权威事实却没有 §6 的发布引用，或该记录不属于语义记忆
- **THEN** 写入被具名拒绝

#### Scenario: A write omits its scope, source reference or timestamp

- **WHEN** 写入缺少作用域、来源标识或时间中的任一项
- **THEN** 写入被具名拒绝，MUST NOT 以默认值补齐

### Requirement: Memory reads filter by scope before time, and every refusal is named

读取 SHALL **先按作用域**（workspace／session／观众作用域）过滤，**再按时效**过滤；过期、被撤销、权限不足的条目
MUST NOT 进入上下文，MUST NOT 用「近似条目」顶替。观众长期记忆 **默认关闭**；其读取 MUST 有授权引用并记录用途，
缺任一项 MUST 以「无记忆」继续，MUST NOT 以推断补齐。记忆段 SHALL 有独立配额，超配额 MUST 按
「**场次记忆 > 工作记忆 > 观众长期记忆**」的固定优先级裁剪。每一次拒绝（越作用域、过期、撤销、权限不足、
近似顶替、超配额）MUST 具名并保留记录引用，MUST NOT 静默丢弃。

#### Scenario: A read crosses scopes

- **WHEN** 某会话读取另一个 workspace、另一个会话或另一个观众作用域的记忆
- **THEN** 该条目被具名拒绝且不进入上下文，MUST NOT 返回近似条目顶替

#### Scenario: Expired, revoked or under-permitted records

- **WHEN** 命中记录已过期、已被撤销，或调用方权限不足
- **THEN** 该记录 MUST NOT 进入上下文，并以各自的名字出现在拒绝清单里

#### Scenario: Long-term viewer memory is off by default

- **WHEN** 未显式启用观众长期记忆，或读取时缺少授权引用或用途记录
- **THEN** 读取以「无记忆」继续并具名拒绝，MUST NOT 写画像、MUST NOT 以推断补齐

#### Scenario: The budget is exceeded

- **WHEN** 作用域与时效过滤后的条目超出记忆段配额
- **THEN** 按「场次 > 工作 > 观众长期记忆」裁剪，被裁条目 MUST 以名字出现在拒绝清单里，MUST NOT 静默截断

### Requirement: Memory forgetting is verifiable and claims no more than the purge achieved

TTL 到期、事实失效、权限撤销与用户请求四类触发的遗忘 SHALL 共用同一条删除链：**先阻断检索与读取，再异步清除副本**。
删除 MUST 可验证：读取侧 MUST 先失效，清理侧 MUST 给出完成状态。存在第三方留存时，MUST 在使用前明示该政策。
未完成清除却宣称已完全擦除、或存在第三方留存却宣称已完全擦除、或阻断态未给出留存说明，MUST 均被**具名拒绝**。

#### Scenario: The read side is blocked after the purge

- **WHEN** 某次遗忘的读取侧失效发生在清除之后，或从未发生
- **THEN** 拒绝，MUST NOT 记为已完成删除

#### Scenario: An erasure claim outruns the purge

- **WHEN** 清除未完成（或仍有第三方留存）却宣称已完全擦除
- **THEN** 拒绝该宣称并具名指出原因

#### Scenario: A third-party retention is not disclosed

- **WHEN** 清除受阻于第三方留存却没有给出留存说明
- **THEN** 拒绝，MUST NOT 让「说不清」被读成「已清完」

### Requirement: The memory surface exposes exactly the four operations of memory.access@1

`memory.access@1` 已由能力目录标为 `frozen`（与基线 §20.6 的「保持 Draft」不一致，**以目录为准**）。
记忆访问 SHALL 以四项操作 `write` / `read` / `forget` / `scopes` 表达，四项以外的操作名 MUST 被**具名拒绝**，
MUST NOT 被当作未知操作静默接受。四项操作的语义 SHALL 落进一致性套件，覆盖作用域越权的拒绝、过期与撤销的拒绝、
以及删除的可验证性。

#### Scenario: An operation outside the four is offered

- **WHEN** 请求携带四项之外的操作名
- **THEN** 具名拒绝，MUST NOT 静默接受或猜测其意图

#### Scenario: The four operations are exercised together

- **WHEN** 一致性套件依次执行 write、read、forget 与 scopes 的作用域完整性判据
- **THEN** 每一项都必须能在**不依赖真实存储**的前提下被构造与判定

### Requirement: Harness basics stay in the first release, the rest are capability packs

harness 的**基础纪律** MUST 留在首版门槛（R1）内：上下文装配与分段预算（H01、H02）、记忆写入门槛与作用域读取（H03、H04）、
观测入决策日志（H15）、逐层具名降级（H17），以及既有的模式爬坡门控（H18）。其余 SHALL 属 `capability_pack`：
意图识别（H05、H06）、语义缓存（H07、H08）、多步工具循环（H09、H10）、MCP（H11、H12）、A2A（H13、H14）、
harness 黄金集（H16）。分界判据 MUST 是「**一个确定性的上下文装配函数能不能满足它？**」。
降级 MUST NOT 删除该能力的契约边界、信任域归属与不变量（**INV-34**），MUST NOT 因降级使既有 `§NNN` 引用失效。

#### Scenario: A basic discipline is demoted to a capability pack

- **WHEN** 某一项被当作「可选子系统」后置（例如记忆写入门槛、逐层具名降级或模式爬坡门控）
- **THEN** 该后置 MUST 被拒绝：没有它默认组合**无法**正确调用模型，它不是增强项；分界判据 MUST 被重新适用

#### Scenario: A capability pack is enabled

- **WHEN** 一个能力包被安装或在默认组合中被启用
- **THEN** 它 MUST 先通过自身验收、MUST 走同一条升级与迁移图（MUST NOT 出现第二条安装路径），
  且 MUST NOT 改变默认组合的行为、成本或既有降级顺序；§5.9 的模式爬坡对它同样适用

#### Scenario: A demotion deletes a boundary

- **WHEN** 某个被降级的能力，其契约边界、信任域归属或不变量在降级过程中被删去，或使既有 `§NNN` 引用失效
- **THEN** 该改动 MUST 被拒绝——降级是**排期**动作，不是**设计**动作（INV-34）

### Requirement: Harness observations are decidable before they reach the log

harness 的五类观测（装配／意图／记忆／工具／缓存）SHALL 各有一组固定必需记录项，取自 §47.1 且不增不减。
一条记录进入只追加的哈希链决策日志之前 MUST 先被判定：必需项齐备、值内不含明文凭据（INV-11）、
观众原文与昵称已按 §5.8 保留期脱敏。任一项不满足 MUST **具名拒绝**并给出可落日志的已脱敏载荷，
MUST NOT 原样透传，也 MUST NOT 凭字段名猜测敏感性。五类 MUST 恰为五类，缺任一类 MUST NOT 被读作齐全。
判定的是「能否进日志与以何形态进」；真正把可落日志的记录追加进哈希链由 `runtime-composition` 的「A judged-loggable harness observation reaches the hash chain」需求承担，本需求 MUST NOT 被读作「已落盘」。

#### Scenario: A record carries a plaintext credential

- **WHEN** 某条观测记录的**值**内出现凭据字面量（如 `sk-…`）
- **THEN** 该记录被具名拒绝且不可落日志，MUST NOT 以「字段名不含 key」为由放行

#### Scenario: Only a field name looks sensitive

- **WHEN** 某字段名含 `key` 但其值不含任何凭据字面量
- **THEN** 该记录 MUST 被放行；按字段名黑名单拒绝的实现不满足本需求

#### Scenario: A required field is missing

- **WHEN** 某一层记录缺少 §47.1 为该层规定的必需项之一
- **THEN** 该记录被具名拒绝并指名缺失项，MUST NOT 以空值补齐

#### Scenario: Viewer text outlives its retention

- **WHEN** 观众原文或昵称未脱敏，或其保留期已过仍在记录中
- **THEN** 分别具名拒绝，MUST NOT 让「说不清」被读成「已脱敏」

### Requirement: Every harness layer failure lands on a named, ordered and visible degradation

harness 每层失败 SHALL 落到 §12.2 降级顺序（模板 → 已批准缓存 → 人工 → 静音待机）中的确定一级：
先在 §47.4 指定的首选级起取第一个可用级，静音待机恒可用。每层 MUST 有互异的具名错误码，
降级 MUST 在能力视图与界面可见（**不存在不可见的降级**）。装配层失败 MUST 拒绝该次调用，
且不因落点变化而解除拒绝；MCP/A2A 对端不可用 MUST NOT 影响已有会话。
当本次调用存在任一降级且装配后上下文失去策略或人设约束时，MUST 判为违规并 MUST NOT 继续生成（INV-32）。

#### Scenario: Assembly fails while a template is available

- **WHEN** 装配层失败（不可裁剪段缺失）且模板可用
- **THEN** 落**模板**级并**拒绝该次调用**；MUST NOT 生成无策略约束的回答

#### Scenario: Intent or cache fails while the template is available

- **WHEN** 意图不可用或低置信，或缓存不可用／失效，且模板与人工均可用
- **THEN** 落**人工**级；MUST NOT 沿用旧缓存引用，也 MUST NOT 因为模板可用就落模板

#### Scenario: The preferred level is unavailable

- **WHEN** 工具失败且模板不可用、人工可用
- **THEN** 落**人工**级；全部不可用时落**静音待机**；把一切映射到同一级的实现不满足本需求

#### Scenario: A degraded call would generate free-form

- **WHEN** 本次调用存在任一降级，且上下文已失去策略或人设约束
- **THEN** 判为具名违规并 MUST NOT 继续生成；仅给出错误码却仍继续生成同样不满足

### Requirement: Harness mechanisms ramp through stages, and supervised automatic mode is gated

harness 机制 SHALL 按 §5.9／§47.3 的四段爬坡（演练 → 影子 → 人工辅助 → 受监督自动）推进；
**默认启用集合 MUST 按爬坡中的位置判定**——受监督自动**之前**的全部阶段默认启用，受监督自动是唯一被门控的阶段。
该集合 MUST NOT 以**枚举阶段名**的方式表述：某条发布线未包含的阶段（如 R1 的影子）即**不存在**，
MUST NOT 被读作缺陷，也 MUST NOT 为对齐枚举而新增该阶段。
晋升到受监督自动 MUST 同时满足：相关回归已通过（MUST 声明受影响的类别、这些类别均已通过、且回归结论绑定**当前** harness 修订），
以及启用记录已备（指名同一机制与目标阶段、值内无明文凭据）。
任一条不满足 MUST **具名拒绝**并列出全部缺失项。判定的是「这次晋升是否被允许」；
把放行的启用记录追加进只追加哈希链由 `runtime-composition` 承担，本需求 MUST NOT 被读作「已落链」。

#### Scenario: A new mechanism arrives

- **WHEN** 一个 harness 新机制被引入
- **THEN** 它 MUST 只在受监督自动**之前**的阶段启用，MUST NOT 默认进入受监督自动

#### Scenario: A ramp stage has no contract landing point

- **WHEN** 询问各爬坡阶段对应的会话模式落点
- **THEN** 演练、人工辅助与受监督自动各有落点，而**影子在首版门槛（R1）内没有落点** MUST 被如实暴露，
  MUST NOT 被假装为已有；同时按本需求，**该缺失 MUST NOT 被读作缺陷**——默认启用集合由爬坡**位置**判定，
  不由阶段名枚举，故 R1 内不需要为影子新增 `SessionMode` 取值，也不需要把该阶段从 §5.9 的四段里删掉

#### Scenario: Promotion without the relevant regression

- **WHEN** 晋升到受监督自动，但受影响的回归类别未全部通过，或根本未声明受影响的类别
- **THEN** 具名拒绝并列出**全部**缺失类别，MUST NOT 只报第一个，也 MUST NOT 让「未声明」被读成「无相关回归」

#### Scenario: The harness changed after the regression passed

- **WHEN** 回归通过后 harness 修订发生变化（提示词、装配顺序、阈值、路由或技能清单）
- **THEN** 该回归不再使晋升生效并具名拒绝，MUST NOT 以「曾经通过过」放行

#### Scenario: Promotion without an enablement record

- **WHEN** 晋升到受监督自动却没有启用记录，或记录指名了别的机制或别的阶段
- **THEN** 具名拒绝；允许时 MUST 产出一条可落日志的启用记录载荷（含机制与目标阶段），
  MUST NOT 只返回「允许」而不产出记录

#### Scenario: The enablement record carries a credential

- **WHEN** 启用记录的**值**内出现凭据字面量
- **THEN** 具名拒绝；字段名含 `key` 而其值干净的记录 MUST 被放行——按字段名黑名单拒绝的实现不满足本需求

### Requirement: A closed viewer-memory capability is closed on the write face too, from one answering point

「观众长期记忆默认关闭」SHALL 由**唯一一处**判定，记忆的读取面、写入面与请求准入面 MUST 共用它，
MUST NOT 各自复写这个开关。未启用时，观众长期记忆的**写入** MUST 被具名拒绝，且拒因与读取面**相同**，
MUST NOT 出现「读取被拒、写入却被接受」的半覆盖。该判定的位置 SHALL 先于**来源与标注**的判定，
与读取面的「结构必填 → 能力开关 → 授权」同序。

#### Scenario: A viewer record is written while the capability is closed

- **WHEN** 未显式启用观众长期记忆，却试图写入一条观众长期记忆记录
- **THEN** 写入被具名拒绝，其名字与读取面的拒因**相同**，MUST NOT 被接受

#### Scenario: The capability is closed and the source is untrusted at the same time

- **WHEN** 一条候选同时命中「观众长期记忆未启用」与「来源不可信」
- **THEN** 报的是**能力未启用**——证明能力判定先于来源判定，且该顺序不是一句注释

#### Scenario: The capability is open and the candidate is compliant

- **WHEN** 观众长期记忆已启用，且候选的来源、作用域、来源标识与时间均已齐备
- **THEN** 写入通过；且该开关 MUST NOT 拒绝非观众类别的写入
