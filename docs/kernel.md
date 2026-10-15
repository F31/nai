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
- `memory::working_memory/session_summary/viewer_record_allowed/viewer_memory_gate`：工作记忆只保留已播完内容；
  场次摘要在主题/已回答/承诺/恢复点做确定性去重；观众长期记忆默认关闭，需作用域/用途/授权/保留/删除
  （「默认关闭」的判定只有 `viewer_memory_gate` **一处**，读取面/写入面/请求准入面共用，W89）。
- `selling::evaluate_pitch`：§53 基础讲品循环的确定性规则——段落四段与顺序、检查点只含段落与已完成句、
  段落级冷却与最小切换间隔、受约束断言须绑定当前有效事实版本。
- `show::evaluate_show`：时长/打断政策有界；敏感场景要求证据绑定；checkpoint 恢复必须显式匹配场景版本。
- `assembly::evaluate_assembly`：§41 上下文装配的两半——九段固定顺序与不可裁段 1–3、裁剪只从后段发生且须记录
  被裁项与原因、分段预算 token/char/items 三维；并由决策日志单独重建同一片段序列（不依赖当时内存）。

示例：`cargo test -p nai-kernel` 覆盖决策降级、四阶段守门、技能注册拒绝、记忆未播排除与节目恢复。

## W57 记忆读写面（V3.11 §42.2–§42.6；验收 H03/H04）

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

`MemoryRejection` 收录 **23** 个具名拒绝，`label()` 互不相同；`tests/memory.rs`（**23 项**，W89 起）断言
「构造出的拒绝集合**恰为** `MemoryRejection::ALL`」（既证每条可构造，也防未登记的名字），并含正向不变量
（合规候选零拒绝）与**顺序断言**（同时越作用域又过期的记录报 `out_of_scope` ⇒「先作用域后时效」不是注释）。

**边界（如实）**：H03/H04 **保持 `pending`**——判据描述的是**运行时行为**，本轮交付的是判定面。
⚠️ **已由 W107／W109 订正（写入面与读取面均已接上真实存储）**：
`admit_write`（H03 写入面）由 W107 接上 `Composition::write_memory`——先跑判定，放行才落 `memory_records`、
拒绝则具名且一列不落（`Store::record_memory` 只做存储层的事，不重跑判定）。
`read_records`（H04 读取面）与 §42.4 遗忘链的**阻断**落点由 W109 接上：`Composition::read_memory`
**先**取数、**再**逐条**诚实派生**三个条目级输入——`revoked` ← `memory_records.revoked_at_ms`
（`0008-memory-revocation.sql` 新增的可空列，NULL = 未撤销）、`permitted` ← 该记录**所属类别**要求的
授权是否在场（观众记忆看记录自己的 `authorization_ref`，其余类别 §42.3 无记录级授权要求 ⇒ 结构蕴含）、
`approximate_of` ← 恒 `None`（存储里结构上不存在顶替条目）；`Composition::forget_memory` 则
**先**写撤销列阻断读取、**再**判这次遗忘的**陈述**是否如实。
⚠️ **已由 W110 订正（控制面接线已建）**：两个入口的**生产调用方**由 W110 补上——
`nai-control-api` 定义接缝 `MemorySurface`（定义在 shared 层，因为分层禁止 shared 依赖 runtime），
`nai-runtime` 以 `MemoryAdapter` 实现它，**app 层（engine）注入**；
`POST /api/v1/memory/read`（权限 `memory.read`）与 `POST /api/v1/memory/forget`（权限 `memory.forget`）
经它调到装配根。作用域维度由**控制面自身的绑定**给出、请求里出现同名字段一律忽略；
判定**只在一处**（内核），控制面与适配器都只转发。
**已由 W111 订正（清除侧已从「字段」变成「动作」）**：W111 给 `ForgetOutcome` 增 `body_purged`
——它**从存储读出**（`Store::memory_body_purged_at`），不是调用方能填的字段；并新增拒因
`ErasedClaimWithoutPurge`（「已完全擦除」而载荷一次都没清过）。`forget()` 的检查顺序
（Complete → body_purged → 留存）由**可达性**决定：反过来排会让
`ErasedClaimWithoutCompletePurge` 不可达。清除执行体 `Composition::purge_memory`
先阻断、再清载荷、最后清点副本清单 `COPY_SITES`（**当前为空集**，且由代码派生的落点集合钉住）。

**仍未接线**的是：Pro 跨会话（§42.5：授权/审批/租约/设备身份不跨场继承）；
远端留存清除（§24.4 知识链那套 `RemoteRetention`，与记忆记录不共用落点）。
见 [ADR 0085](adr/0085-memory-write-gate-wiring.md)、[ADR 0087](adr/0087-memory-read-surface.md)、
[ADR 0088](adr/0088-memory-control-surface.md)、[ADR 0089](adr/0089-memory-purge-surface.md)。

## W66 harness 观测与逐层具名降级（V3.11 §47.1、§47.4；验收 H15/H17）

落在 `evaluate::harness`（**不是**新建顶层模块）：`evaluate/` 的模块契约就是纯逻辑判定，且 `memory.rs`
的头注释已引用 INV-32——本轮是同一条不变量的后半段，放同处才不会出现两个都声称实现 INV-32 的模块。
**不复用** `bridge::Degradation`：那个只服务 §119 的平台中断（`bridge_lost`／`auto_reply_revoked`），
与 harness 六层是两件事，合并会让「这次降级属于哪一类」重新不可判定。

- **观测（H15）**：`ObservationLayer` 五类（装配／意图／记忆／工具／缓存）各带 `required_fields()`，
  逐项照抄 §47.1；`evaluate_observation` 判「能否进日志、以何形态进」——必需项齐备、值内无凭据字面量
  （INV-11，**扫值不扫字段名**）、观众原文已脱敏且在 §5.8 保留期内。输出 `redacted` 是已脱敏载荷，
  凭据值替换为 `[redacted]`、超期观众文本**丢弃**而非仅标记。
- **降级（H17）**：`FailureLayer` 六类各带互异 `code()` 与 `preferred_level()`；`evaluate_degradation`
  从首选级起沿 §12.2 四级（模板 → 已批准缓存 → 人工 → 静音待机）取第一个可用级，静音待机恒可用故必有落点。
  装配层置 `refuses_call`（**不因落点变化而解除拒绝**）；对端不可用置 `keeps_existing_session`。
  `visible` 恒为真——**不存在不可见的降级**。
- **INV-32 后半段**：`assert_no_free_form` 判「已降级且上下文失去策略或人设约束」为具名违规。
  只给错误码仍继续生成，同样判违规。

`ObservationRejection` 收录 **5** 个具名拒绝；`tests/harness.rs`（**24 项**）含四组**能区分两种实现**的守卫，
每组都用注入验证过确实会失败：

| 注入 | 被抓住的测试 |
|---|---|
| 把凭据扫描改成字段名黑名单 | `a_field_named_key_with_a_clean_value_passes` 等 3 项 |
| 把所有失败压到第一级 | `intent_and_cache_land_on_human_even_when_the_template_is_available` |
| 「命名了就继续生成」 | `a_degraded_call_without_its_policy_constraint_is_a_violation` |
| 悄悄删掉 `crop_reason` | `required_fields_match_the_baseline_table` |

最后一条是本轮**补出来的缺口**：在此之前 `required_fields()` 的内容没有任何测试钉住 §47.1，
删一项不会有任何测试失败；故新增一条把五类字段表**逐项写死**的断言。

**边界（如实）**：H15/H17 **保持 `pending`**——W66 交付的是**判定面**；观测侧的**落链**已由 W103 补上
（[ADR 0081](adr/0081-observation-log-wiring.md)：`crates/nai-runtime/src/observation.rs` 判定后追加脱敏载荷或具名拒绝），
但五类观测的**真实产生者**与把 `DegradeVerdict` 渲染进 `CapabilityView`／界面仍属接线，故未达标的措辞只对后者成立。

## W67 harness 爬坡门控（V3.11 §5.9、§47.2、§47.3；验收 H18）

落在 `evaluate::ramp`（**不是**并入 `evaluate::harness`）：§47.1/§47.4 判的是**一次调用**，
§47.3 判的是**一个机制 × 一个阶段**，两个维度不同；合并会让「这次拒绝属于哪一类」重新不可判定
（与 W66 不复用 `bridge::Degradation` 同一条理由）。也**不**复用 `session::change_mode`：
那是**会话维度**（值守租约、二次确认，且已实现 `AutoSupervised → Assisted` 回落），本模块是**机制维度**
——同一会话里不同 harness 机制可以处于不同爬坡阶段。

- **四段爬坡（§5.9）**：`RampStage` = 演练 → 影子 → 人工辅助 → 受监督自动。
  `DEFAULT_ENABLED_STAGES` 列出前三段——它与 §47.3 **等价但不逐字相同**：V3.10 §58-E4 把基线表述改为
  「默认只在**受监督自动以外的阶段**下启用」（按 §5.9 爬坡中的**位置**判定，不枚举阶段名），
  而前三段恰为受监督自动之前的阶段 ⇒ **受监督自动是唯一被门控的阶段**；**代码侧一字未改**。
- **六类回归（§47.2）**：`RegressionCategory` = 装配回归／意图拒识／工具失败恢复／缓存误命中／
  MCP 描述漂移／A2A 越权委派。晋升须**声明受影响类别**（`touched`）且 `passed ⊇ touched`；
  **`touched` 为空**判 `regression_scope_undeclared`——否则一段空声明即可绕过门控。
- **回归失效**：§47.2 的「通过相关回归**才生效**」⇒ 回归结论绑定 harness 修订，
  修订变化判 `regression_stale`，不认「曾经通过过」。
- **启用记录（§47.3）**：必须存在、指名同一机制与目标阶段、值内无明文凭据（INV-11，
  复用 `harness::CREDENTIAL_MARKERS`）；允许时给出 `record_to_append`。
  **本模块不校验「是否真的已追加进哈希链」**；把放行的启用记录追加进链**已由 W105 落地**
  （`crates/nai-runtime/src/ramp.rs`，见 `docs/runtime.md`、[ADR 0083](adr/0083-ramp-enablement-log-wiring.md)）。
- `RampRejection` 收录 **8** 个具名拒绝；`tests/ramp.rs`（**13 项**）含五组**能区分两种实现**的守卫。

**照实登记的发现**：§5.9／§47.3 是**四段**，但契约 `SessionMode` 只有**三个**取值、**没有 `Shadow`**，
且 §1.4／§17.3 把「影子模式」列在 **R2** ⇒ **H18 的默认启用集合里有一个阶段在 R1 内没有落点**。
`RampStage::session_mode()` 对 `Shadow` 返回 `None`（不伪造映射），并由
`every_stage_but_shadow_has_a_session_mode` 钉死。本轮**不改基线文本**，作为待确认项交下一轮裁定。
**该待确认项已由 W71 裁定（E4：与阶段枚举解耦）**：默认启用集合改为按**位置**判定 ⇒ 影子在 R1 内**只是不存在**，
MUST NOT 为对齐枚举而新增该阶段；`SessionMode` 仍三取值、§1.4／§17.3 的影子归属不变。见 V3.10 §58.2 的 E4、§58.4。
**H18 仍 `pending`**——E4 只消除**文本不对称**，不宣称达标。

**边界（如实）**：H18 **保持 `pending`**——把放行的启用记录追加进链**已由 W105 落地**
（`crates/nai-runtime/src/ramp.rs` 的 `record_promotion`，见 `docs/runtime.md`）；
但**按阶段真正启用/停用机制**仍属 runtime 接线，**六类回归的执行体**是 **H16（能力包）**——
本模块只判「是否已记录为通过」，**不**跑用例。

## W68 基础讲品循环（V3.11 §53、§5.3、§5.7、§5.4、§45.3；新增验收 B07）

§53 把「基础讲品循环」前移 R1 并自称「本版最重要的一条」，但它此前在 `tests/registry.yaml` 里**没有任何条目**
（§48.5 的家族索引写「B | B01–B06」、§57.3 的 R1 清单无对应项，全仓检索命中数为 0）⇒ R1 的**门槛定义**里有它、
**验收清单**里没有它。本轮补上判定面与条目。

落在 `evaluate::selling`（**不**并入 `evaluate::show`）：`show.rs` 判的是**单个 `ShowPlan` 自身**是否合法
（场景/版本 id 一致、时长在范围、引用不超上限、打断策略在已知三值、检查点**存在性**、敏感场景证据绑定），
本模块判的是**跨段落**的编排规则；两者输入形状与失败类别都不同。也**不**并入调度面——那里的「冷却」
是观众/任务维度的（单观众占比上限、欠服务阈值），这里的冷却是**段落**维度的。

- **段落编排**：`SEGMENT_ORDER` = 卖点 → 规格 → 适用场景 → 售后说明，逐项照抄 §53；
  判**序列**不判集合（排序后比较集合的实现会放行「规格在卖点之前」）；
  未定义段名 / 缺段（**指名**缺哪些、不只报数量）/ 乱序各自具名，且**不短路**——缺段与乱序可同时报出。
- **插问后归位（§5.7）**：`Checkpoint` 只允许「所指段落 + 已完成句引用」；`carried_text` 非空即
  `checkpoint_holds_unspoken_text`。该字段是**建模选择**：没有它，「带了未播出文本」在类型上无法表达，
  守卫会恒真。恢复位置必须等于 `checkpoint.segment`。
- **段落级冷却与最小切换间隔**：两条**分开**判定（同段重复进入 vs 任意相邻两次进入），
  阈值由 `PitchPolicy` 给定、模块**不内建**（§48.7 待本场景回放实测，MUST NOT 采外部观察值）。
  进入时刻序列须非降序；前置条件失败**具名**且区间规则不再评估——间隔在递减序列上没有定义，
  直接相减会下溢（这也顺带堵死一类实现缺陷）。
- **事实约束的行动提示**：`CLAIM_KINDS` = §45.3 禁用域五类，逐项照抄；`unbound_claims` 与 `stale_claims`
  **分开**——只校验「引用非空」的实现会让三天前的价格通过。

`PitchRejection` 收录 **10** 个具名拒绝；`tests/selling.rs`（**19 项**）含五组**能区分两种实现**的守卫，
每组都做过注入验证。**边界（如实）**：B07 **保持 `pending`**——本轮是判定面；
段落内容与话术、TTS 时长与回放阈值属内容创作与运行时，§5.7 的「恢复前重新校验产品、价格、活动、素材与授权」
由守门与知识层执行 ⇒ **MUST NOT** 称「讲品循环已可运行／已可恢复」。

**基线偏差（如实登记）**：新增 B07 后 §48.5 的「B | B01–B06」与 §57.3 的计数行「= 87」不再等于实际值；
本轮**不改基线文本**（那是升基线轮次的事且 MUST 是 1:1 行替换），留待下一轮一并收录。**该偏差已在 W71 收录**：V3.10 §48.5 为「B | B01–B07 | 第 16 章、53」、§57.3 计数为 `r1` **37** / 合计 **88**（§58.2 的 E1–E3）。

## W69 上下文装配（V3.11 §41.1–§41.5、§47.1、§47.4；验收 H01/H02）

H01/H02 是 W56 立的项目标（§47.6 的 W56 行写的正是「上下文装配器」）。此后 W57/W66/W67 先后落下
记忆读写面、harness 观测与逐层降级、harness 爬坡门控，**这两条一直停在「有名称、有 registry 条目、无实现」**。
一处极易误判：`harness.rs` 里 `Assembly` 出现十余次，但每处都是**层名或字段名**
（`ObservationLayer::Assembly`、`FailureLayer::Assembly`、必需字段 `segment_budget`/`cropped_items`/`crop_reason`），
**没有一处判定一次装配是否合法**（W68 报告里「R1 七条已全部有判定面」的说法因此不成立，本轮一并订正）。
另外 `agent-harness` 能力里**已有**这两条需求 ⇒ 本轮走 `MODIFIED`，**不是**新建能力、**不是** `ADDED`。

落在 `evaluate::assembly`（**不**并入 `harness`，也**不**并入 `selling`）：`harness` 判的是「一条记录能否进日志、
一次**失败**该落到 §12.2 哪一级」，`selling` 的「四段」是 §53 的讲品段落、是**另一条轴**；
本模块判的是 §41 的 L1 信息边界。合并会让「这次拒绝属于哪一类」重新不可判定——
与 W66 不复用 `bridge::Degradation`、W67 不复用 `session::change_mode` 同一条理由。

- **九段与顺序（§41.1）**：`SEGMENT_ORDER` = 系统策略 → 人设 → 输出契约 → 事件摘要 → 证据 → 记忆 → 技能 →
  最近播完句 → 工具结果，逐项照抄；顺序存在的理由写在 §41.1——**约束不被挤掉**（所以前三是不可裁，且裁剪倒着走）。
  判**序列**不判集合；未定义段名与乱序各自具名；**不短路**——缺段与乱序同时成立时两条都报。
- **不可裁段**：1–3 缺失即拒，且**按名列出**缺了哪几段（不只报数量）。
  `Segment::uncroppable()` 由 `UNCROPPABLE_COUNT = 3` 派生，**不**另写一张硬编码清单。
- **裁剪只从后段（§41.1／§41.2）**：预算不足时从后往前裁 ⇒ 被裁的可裁段必须构成**后缀**。
  「证据被裁、而更靠后的技能与播完句完好」这种形状不可能由倒序行走产生——所以只判「有裁剪记录、有原因」不够。
  裁剪还须**同时**记录被裁项与原因，缺任一项各自具名。
- **分段预算（§41.2）**：token／字符／条数**三维**、逐段判定；**token 与字符互不替代**，缺一维即拒；
  超限时**报出是哪一维**（`BudgetDimension`）；不可裁段另须有 floor。阈值**由调用方给**、模块不内建——
  §41.2 的初值只是起点、须经 §47.2 回放调优并记入 ADR，§48.7 更禁止采外部观察值。
- **片段标注（§41.3）**：七类来源逐项照抄；「**观众文本、检索片段、工具返回、外部智能体返回**一律标为数据」
  由 `SourceKind::must_be_data()` 表达，另三类 §41.3 无此约束、本模块**不判**（既不放行也不禁止）。
  作用域是**必填**（§41.3 未加限定），时效是「如适用」⇒ 缺失**不算违规、也不判**。
  `Trust` **不是** `Option`：缺标注在类型上不可表达，于是只有**标错**才需要拒绝。
- **可重建（§41.5／H02）**：`rebuild_assembly` 只消费**决策日志**，按**日志自身顺序**排出被引用片段再与记录比较。
  按记录顺序排序后比较会让「日志顺序与记录不同」**恒真** ⇒ 顺序断言形同注释（注入已验证）。
  `FragmentNotLogged`（根本没记）与 `FragmentContentNotInLog`（记了，但正文没落盘、只存了指向当时内存的指针）
  **分开具名**——H02 要禁的正是后者。明文凭据复用 `harness::CREDENTIAL_MARKERS`（INV-11，不建第二张表）。

`AssemblyRejection` 收录 **14** 个具名拒绝、`RebuildRejection` 收录 **6** 个；`tests/assembly.rs`（**36 项**）
含两条「恰为 `ALL`」总账、**逐项写死**的基线表守卫（九段／七类来源／不可裁序号／四类一律为数据），
以及**跨模块**守卫「装配记录字段 == `harness` 的 L1 必需字段表」（改一张不动另一张即失败），
并含七组**能区分两种实现**的注入验证：

| 注入 | 被抓住的测试 |
|---|---|
| 把顺序判定改成集合比较 | `the_right_segments_in_the_wrong_order_are_refused` 等 3 项 |
| 缺段只报第一个 | `two_missing_uncroppable_segments_are_both_named` |
| 忽略未定义段名 | `an_undefined_segment_name_is_refused` 等 2 项 |
| 去掉「裁剪须成后缀」检查 | `cropping_that_skips_a_front_croppable_segment_is_refused` 等 2 项 |
| 把六条重建拒绝合成一条 | `a_fragment_whose_body_was_not_logged_is_refused` 等 2 项 |
| 重建时先按记录顺序排序 | `a_log_whose_order_differs_from_the_record_is_refused` 等 2 项 |
| 悄悄改一个字段名 | `the_assembly_record_fields_match_the_harness_l1_table` |

**边界（如实）**：H01/H02 **保持 `pending`**——W69 交付的是**判定面**。把预算初值真正回放调优并记入 ADR、
把装配记录追加进 `nai-storage` 的哈希链**已由 W104 落地**、把 §41.1 的**判定**接上同一入口并补齐 §41.5 的**四项**（`segment_usage`／`crops`）**已由 W106 落地**（`crates/nai-runtime/src/assembly.rs` 的 `record_assembly`／`rebuild_assembly_from_log`／`read_logged_assemblies`，见 [ADR 0082](adr/0082-assembly-log-wiring.md)、[ADR 0084](adr/0084-assembly-judgement-wiring.md)）；
执行 §47.2 的六类回归（能力包 **H16**）与把九段片段**喂给模型**仍属证据生产侧 ⇒ 报告 MUST NOT 出现「装配器已接入运行期」「已不依赖当时内存」这类运行期断言。

**复核方法上的一处坑（本轮实测）**：注入脚本用 `fs.copyFileSync` 复原源码时，**Windows 会连同 mtime 一起复制**
⇒ 复原后的源码 mtime 比注入构建更旧，cargo 判定「未变更」而**复用上一次注入的二进制**，
于是「基线」跑出的是上一组注入的结论（本轮曾两次把这类陈旧结论误读为真缺陷）。
改用 `fs.writeFileSync(内容)` 并核对内容哈希后，前后两次基线均为 **36 通过 / 0 失败**。

**本轮不新增 registry 条目**（H01/H02 早已在册）⇒ **不产生新的基线偏差**；W68 登记的 B07 偏差仍未收录（**已在 W71 收录**，见 V3.10 §58.2 的 E1–E3）。

## W70 瞬态宿主条件（V3.11 §5.9、§12.2、§20.6、§46.3；装配根与会话共享）

本轮**不是**「清 flaky CI」的运维动作。触发是 W69 收尾时登记的两族 CI 红，核实后是**两个可机械证明的缺陷**，
其中一个**是安全缺陷**。W48 报告 §5 第 2 条早把这件事留给单独一轮（原句：「Windows 侧 `nai-runtime` 的时序脆弱未修…需单独一轮」）。

判据（写进规格，可判定）：**宿主的瞬态条件（锁竞争、慢盘）MUST NOT 改变被测系统的结论。**

- **共享会话锁上的 `try_lock` 不得被当作故障**：`ControlApi` 与装配根共享同一个 `Arc<Mutex<Session>>`，
  两侧都 fail-fast；`DrillGuard::tick` **每 5 ms** 调 `live_epoch()`，旧实现以 `?` 上抛取锁错误
  ⇒ 一次**微秒级、会自清除**的竞争被当成故障 ⇒ 闭包返回 `Err` ⇒ **`supervise` 结束监督循环**
  ⇒ 此后 keeper 被杀**再也不被检测与撤权**。`Session::revoke` 自身只剩 `actor_allowed`（本场景恒 `true`）
  ⇒ 「撤权被跳过」**只有取锁一条机械通道**，链条因此是唯一可能的原因。
  还有一处巧合式掩盖：既有规格的「续租失败不得静默退出」只有**一次性补偿 `halt`** 满足，
  而那个出口用的是**同一条** fail-fast 取锁 ⇒「到底撤没撤权」取决于第二次取锁运气。
- **修法 = 一个预算一份定义（ADR 0027）**：撤权/故障处置（`observe`/`control`/`enforce_duty`）用
  `SESSION_REVOKE_BUDGET` = **250 ms** 有界重试，耗尽仍是**显式 `Conflict`**（撤权**要么落地、要么被报告**，
  **绝不静默跳过**）；epoch 轮询用 `SESSION_POLL_BUDGET` = **5 ms**，耗尽返回 **`Ok(None)` = 「本 tick 未观测到」**，
  **不是故障**。`live_epoch()` 签名变 `Result<Option<u64>>`：`Some` 正常／`None` 竞争／**`Err` 仅锁中毒**；
  `SessionLockMiss { Busy, Poisoned }` 就是为此存在。
- **bring-up 期限要按工作量标定**：`Store::open` 在**全新数据根**上跑**全链迁移**（`VERSION = 7`，7 个脚本，
  每段 `BEGIN IMMEDIATE`+`COMMIT`），连接以 **`synchronous=FULL`** 打开，却曾用**逐次调用**的 `SHORT_OPERATION` = 5 s
  ⇒ 慢盘上 `DEADLINE_EXCEEDED`。ADR 0004 的**上界是 30 s**（`Operation::new` 拒绝 `> 30s`），
  同仓同类迁移工作一律 30 s ⇒ 新增 `BRINGUP_OPERATION` = **30 s**（`pub`，`apps/engine` 的 `--provision` 复用同一常量，
  一处定义）；`SHORT_OPERATION` **保持 5 s**。失败文案**指名步骤 + 已耗时**（否则下一读者看不出哪一步慢）。
- **刻意的不改**：`apps/engine/src/pipeline.rs` 的两处 5 s **保持原样**——它们在**已打开的 store** 上读、**不含迁移**
  ⇒ 这是判据的**结果**，不是遗漏。

**被否决的三个替代**（写进 design，各自给理由）：①把 `session_lock` 一律改阻塞 `lock()`——把宿主竞争换成无限等待、
控制面 `status` 会挂住；②只在测试夹具里等——被修的是**产品代码对瞬态条件的处理**，不是调度假设；
③`--test-threads=1`——把「并发下必须成立」降级成「串行下成立」，是**放宽**而非**修**。

**明确不做**：不改断言、不放宽窗口、**不用 sleep 掩盖**、不改 CI workflow、不降 `--test-threads`、
不改 `apps/engine/tests/*` 任何一行、**不新增契约名**、**不新增 registry 条目**、不改任何 `status`、不改基线文本。

测试全部落在**既有**目标内（`crates/nai-runtime` 的 lib 单测 + `tests/composition.rs`）⇒ **不新增测试目标**：
3 条行为测试（持锁 20 ms 期间 `control(Mute)` 仍返回 `1`；持锁 30 ms 期间 `live_epoch()` 为 **`None` 而非 `Err`**；
**正向前置**「撤权后 epoch 确实变化」——否则「恒报未观测到」也能变绿）+ 2 条策略钉死（逐项钉死四个常量与三条不等式；
在**新建临时目录**上真调 `open_bringup_store` 并断言 `remaining() > SHORT_OPERATION`）。
**持锁时序用 mpsc 握手**确保持锁**先于**被测调用，**不用 sleep 竞猜**。五组非空洞注入各自被抓住、源文件按**内容哈希**复原为 `IDENTICAL`：
去掉重试截止／把 `Ok(None)` 改回 `Err`／`BRINGUP` 退回 5 s／`bringup()` 换 `short()` 常量／
`open_bringup_store` 内换 `short()`。**第五组是发现「只钉常量对『常量对、调用点接错』是瞎的」后补的**——
把 `open_bringup_store` 提成 `prepare` 真正调用的函数再让它进测试。

**边界（如实）**：`apps/engine/tests/engine.rs` 是 `#[cfg(target_os = "linux")]` ⇒ **本机 Windows 门控不能证明 Linux 侧修复**，
只能**以 CI 结论为准**；family 1 的链是**从源码读出的、不是从失败日志读出的** ⇒ **不主张**已复现当时失败，
只主张「链存在且被消除」；H 系与所有 `status` 一律不动。**本轮不新增条目 ⇒ 不产生新的基线偏差**
（W68 的 B07、W67 的「影子在 R1 无落点」仍待升基线轮次收录——**两项均已在 W71 收录**，见 V3.10 §58.2 的 E1–E4）。

## W89 观众长期记忆开关的单一来源与写入面（V3.11 §5.5；PD-31）

「观众长期记忆**默认关闭**」这条开关在仓库里被写了三处，W89 实测后**收成一处判定**：

- **`memory::viewer_memory_gate(policy)`**：这条开关的**唯一判定处**（返回 `viewer_memory_disabled`）。
  读取面 `read_records`、写入面 `admit_write` 与请求准入面 `viewer_record_allowed` 都**调用它**，
  MUST NOT 各自复写 `if !policy.viewer_memory_enabled`（一条开关答两次就会分家）。
  legacy `viewer_record_allowed` 保留其 `EvaluateError` 契约，只把 gate 的拒绝**映射**成 `Unauthorized`
  ——**映射不是第二处定义**。
- **写入面此前是空的**（本轮补的执行体）：`admit_write` 原先只收 `&MemoryCandidate`、**没有策略** ⇒
  默认关闭下一条 `kind = "viewer"` 的候选**会被接受**（实测改前返回 `Ok(())`）。
  本轮给它加 `policy` 入参，并把能力判定放在**必填项之后、来源判定之前**——与读取面
  「**结构必填 → 能力开关 → 授权**」同序；理由是能力未启用**不依赖条目内容**即成立（能力级），
  而「来源不可信」是条目级。**顺序有断言**（同时「未启用 + 不可信来源」⇒ 报能力未启用）。
- **开关 MUST NOT 塞进 `MemoryCandidate`**：让被审对象自带「能力是否启用」，就是 W57 §四.1 已排除的
  同一反模式（门槛交给被审对象自己填）。
- **拒因复用 `viewer_memory_disabled`**，不新造名字（同一件事 MUST NOT 有两个名字）⇒ `MemoryRejection::ALL`
  仍 **23**，既有「构造出的集合**恰为** `ALL`」的总账断言**不放宽**。
- **非空洞注入**：F0/F1 改前/改后对照（**真跑改前那一行**：默认策略下 viewer 候选改前 `Ok(())`、
  改后 `Err(viewer_memory_disabled)`）、F2 承重（移出写入面 gate ⇒ 2 failed）、F3 顺序断言、
  F4 反向不红（启用后合规候选通过、非观众类别不受影响）、F7 反转 gate ⇒ 读取面与写入面
  **6 failed 同时变红**（证明两面确实共用一个判定）。`tests/memory.rs` **21 → 23**。

**边界（如实）**：本轮修的是**判定面自足性**，**不是运行时接线**——`admit_write`／`read_records`
在生产代码里**无调用方**（H03／H04 仍 `pending`，原因与 W57 §五一致）⇒ MUST NOT 读成
「默认关闭已生效于产品」。另有一处**配置面死键**本轮只登记未改：`schemas/config/runtime.schema.json`
的 `session_defaults.viewer_memory_enabled` 是 **`required`** 键，而全仓非测试代码只读过同层的
`allow_auto_without_platform_grant`（`apps/engine/src/pipeline.rs:874`）⇒ 改它**无任何效果**。
