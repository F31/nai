# spec-driven-delivery 变更（w88）

## MODIFIED Requirements

### Requirement: Pending decisions carry a disposition and a basis

基线各章「待决策与待确认」的条目 SHALL 被登记进一份机器可读台账（`tests/pending-decisions.yaml`）。
每条 MUST 有**封闭枚举的处置**与**一条非空洞依据**：`answered_by_baseline` MUST 给 `basis`，
其中每个 `§NNN` MUST 真实存在于当前基线；`decidable_in_repo` MUST 给 `recommendation`；
`decided_in_repo` MUST 给 `landing`，其中每个元素 MUST 是**本仓库已跟踪**的路径；
`needs_precondition` MUST 给一组**真实存在**的前置（验收 id 或工作项编号）；
`needs_external` MUST 给 `closure`（取到什么算闭合）；`out_of_scope` MUST 给 `note`。
一条推荐**已被采纳并落地**的条目 MUST 迁到 `decided_in_repo`，MUST NOT 留在「可决定」里。
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

#### Scenario: A settled recommendation stays in the undecided class

- **WHEN** 某条 `decidable_in_repo` 的推荐已经在仓库里落地（代码、数据或既有轮次的产物）
- **THEN** 它 MUST 被登记为 `decided_in_repo` 并给出 `landing`——一个已经决定了的条目继续挂在
  「可在仓库内决定」上，台账读起来会说「还等人决定」，这是**不实**，不是措辞不够好

#### Scenario: A settled item is recorded as answered by the baseline

- **WHEN** 某条待裁定项的答案来自**仓库侧的决定**，而基线正文对它的措辞仍是「待选型验证」
- **THEN** 它 MUST NOT 被登记为 `answered_by_baseline`——该处置断言「基线正文回答了它」，
  而这正是**不实**之所在；`decided_in_repo` 与 `answered_by_baseline` 回答的是两个不同的问题

#### Scenario: A settled item points nowhere

- **WHEN** 某条 `decided_in_repo` 缺 `landing`，或 `landing` 为空
- **THEN** 校验 MUST 失败——「已落地」若不指出落在哪里，它就只是一句自述

### Requirement: The ledger's field set is closed and fully classified

待裁定台账的**字段集** SHALL 在**两个层级上各自封闭**：**信封**（台账顶层）出现的每个键 MUST ∈
已声明集合 `TOP_LEVEL_FIELDS`，**条目**出现的每个键 MUST ∈ 已声明集合 `LEDGER_FIELDS`；
两个集合 MUST **不相交**——一个字段名只属于一个层级。每个条目 MUST 被分入**结构字段**∪
**引用字段**∪**id 清单字段**∪**仓库路径字段**之一，四类 MUST 两两不相交且其并集恰等于
`LEDGER_FIELDS`；信封的每个已声明字段 MUST 在台账上**必需**。只遍历一份清单的扫描，盲区恰是
**清单的补集** ⇒ 新增键 MUST 命中一条**闭世界**断言，且 MUST NOT 只在一个层级上成立。
`question` 这类**复述字段** MUST 被纳入引用扫描，MUST NOT 只做非空检查。

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

- **WHEN** 新增一类检查（如**仓库路径**）而对应的字段没有同步进 `REFERENCE_FIELDS`，
  或进了类别表却不在 `LEDGER_FIELDS` 里
- **THEN** 校验 MUST 失败并指名该字段——「四类并集恰等于 `LEDGER_FIELDS`」与「引用域与类别划分
  双向相等」两条断言 MUST 在**两个方向**上各报一次，MUST NOT 让新类别落在缝里

## ADDED Requirements

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
