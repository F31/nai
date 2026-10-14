## ADDED Requirements

### Requirement: An assembly that may not be called is named and leaves the chain untouched

装配根 SHALL 在把一次装配交给只追加哈希链之前先跑 `agent-harness` 的 §41.1–§41.3 判定，且该判定
MUST NOT 被运行期短路：判定为**不可调用**（不可裁剪段缺失或被裁、段序错乱、裁剪落在前段、裁剪缺被裁项
或缺原因、某段缺自有配额、缺预算维度、不可裁段缺固定下限、任一分量超限、片段缺来源标识或作用域、
不可信来源被标为指令）时 MUST **具名拒绝**，且 MUST NOT 追加任何条目。拒绝 MUST 逐条列出两项判定
各自给出的拒绝名，**两类都报**，MUST NOT 只报先命中的那一类。入口的返回 MUST 可区分「不可调用」
「不可重建」与「两者皆拒」三种拒绝，MUST NOT 把其中任意两种折叠成同一种结局。

#### Scenario: An uncroppable segment is missing and nothing is appended

- **WHEN** 一次装配缺少 §41.1 的三个不可裁剪段之一
- **THEN** 入口具名拒绝（含 `assembly.uncroppable_segment_missing`），且决策日志**未新增**任何条目

#### Scenario: Both judgements refuse, and both are named

- **WHEN** 一次装配既不可调用（如裁剪落在前段）又不可重建（如某片段正文不在日志里）
- **THEN** 拒绝里同时出现 `assembly.*` 与 `rebuild.*` 两类拒绝名；
  只报其中一类、或报了一类就停下的实现不满足本需求

#### Scenario: The three refusals are distinguishable

- **WHEN** 分别构造「仅不可调用而可重建」「仅不可重建而可调用」「两者皆拒」的三次装配
- **THEN** 三次的返回 MUST 分属三种不同的具名结局；把它们折成同一个结局的实现不满足本需求

### Requirement: The logged payload carries per-segment usage and the crops with their reasons

追加的 `assembly` 条目其载荷 MUST 含 §41.5 列明的**四项**：进入的片段标识、**各段预算占用**、
**裁剪项与原因**、模型与提示词版本。各段预算占用 MUST 按段给出该段的 Token、字符与条数三个维度的用量，
并在该段自带配额时一并给出该配额；裁剪项与原因 MUST 按段**同时**给出被裁项标识与原因原文。
载荷的取值 MUST 全部来自两项判定**审过**的输入，runtime MUST NOT 自行推算、补零或改写；
判定未审过的字段（如片段的时效、数据/指令标注，以及判定产出的诊断清单）MUST NOT 落账。

#### Scenario: A cropped assembly lands with its usage and crops

- **WHEN** 一次装配的某可裁段被裁剪，且其被判定的输入同时记了用量、被裁项与原因
- **THEN** 落账载荷该段的裁剪记录含被裁项标识与原因原文，且各段的 Token、字符与条数用量齐备

#### Scenario: A crop record with only one half does not land

- **WHEN** 实现只落裁剪的原因而不落被裁项（或反之）
- **THEN** 该实现不满足本需求——§41.5 要求的是「裁剪项与原因」两半

#### Scenario: Usage is the judged input, not a recomputation

- **WHEN** 某段在判定输入里的用量与该段片段正文的实际长度不符
- **THEN** 落账的用量 MUST 是判定输入里的那一份；按正文重新计算或补零的实现不满足本需求

### Requirement: The two inputs must describe the same fragments

入口 MUST 校验判定输入的各段片段标识序列与其记录声明的进入片段标识序列相同；不同 MUST **具名拒绝**，
且 MUST NOT 追加任何条目。依据是 **§41.5 的四项描述的是同一次装配**：若把两组不一致的标识都落账，
同一条链记录会自相矛盾——「各段预算占用」说这一段进了某个片段，而「进入的片段标识」说进的是另一个。

#### Scenario: Disagreeing inputs are refused, not logged

- **WHEN** 判定输入里出现的片段标识序列与记录声明的进入片段标识序列不一致
- **THEN** 入口具名拒绝，且决策日志**未新增**任何条目；把两组标识一并落账的实现不满足本需求

#### Scenario: Equal sequences are required, not equal sets

- **WHEN** 两份输入含同一组标识但顺序不同
- **THEN** 具名拒绝——四项描述同一次装配，而进入顺序是其中一项已声明的要求，
  集合相等不足以放行

### Requirement: Logged assemblies are read back row by row, with their usage and crops

装配根 SHALL 提供一处**只读**入口，把链上的 `assembly` 条目**逐行**读回，每行携带该行落账的四项；
读回 MUST NOT 追加、修改或重排链。**逐行**是必需的：把多行合并成一个池会让「各段预算占用」失去
可归属的行。一行不可解析 MUST 是**具名失败**，MUST NOT 被静默跳过——静默跳过会让「读回了 N 行」
与「链上有 N 行」无法区分。**早于本次接线落下的行**（其载荷未含预算与裁剪两项）MUST 仍可被读回，
其缺失的两项 MUST 读作**空**，MUST NOT 被读作解析失败。

#### Scenario: Every logged row is read back with its own usage and crops

- **WHEN** 两次装配各自落链，其中一次含裁剪
- **THEN** 只读入口返回**两行**，每行各带自己的片段序列、预算占用与裁剪记录，互不合并

#### Scenario: An unparsable row is a named failure, not a skip

- **WHEN** 链上某条 `assembly` 条目的载荷不是本入口认可的形状
- **THEN** 入口返回具名错误；静默跳过该行而返回其余行的实现不满足本需求

#### Scenario: A row logged before the payload carried usage and crops

- **WHEN** 链上存在一条不含各段预算占用与裁剪两项的既有 `assembly` 条目
- **THEN** 该行仍被读回，其预算占用与裁剪读作**空**；把它读作解析失败、或跳过该行的实现不满足本需求

#### Scenario: Reading the log appends nothing

- **WHEN** 调用只读入口读回装配行
- **THEN** 决策日志**未新增**任何条目，链的既有序与哈希不变

## MODIFIED Requirements

### Requirement: A rebuildable assembly reaches the hash chain as its own fragments

装配根 SHALL 提供一处把一次上下文装配交给只追加哈希链的入口，且该入口 MUST 先经 `agent-harness` 的
**两项**装配判定：只有**可调用**（§41.1–§41.3 的段序、不可裁段、裁剪位置、分段预算与片段标注全部通过）
**且可重建**（§41.5 的日志重建）的装配才追加一条 `assembly` 类条目。追加的载荷 MUST **恰为** §41.5
要求落账的**四项**——按记录**进入顺序**排列的片段（每项带其 `id`、来源标识 `source_ref` 与**字面正文**）、
各段预算占用、裁剪项与原因、模型与提示词版本；正文 MUST NOT 退化成指向当时内存的指针。
追加 SHALL 复用装配根既有的单 owner 决策写入路径与逐次调用预算，MUST NOT 另开第二条写入路径。
追加之后链 SHALL 校验通过。

#### Scenario: A rebuildable assembly is appended with its fragment bodies

- **WHEN** 一次装配经两项判定为可调用且可重建
- **THEN** 决策日志追加一条 `assembly` 条目，其载荷按进入顺序含每个被引用片段的标识、来源标识与**正文**，
  且哈希链校验通过

#### Scenario: A fragment body that only lives in memory is refused, not logged as a pointer

- **WHEN** 某被引用片段的正文不在交给判定的材料里（只存在于当时的内存）
- **THEN** 该装配被判为不可重建，入口具名拒绝且不追加；把正文落成指针（或 `null`）后追加的实现
  不满足本需求

#### Scenario: One of the two judgements refuses, and nothing is appended

- **WHEN** 一次装配可重建但不可调用（或缺段、或超预算）
- **THEN** 入口具名拒绝且不追加；只跑可重建那一项判定就放行的实现不满足本需求
