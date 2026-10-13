# spec-driven-delivery Specification

## Purpose
让 nai 的需求、实现和验收证据在仓库内可追溯，以**当前设计基线**为准（版本号只在 `docs/design.md` 与最新基线 ADR 处声明，本规格不写死版本），通过 OpenSpec 的增量规格和任务记录驱动每轮开发，避免聊天计划与真实工程状态分离。

## Requirements

### Requirement: Specification before implementation

开发流程 SHALL 在实现每轮 Dxx 任务前创建 OpenSpec proposal、需求场景、design 和 tasks，引用**当前设计基线**（`docs/design.md` 指明的版本）及相关 ADR；基线变更必须有 ADR，OpenSpec 不得隐式覆盖基线。

#### Scenario: Start a development task
- **WHEN** 开始一个包含功能或工程行为变化的任务
- **THEN** 存在可校验的变更规格及未勾选的实现/验证清单，再开始编码

#### Scenario: A specification pins a stale baseline version
- **WHEN** 某规格把某一具体历史版本（如 V3.5）写成「统一设计基线」，而该版本已被后续 ADR 取代
- **THEN** 该陈述 MUST 被就地订正为**不写死版本号**的形式（以 `docs/design.md` 与最新基线 ADR 为准）；
  「纯追加」MUST NOT 被用来让这类陈述长期存续

### Requirement: Evidence-based completion
开发流程 SHALL 仅在对应实现与验证完成后勾选任务，更新开发计划和报告，分别记录实现、实测、Blocked、NotApplicable 与未开始。

#### Scenario: External environment unavailable
- **WHEN** Linux 检查通过但尚无 Windows 设备或真实 SDK/API/GPU 测试结果
- **THEN** 报告保留环境边界，不将这些外部认证记录为 Passed

### Requirement: Validated spec lifecycle
项目 SHALL 提供锁定版本的 OpenSpec CLI；变更和主规格必须通过严格校验，已完成变更经校验后归档并合并增量规格，保留可追溯任务与报告。

#### Scenario: Complete and archive a change
- **WHEN** 变更任务全部完成且实际检查通过
- **THEN** 使用标准 OpenSpec 归档操作更新主规格，归档文件保留完整任务和证据链接

### Requirement: Corrections distinguish stale statements from provenance

基线与其活文档的就地订正 SHALL 只作用于**与当前事实不符的陈述**，并按 W65 先例分**四种情形**：
① 声明「**当前基线是哪一份**」的位置（`docs/design.md`、`README.md`、`AGENTS.md`、`openspec/config.yaml`、
输出串与界面文案）MUST 随基线更新，或改为**不写死版本号**（§56-C9 先例）；
② **`§NNN` 的版本前缀是行号契约的锚点**（解析为**当前基线**），故 MUST 随基线更新；
③ **关于某一版的陈述**（升基线链条、该版「收缩/新增/晚了什么」、按该版实测的数字）与**该版专属的版本章节**
MUST NOT 随之改写；④ 代码 doc-comment 的版本前辍 MUST NOT 改。
②与④的分界是**载体**：doc-comment 记归属，输出串与错误消息是**当前值**。
四情形 MUST 被分别识别，MUST NOT 以「版本号出现即改」的规则批量替换。

#### Scenario: A baseline version bump sweeps live documents

- **WHEN** 基线由 Vx 升为 V(x+1)，活文档中同时存在「唯一实现基线为 Vx」「该条款见 Vx §NNN」与「Vx 收缩了什么」三类陈述
- **THEN** 情形①与② MUST 被改写（或去掉版本号），情形③ MUST 保留原文；
  MUST NOT 只做前两者，也 MUST NOT 把三者一并替换

#### Scenario: A provenance reference is swept into a false claim

- **WHEN** 一次「订正」把**关于某一版**的陈述（情形③）一并改写——例如「V3.9 收缩」→「V3.10 收缩」，
  或「源自 V3.8 新增第四部分」→「源自 V3.10」
- **THEN** 该改写 MUST 被拒绝：归属变成了不实陈述，而订正的目的恰恰是消除不实陈述
- **AND** 与之相邻的**章节指针**（情形②，如「V3.9 §51.3」）MUST 反而被更新为当前基线——
  两者 MUST 被分别识别，MUST NOT 以「这一段里有版本号」为理由一并处理

#### Scenario: An earlier draft of this rule is still quoted as the rule

- **WHEN** 某处（规格、活文档或 ADR 的追加段）仍写着「**版本归属引用**（形如「V3.9 §51.3」）MUST NOT 随基线号变更」
  这类**已被 W65 先例否证**的表述——W71 的 E5 首版判据即此形状，其残留散落在规格与两处 ADR 追加段
- **THEN** 该表述 MUST 被视为**待订正的陈述**并按上列四情形改写：
  规格（活的主规范）就地订正；`docs/reports/`、`docs/adr/`、`openspec/changes/archive/` 保留原文 + **追加勘误段**

### Requirement: Registered deviations do not outlive a baseline version

「如实登记之后 MUST 紧接着订正现行规范」中的**「紧接着」SHALL 被读作「不晚于下一次基线修订轮次」**：
已登记的偏差 MUST NOT 跨基线版本存活，其判定形式是台账条目的 `since_baseline` MUST 等于当前基线版本。
MUST NOT 被读作「同一轮内」（不可行），也 MUST NOT 被读作「N 轮之内」（轮数是外部观察值）。
跨版处置只有两种：**收录**，或**显式结转**（改版本 + 记结转轮次 + 非空理由）。

#### Scenario: An infeasible deadline is not enforced as written

- **WHEN** 某偏差在实现轮次内登记，而该轮**不是**升基线轮次（基线被 ADR 批准后不可变，
  就地订正又有 MUST 1:1 行替换的硬约束 ⇒ 改 §57.3 只能发生在一次专门的升基线轮次里）
- **THEN** MUST NOT 因「未在同一轮内订正」而判为违规——那条读法没有可执行的落点；
  判据 MUST 是「**下一次**基线修订轮次是否处置了它」

#### Scenario: A deviation is carried across a baseline upgrade in silence

- **WHEN** 一次升基线之后，台账里仍有 `since_baseline` 指向旧版本的条目，而该条目既未被收录、
  也未被显式结转
- **THEN** 门控 MUST 失败并指名该条目与两侧版本；正确做法是把「这一版故意不收它」**写下来**
  （改 `since_baseline` + 记结转轮次 + 非空理由）——沉默与记录是两个不同的状态，
  本条只禁止前者

### Requirement: Pending decisions carry a disposition and a basis

基线各章「待决策与待确认」的条目 SHALL 被登记进一份机器可读台账（`tests/pending-decisions.yaml`）。
每条 MUST 有**封闭枚举的处置**与**一条非空洞依据**：`answered_by_baseline` MUST 给 `basis`，
其中每个 `§NNN` MUST 真实存在于当前基线；`decidable_in_repo` MUST 给 `recommendation`；
`needs_precondition` MUST 给一组**真实存在**的前置（验收 id 或工作项编号）；
`needs_external` MUST 给 `closure`（取到什么算闭合）；`out_of_scope` MUST 给 `note`。
「待裁定」MUST NOT 被读作「无人负责」。

#### Scenario: A pending decision is answered by the baseline itself

- **WHEN** 某条待裁定项在基线正文里已有答案（如打包粒度已由 §51.3 的能力包清单回答）
- **THEN** 它 MUST 被登记为 `answered_by_baseline` 并给出该 `§NNN`；MUST NOT 被重新裁定，
  MUST NOT 继续留在「待裁定」里悬空

#### Scenario: A basis points at a section that does not exist

- **WHEN** 某条 `answered_by_baseline` 的 `basis` 引用一个当前基线里不存在的章节号
- **THEN** 校验 MUST 失败并指名该条目与那个章节号——指向不存在章节的依据与没有依据是同一件事

#### Scenario: An external item has no closure condition

- **WHEN** 某条 `needs_external` 缺 `closure`，或 `closure` 为空
- **THEN** 校验 MUST 失败——外部条件无法被「完成」，只能被「以什么为标志闭合」

#### Scenario: A precondition names something that does not exist

- **WHEN** 某条 `needs_precondition` 的前置写成 registry 里没有的验收 id，或基线里没有的工作项编号
- **THEN** 校验 MUST 失败并指名它——把待裁定项挂在不存在的东西上，等于把它挂在了空气上

#### Scenario: The ledger silently loses an item

- **WHEN** 台账少登记了基线列的条目（总数或某一章的新增数低于基线事实）
- **THEN** 校验 MUST 失败并给出两侧计数——覆盖性 MUST 由「基线各章的新增条数」**逐章写死**保证，
  MUST NOT 由「台账自己说自己全」保证

### Requirement: Every field that can carry a section reference is checked

待裁定台账的引用完整性 SHALL 覆盖**每一个可能承载 `§NNN` 的字段**——`basis`、`introduced_in`、
`also_in`、`recommendation`、`closure`、`note`；扫描域 MUST 由一份**单一清单**派生，MUST NOT 在
循环里手写。校验器自身的两张常量表 MUST 互相一致：处置枚举与必填字段 MUST **一一对应**。

#### Scenario: A reference in a field outside the scanned set

- **WHEN** 任一承载 `§NNN` 的字段（含 `recommendation`／`closure`／`note`）引用一个当前基线里
  不存在的章节号
- **THEN** 校验 MUST 失败，并**指名该条目、该字段与那个章节号**——漏掉字段里的一条引用，
  与没有这条断言是同一件事

#### Scenario: A disposition without a required field

- **WHEN** 处置枚举里出现一个在必填字段表里没有对应项的处置
- **THEN** 校验 MUST 失败并指名该处置——否则它的条目会「没有必填字段」地通过，而校验对它沉默

#### Scenario: A required field that no check covers

- **WHEN** 某个处置的必填字段既不在引用扫描清单里、也不在 id 校验清单里，或一个字段同时出现在两处
- **THEN** 校验 MUST 失败并指名该字段——否则它在两个检查之间落空，且没有任何信号

### Requirement: The ledger's field set is closed and fully classified

待裁定台账的**字段集** SHALL 在**两个层级上各自封闭**：**信封**（台账顶层）出现的每个键 MUST ∈
已声明集合 `TOP_LEVEL_FIELDS`，**条目**出现的每个键 MUST ∈ 已声明集合 `LEDGER_FIELDS`；
两个集合 MUST **不相交**——一个字段名只属于一个层级。每个条目 MUST 被分入**结构字段**∪
**引用字段**∪**id 清单字段**之一，三类 MUST 两两不相交且其并集恰等于 `LEDGER_FIELDS`；
信封的每个已声明字段 MUST 在台账上**必需**。只遍历一份清单的扫描，盲区恰是**清单的补集** ⇒
新增键 MUST 命中一条**闭世界**断言，且 MUST NOT 只在一个层级上成立。`question` 这类
**复述字段** MUST 被纳入引用扫描，MUST NOT 只做非空检查。

#### Scenario: An entry carries a field the checker does not know

- **WHEN** 某条目新增一个不在 `LEDGER_FIELDS` 里的字段（如 `rationale: "见 §99.9"`），
  **不论其值是否含 `§NNN`**
- **THEN** 校验 MUST 失败并指名该条目与那个字段名——一个不受检查的字段，与没有这条断言是同一件事

#### Scenario: A top-level key the checker does not know

- **WHEN** 台账顶层新增一个不在 `TOP_LEVEL_FIELDS` 里的键（如 `revision: "见 §99.9"`），
  **不论其值是否含 `§NNN`**
- **THEN** 校验 MUST 失败并指名那个键——条目的键集封闭 MUST NOT 被当作整个台账的键集封闭

#### Scenario: A declared envelope field carries no value

- **WHEN** `TOP_LEVEL_FIELDS` 声明了某字段，而台账上没有它
- **THEN** 校验 MUST 失败并指名该字段——声明它就要在数据里有它，否则它是一个没人读的幽灵字段

#### Scenario: A field name that claims both levels

- **WHEN** 某个字段名同时出现在 `TOP_LEVEL_FIELDS` 与 `LEDGER_FIELDS` 里
- **THEN** 校验 MUST 失败并指名该字段——一个名字只属于一个层级；否则它会同时在两个闭世界的缝里

#### Scenario: A declared field falls outside every check category

- **WHEN** 某字段进了 `LEDGER_FIELDS` 却没有被分到任何一类，或同时落入两类
- **THEN** 校验 MUST 失败并指名该字段——否则它在两个检查之间落空，且没有任何信号

#### Scenario: A restatement field is left unscanned

- **WHEN** `question` 这类复述基线内容的字段只做非空检查、不在引用扫描清单里
- **THEN** 它 MUST 被纳入引用扫描（`CITATION_FIELDS`），MUST NOT 只做非空检查——
  「复述处可以随便编」适用于**每一个**复述字段，遗漏一个即与无断言等价
