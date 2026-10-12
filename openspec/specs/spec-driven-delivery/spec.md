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
