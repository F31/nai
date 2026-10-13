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
`decided_in_repo` MUST 给 `landing`，其中每个元素 MUST 是**本仓库已跟踪**的路径，并 MUST 逐项配
`landing_anchor`——该锚点 MUST 真实出现在那个落地点文件的内容里；`needs_precondition` MUST 给一组
**真实存在**的前置（验收 id 或工作项编号）；`needs_external` MUST 给 `closure`；`out_of_scope`
MUST 给 `note`。「待裁定」MUST NOT 被读作「无人负责」。

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

#### Scenario: A settled recommendation stays in the undecided class

- **WHEN** 某条 `decidable_in_repo` 的推荐已经在仓库里落地（代码、数据或既有轮次的产物）
- **THEN** 它 MUST 被登记为 `decided_in_repo` 并给出 `landing` 与 `landing_anchor`——一个已经决定了
  的条目继续挂在「可在仓库内决定」上，台账读起来会说「还等人决定」，这是**不实**，不是措辞不够好

#### Scenario: A settled item is recorded as answered by the baseline

- **WHEN** 某条待裁定项的答案来自**仓库侧的决定**，而基线正文对它的措辞仍是「待选型验证」
- **THEN** 它 MUST NOT 被登记为 `answered_by_baseline`——该处置断言「基线正文回答了它」，
  而这正是**不实**之所在；`decided_in_repo` 与 `answered_by_baseline` 回答的是两个不同的问题

#### Scenario: A settled item points nowhere

- **WHEN** 某条 `decided_in_repo` 缺 `landing`，或 `landing` 为空
- **THEN** 校验 MUST 失败——「已落地」若不指出落在哪里，它就只是一句自述

#### Scenario: A settled item points somewhere the content is gone

- **WHEN** 某条 `decided_in_repo` 的 `landing` 每个元素都是仓库成员，但 `landing_anchor` 的内容
  已不在那个文件里（落地内容被撤销或落地点被换成另一个已跟踪文件）
- **THEN** 校验 MUST 失败——指着**任意**一个已跟踪文件都算「已落地」，等于把「已落地」降级成了
  「路径存在」

### Requirement: Every field that can carry a section reference is checked

待裁定台账的引用完整性 SHALL 覆盖**每一个可能承载 `§NNN` 的字段**——`basis`、`introduced_in`、
`also_in`、`recommendation`、`closure`、`note`、`question`；扫描域 MUST 由一份**单一清单**派生，
MUST NOT 在循环里手写。触发条件 MUST 只由**字段是否存在**决定，MUST NOT 由**条目当前的处置**
决定——一个字段由谁解析它的引用，是**字段的属性**；按处置触发会让同一个字段换个处置就**没人管**。
校验器自身的常量表 MUST 互相一致：处置枚举与必填字段 MUST **一一对应**。

#### Scenario: A reference in a field outside the scanned set

- **WHEN** 任一承载 `§NNN` 的字段（含 `recommendation`／`closure`／`note`／`question`）引用一个
  当前基线里不存在的章节号
- **THEN** 校验 MUST 失败，并**指名该条目、该字段与那个章节号**——漏掉字段里的一条引用，
  与没有这条断言是同一件事

#### Scenario: A restatement field is left unscanned

- **WHEN** `question` 这类**复述字段**只做非空检查、不在引用扫描清单里
- **THEN** 它 MUST 被纳入引用扫描，MUST NOT 只做非空检查——「复述处可以随便编」适用于
  **每一个**复述字段，遗漏一个即与无断言等价

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
**引用字段**∪**id 清单字段**∪**仓库路径字段**∪**内容锚点字段**之一，五类 MUST 两两不相交且其并集
恰等于 `LEDGER_FIELDS`；信封的每个已声明字段 MUST 在台账上**必需**。只遍历一份清单的扫描，盲区恰是
**清单的补集** ⇒ 新增键 MUST 命中一条**闭世界**断言，且 MUST NOT 只在一个层级上成立。
`question` 这类**复述字段** MUST 被纳入引用扫描，MUST NOT 只做非空检查。**内容锚点字段**
（承载"文件里应有这段内容"的证据）MUST **不**进引用扫描域——它不需要被解析成章节、id 或路径，
它需要被**读到并比对**。

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

#### Scenario: A new check category enters the partition

- **WHEN** 新增一类检查（如**仓库路径**或**内容锚点**）而对应的字段没有同步进 `REFERENCE_FIELDS`，
  或进了类别表却不在 `LEDGER_FIELDS` 里
- **THEN** 校验 MUST 失败并指名该字段——「各类并集恰等于 `LEDGER_FIELDS`」与「引用域与类别划分
  双向相等」两条断言 MUST 在**两个方向**上各报一次，MUST NOT 让新类别落在缝里

#### Scenario: A content-evidence field joins the reference scan

- **WHEN** 「内容锚点」这类字段被加进了 `REFERENCE_FIELDS`（可解析引用的扫描域）
- **THEN** `shapeErrors()` 的「被类别认领却不在声明集里」方向 MUST 覆盖它并**具名**点名——
  锚点不是可解析引用，把它塞进引用扫描域会让「谁在管它」失去单一答案

### Requirement: Every register-id reference is resolved wherever its field appears

待裁定台账里**承载登记 id 引用**的字段（`precondition`），其每个元素 SHALL 被解析为**基线作业项**
或**注册表验收 id**。校验 MUST 按**字段是否存在**触发，MUST NOT 按**处置**触发——把
`precondition` 挂到非 `needs_precondition` 的条目上，此前**完全静默**。引用扫描域 MUST 是**单一
声明**（`REFERENCE_FIELDS`），且 MUST 与类别划分**双向相等**——声明了却无类别认领，其引用
**没人解析**；被类别认领却不在声明集里，扫描会**跳过它**。「解析得开」只答「这个 id 存在」，
**不**答「它现在还悬空着」——后者是**另一条**断言，两条 MUST NOT 互相冒充。

#### Scenario: An id reference on an entry of another disposition

- **WHEN** 某条**非** `needs_precondition` 的条目的 `precondition` 写成基线里不存在的作业项编号，
  或注册表里没有的验收 id
- **THEN** 校验 MUST 失败并**指名该条目、该字段与那个 id**——静默接受一条「等不存在的东西」的
  前置，等于把这条登记挂在了空气上

#### Scenario: A legal reference on an entry of another disposition

- **WHEN** `precondition` 出现在**非**其专属处置的条目上，而它的每个 id 都能解析
- **THEN** 校验 MUST NOT 失败——本轮拒绝的是**不可解析**的引用，不是**放错位置**的引用；
  把「位置」也一并拒绝会把合法数据判红（假阳性方向）

#### Scenario: A reference field that no category owns

- **WHEN** 某字段进了引用扫描域却没有被任何类别认领，或被类别认领却不在扫描域里
- **THEN** 校验 MUST 失败并指名该字段——否则它在两个检查之间落空，且没有任何信号

### Requirement: Every tracked test file is reached by a registration site

仓库里每个**在册**（已跟踪）的 Node 测试文件 SHALL 被某个**登记站点**覆盖，或被登记在
**未覆盖台账**里。覆盖集 MUST 由站点的 marker **原文解析**得出，MUST NOT 另行声明。
每个站点声明的 marker MUST 在其 host 里**逐字存在**，且 MUST 至少覆盖一个在册文件。

#### Scenario: A new test file nobody registers

- **WHEN** 一个测试文件被加入仓库（已在 `.git/index` 里）却没有任何登记站点覆盖它，
  且它不在未覆盖台账里
- **THEN** 校验 MUST 失败并**指名该文件**——一个谁都不跑的测试文件此前**完全静默**
  （门控 `EXIT=0`、目标数与用例数不变、其用例名在日志里零出现）

#### Scenario: A site that is declared but not written

- **WHEN** 某个登记站点在表里声明了 marker，而它的 host 文件里**没有**这段 marker 原文
- **THEN** 校验 MUST 失败并**指名该站点与该 marker**——「名字在表里」不等于「线上真的有它」

#### Scenario: A site that covers nothing

- **WHEN** 某个登记站点的 marker 解析后在在册集合上**匹配不到任何文件**
- **THEN** 校验 MUST 失败并指名该站点——一条覆盖不到东西的站点是空声明，
  它与「站点数大于零」是两回事

#### Scenario: A stale exemption in the unreachable ledger

- **WHEN** 未覆盖台账里的某个文件**已经**被某个站点覆盖，或它不是被识别的测试文件、不在册，
  或缺少非空理由
- **THEN** 校验 MUST 失败并指名该条目——台账**不是豁免**，它必须自清理

#### Scenario: A test extension nobody scans

- **WHEN** 仓库里出现一个**看起来像测试**的在册文件，其扩展名不在被识别的清单里
- **THEN** 校验 MUST 失败并指名该文件与扩展名——文件层的闭世界对它**看不见**，
  因为「不是被识别的测试文件」意味着连「未被覆盖」都不会被报出

#### Scenario: An empty coverage set

- **WHEN** 覆盖集为空（站点表被清空，或全部站点解析失败）
- **THEN** 校验 MUST 失败——台账为空时「覆盖与台账不相交」退化成恒真，
  这条断言是「全不覆盖也算绿」的唯一守卫

### Requirement: Historical records are protected by the commit boundary

对**历史记录**（`docs/reports/`、`docs/adr/`、`openspec/changes/archive/`）的处置是
「保留原文 + 追加勘误段」，其**保护对象是「已发布的原文」**——判据 = 该原文**是否已进入提交**
（即是否曾在**工作区之外**可见）。**同一轮内在工作区内新建、尚未提交的产物**（例如本轮创建并归档的
change 目录）**MUST 就地订正**，不产生勘误段（它从未发布，没有「已发布的原文」需要保护）；
**已发布**的原文 **MUST NOT 被就地改写**，只能**追加勘误段**。该判据 MUST NOT 依赖「轮次」——
「本轮」在多人协作下无定义，只有**提交边界**可判。

#### Scenario: A same-round archive is edited in place, a published one is not

- **WHEN** 一份变更目录在本轮被**创建并归档**且**尚未进入任何提交**（同一次未提交工作区内），
  随后发现其中的转述有误
- **THEN** 该产物 MUST 被**就地订正**——它从未发布，没有「已发布的原文」需要保护；
  MUST NOT 因「它已在 `openspec/changes/archive/` 路径下」就把它当作冻结的已发布原文

#### Scenario: A published archive is rewritten instead of corrected

- **WHEN** 一份**已进入提交**的 `docs/reports/`、`docs/adr/` 或 `openspec/changes/archive/` 产物
  的**原文**被直接改写
- **THEN** 该改写 MUST 被拒绝，正确处置是**追加勘误段**——保护的对象是「已发布的原文」，
  与它此刻是否仍在工作区无关

### Requirement: Every repository-path reference is resolved wherever its field appears

待裁定台账里**承载仓库路径引用**的字段（`landing`）SHALL 被逐个解析为**本仓库已跟踪的路径**。
判据的输入 MUST 由 `.git/index` 派生，MUST NOT 遍历工作区——磁盘上存在的未跟踪文件**不是**
仓库成员。校验 MUST 按**字段是否存在**触发，MUST NOT 按**处置**触发。一个字段是**清单型**
还是**文本型** MUST 由**类别表**（`REPO_PATH_FIELDS`）决定，MUST NOT 靠写死字段名判定。

#### Scenario: A landing path that is not a repository member

- **WHEN** 某条 `landing` 的元素是一个仓库里不存在的路径，或一个**磁盘上存在但未被跟踪**的路径
- **THEN** 校验 MUST 失败并**指名该条目、该字段与该值**——「已声明 ≠ 存在」对落地点同样成立

#### Scenario: A path-shaped required field judged as free text

- **WHEN** 必填的路径字段被给成一个**非空字符串**（而不是清单）——即使那个字符串**恰好是**
  一个真实存在的跟踪路径
- **THEN** 校验 MUST 失败并指名该字段——按文本判会把「一个落地点都没给」放行；清单型与文本型
  的分界 MUST 由类别表决定，MUST NOT 由写死的字段名决定

#### Scenario: A landing reference on an entry of another disposition

- **WHEN** `landing` 出现在**非** `decided_in_repo` 的条目上，而它的某个元素不可解析
- **THEN** 校验 MUST 失败并指名该条目、该字段与该值——一个字段由谁解析它的引用，是**字段的属性**，
  与这条数据当前是什么处置无关

#### Scenario: A legal landing reference on an entry of another disposition

- **WHEN** `landing` 出现在**非**其专属处置的条目上，而它的每个元素都是仓库成员
- **THEN** 校验 MUST NOT 失败——本轮拒绝的是**不可解析**的引用，不是**放错位置**的引用

### Requirement: Every landing site carries content evidence that is actually there

`decided_in_repo` 的每个落地点 SHALL 配一个**内容锚点**：`landing_anchor` MUST 与 `landing`
**等长且逐项对应**，第 *i* 个锚点 MUST 真实出现在第 *i* 个落地点文件的**内容**里；两者 MUST
**同生同灭**——有落地点就 MUST 有锚点，反之亦然。判据验的是「**所指文件里有这段内容**」，
**不**验「这段内容只应出现在那里」，也**不**验它语义上兑现了那个决定（那部分仍由人读）。
读不到该文件 MUST **具名失败**，MUST NOT 静默降级。

#### Scenario: A landing site whose anchor is not in the file

- **WHEN** 某条 `landing_anchor` 的第 *i* 项不出现在第 *i* 个落地点文件的内容里
  （落地点被换成另一个已跟踪文件，或落地内容被撤销）
- **THEN** 校验 MUST 失败并指名该条目、该字段与该值——「已落地」若只看路径是否存在，
  它就是一条被判据放过的空声明

#### Scenario: An anchor list that does not match its landing list

- **WHEN** `landing_anchor` 与 `landing` 的长度不等，或任一项不是非空字符串
- **THEN** 校验 MUST 失败并指名该条目与两侧长度——按位置对应的两份清单，长度不等即对应关系不存在

#### Scenario: A landing without an anchor

- **WHEN** 某条目有 `landing` 却没有 `landing_anchor`（或反之）
- **THEN** 校验 MUST 失败并指名该条目与缺失的那个字段——锚点若可选，「不给锚点」会成为
  绕过本条判据的那条缝，洞会原样重开

#### Scenario: An anchor on an entry of another disposition

- **WHEN** `landing_anchor` 出现在**非** `decided_in_repo` 的条目上，而它的锚点不在对应文件里
- **THEN** 校验 MUST 失败并指名该条目、该字段与该值——一个字段由谁检查它的内容证据，是
  **字段的属性**，与这条数据当前是什么处置无关

#### Scenario: A legal anchor on an entry of another disposition

- **WHEN** `landing_anchor` 出现在**非**其专属处置的条目上，而它的锚点确实在对应文件里
- **THEN** 校验 MUST NOT 失败——本条拒绝的是「**锚点不在**」，不是「**放错位置**」

#### Scenario: An unreadable landing file

- **WHEN** 某个落地点文件读不到（不存在、不可读、读取抛错）
- **THEN** 校验 MUST 失败并**指名该路径**——「没读到」与「读到了但不含锚点」是两种不同的错，
  MUST NOT 都被当成通过

### Requirement: Every precondition is still unmet

`needs_precondition` 断言的是「这条现在还不能裁定，因为它在等前置」。判据 MUST 断言
`precondition` 的每个元素**仍未达成**；已达成的前置 MUST 被**具名拒绝**。判定 MUST 由**注入的
事实**算出，MUST NOT 由注入侧直接给答案：注入验收 id → `status` 的表与工作项 → 关联验收的表，
「达成了没有」由判据自己按取值域判定（工作项没有状态列，其达成状态由基线**明写**的关联验收派生，
措辞 MUST 只说到派生那一层）。`status` 取值域 MUST 被 `MET_STATUSES` 与 `UNMET_STATUSES`
**恰好划分**，运行时遇域外取值 MUST 具名失败；工作项缺关联验收、或关联验收不在表内 ⇒ 同样
MUST 具名失败，MUST NOT 静默跳过。触发条件 MUST 只由**字段是否存在**决定，MUST NOT 由处置决定。

#### Scenario: A precondition that is already met is reported

- **WHEN** 某条 `precondition` 指着一个 `status` 已属 `MET_STATUSES` 的验收项，
  或指着一个其关联验收**全部**已达成的工作项
- **THEN** 校验 MUST 失败并指名该条目、该字段与那个 id——「还在等」若只看 id 是否解析得开，
  它就是一条被判据放过的空声明：等一个已达成的前置，与等一个真正悬空的前置会得到同一个答案

#### Scenario: An undecidable precondition does not pass silently

- **WHEN** 某工作项在基线里没有关联验收 id，或其关联验收不在注入的 `status` 表里，
  或取到的 `status` 落在声明的取值域之外
- **THEN** 校验 MUST 失败并指名该条目与那个 id——「没看到」与「出错了」在类型上分开，
  MUST NOT 被当成「仍未达成」而放行

#### Scenario: A status domain that no longer covers reality

- **WHEN** `MET_STATUSES` 与 `UNMET_STATUSES` 之并不再恰等于声明的 `status` 取值域（新增了
  一个取值却忘了归类），或两者相交
- **THEN** 校验 MUST 失败并指名该取值——否则判据会**静默地不认识**那个新取值，而没有任何信号

#### Scenario: A met precondition on an entry of another disposition

- **WHEN** `precondition` 出现在**非**其专属处置的条目上，而它指着一个已达成的前置
- **THEN** 校验 MUST 失败——触发条件是字段是否存在，与该条目当前归哪一类无关
