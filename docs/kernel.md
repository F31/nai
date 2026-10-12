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
- `selling::evaluate_pitch`：§53 基础讲品循环的确定性规则——段落四段与顺序、检查点只含段落与已完成句、
  段落级冷却与最小切换间隔、受约束断言须绑定当前有效事实版本。
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

## W66 harness 观测与逐层具名降级（V3.9 §47.1、§47.4；验收 H15/H17）

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

**边界（如实）**：H15/H17 **保持 `pending`**——本轮交付的是**判定面**，
把记录真正追加进 `nai-storage` 的哈希链、以及把 `DegradeVerdict` 渲染进 `CapabilityView` 与界面，
都属 runtime 与呈现层接线；故报告 MUST NOT 出现「已进决策日志」的措辞。

## W67 harness 爬坡门控（V3.9 §5.9、§47.2、§47.3；验收 H18）

落在 `evaluate::ramp`（**不是**并入 `evaluate::harness`）：§47.1/§47.4 判的是**一次调用**，
§47.3 判的是**一个机制 × 一个阶段**，两个维度不同；合并会让「这次拒绝属于哪一类」重新不可判定
（与 W66 不复用 `bridge::Degradation` 同一条理由）。也**不**复用 `session::change_mode`：
那是**会话维度**（值守租约、二次确认，且已实现 `AutoSupervised → Assisted` 回落），本模块是**机制维度**
——同一会话里不同 harness 机制可以处于不同爬坡阶段。

- **四段爬坡（§5.9）**：`RampStage` = 演练 → 影子 → 人工辅助 → 受监督自动。
  `DEFAULT_ENABLED_STAGES` 照抄 §47.3 的「默认只在演练、影子与人工辅助启用」⇒ **受监督自动是唯一被门控的阶段**。
- **六类回归（§47.2）**：`RegressionCategory` = 装配回归／意图拒识／工具失败恢复／缓存误命中／
  MCP 描述漂移／A2A 越权委派。晋升须**声明受影响类别**（`touched`）且 `passed ⊇ touched`；
  **`touched` 为空**判 `regression_scope_undeclared`——否则一段空声明即可绕过门控。
- **回归失效**：§47.2 的「通过相关回归**才生效**」⇒ 回归结论绑定 harness 修订，
  修订变化判 `regression_stale`，不认「曾经通过过」。
- **启用记录（§47.3）**：必须存在、指名同一机制与目标阶段、值内无明文凭据（INV-11，
  复用 `harness::CREDENTIAL_MARKERS`）；允许时给出 `record_to_append`。
  **不**校验「是否真的已追加进哈希链」——那是 `nai-storage` 的只追加写入。
- `RampRejection` 收录 **8** 个具名拒绝；`tests/ramp.rs`（**13 项**）含五组**能区分两种实现**的守卫。

**照实登记的发现**：§5.9／§47.3 是**四段**，但契约 `SessionMode` 只有**三个**取值、**没有 `Shadow`**，
且 §1.4／§17.3 把「影子模式」列在 **R2** ⇒ **H18 的默认启用集合里有一个阶段在 R1 内没有落点**。
`RampStage::session_mode()` 对 `Shadow` 返回 `None`（不伪造映射），并由
`every_stage_but_shadow_has_a_session_mode` 钉死。本轮**不改基线文本**，作为待确认项交下一轮裁定。

**边界（如实）**：H18 **保持 `pending`**——按阶段真正启用/停用机制属 runtime 接线；
六类回归的执行体是 **H16（能力包）**，本轮只判「是否已记录为通过」，**不**跑用例。

## W68 基础讲品循环（V3.9 §53、§5.3、§5.7、§5.4、§45.3；新增验收 B07）

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

**基线偏差（如实登记）**：新增 B07 后 §48.5 的「B | B01–B06」与 §57.4 的计数行「= 87」不再等于实际值；
本轮**不改基线文本**（那是升基线轮次的事且 MUST 是 1:1 行替换），留待下一轮一并收录。

