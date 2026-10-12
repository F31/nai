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

基线与其活文档的就地订正 SHALL 只作用于**与当前事实不符的陈述**；**版本归属引用**——形如「V3.9 §51.3」「V3.9 收缩」——
陈述的是**该条款由哪一版引入**，MUST NOT 随基线号变更：把它一并改写，等于用一次「订正」制造一批**新的**不实陈述。
**当前基线指针**（形如「唯一实现基线为 ……V3.9.md」「不自行拼接旧条款覆盖 V3.9」）SHALL 随基线升版同步。
两类陈述 MUST 被分别识别，MUST NOT 以「版本号出现即改」的规则批量替换。
版本号 SHALL 只出现在 `docs/design.md` 与最新基线 ADR 处作为**当前值**；其余活文档 MUST NOT 把版本号当作当前事实声明。

#### Scenario: A baseline version bump sweeps live documents

- **WHEN** 基线由 Vx 升为 V(x+1)，活文档中同时存在「唯一实现基线为 Vx」与「该条款见 Vx §NNN」两类陈述
- **THEN** 只有**当前基线指针**被改写；**版本归属引用** MUST 保留——除非该条款确实是在新版本引入

#### Scenario: A provenance reference is swept into a false claim

- **WHEN** 一次「订正」把「V3.9 §51.3」（该条款由 V3.9 引入）改写为「V3.10 §51.3」
- **THEN** 该改写 MUST 被拒绝：归属变成了不实陈述，而订正的目的恰恰是消除不实陈述
