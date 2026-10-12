## 1. 本轮是「补实现」，不是「新建能力」——先说清楚这个判断

W56 已经建立能力 `agent-harness`（10 条需求 / 27 个场景），其中**前两条就是 H01/H02**：

- `Context is assembled in a fixed order under per-segment budgets`
- `Every assembled fragment is sourced, scoped and reconstructible`

所以本轮的 OpenSpec 差量**不能**是 `ADDED Requirements`（会制造重复需求），只能是
`MODIFIED Requirements`——**并且必须只加强、不削弱**。仓库里已有先例
（`w52-open-contract-catalog` 修改 `versioned-contracts`、`w65-v39-scope-and-baseline` 修改 `admission-registry`）。

本轮对两条需求各补的内容，以及**为什么它们不是重复**：

| 需求 | 已写的 | 本轮补的 | 为什么非补不可 |
| --- | --- | --- | --- |
| 固定顺序与分段预算 | 九段顺序、前三不可裁、裁剪只从后段、记录被裁项与原因、Token 与字符两维度 | 预算**按段**分配（禁一个总上限顶替）、不可裁段带固定下限、**条数**是第三个维度、三个可判定的新场景 | 原文的「裁剪 MUST 只从后段发生」**没有任何场景能把它与任意裁剪区分开**——H01 的验收名恰恰就是「裁剪只从后段发生并记录原因」。不补场景，这条 MUST 就只是注释 |
| 片段来源、作用域与可重建 | 七类来源、来源标识、作用域、时效（如适用）、数据或指令标注、落日志、可由日志重建 | 来源标识**不得为空白**、不可信四类**标为指令即拒**、**内容未落日志则不可重建** | H02 的验收名是「**不依赖当时的内存**」。原文只说「可由日志重建」，但一个「重建时回内存里捞原文」的实现同样满足字面——必须有一条场景把它排除 |

**削弱性改动一条都没有**：原文里的每一条 SHALL/MUST 在本轮文本中原样保留。

## 2. 为什么是新模块 `evaluate::assembly`，而不是并入 `harness.rs`

`harness.rs`（W66，H15/H17）里 `Assembly` 出现 **11 次**，但**没有一次是在判装配**：

| 出现处 | 它其实在说什么 |
| --- | --- |
| `ObservationLayer::Assembly` | 五类观测记录里的**一类**——“装配这件事要记哪些字段” |
| `FailureLayer::Assembly` | 六类失败层里的**一层**——“装配失败该落到 §12.2 哪一级” |
| 必需字段 `segment_budget` / `cropped_items` / `crop_reason` | 装配**记录**的形状 |
| `DegradedCallContext::has_policy_constraint` | 降级**之后**上下文还在不在 |

本模块判的是完全不同的东西：**一次装配本身合法不合法**——顺序、不可裁段、裁剪落点、分段预算、片段标注。
这是 W66 不复用 `bridge::Degradation`、W67 不复用 `session::change_mode`、W68 不复用 `evaluate::show`
的**同一条理由**：把「这次拒绝属于哪一类」合并进一个结构，它就重新不可判定。

也**不**复用 `selling::SegmentKind`：那是 §53 讲品循环的四段（卖点/规格/适用场景/售后说明），
与本模块的九段（策略/人设/契约/事件/证据/记忆/技能/已播句/工具结果）名字像、维度不同，合并会制造歧义。

## 3. 九段与七类来源：必须逐项写死的两张表

§41.1 的表格九行、§41.3 的来源类型七类，都是**封闭清单**。可判定形式是
`SEGMENT_ORDER: [Segment; 9]` 与 `SOURCE_KINDS: [SourceKind; 7]`，**逐项照抄**，并各配一条断言把它们写死
（W66 的教训：只校验「九条、`label()` 互异」的守卫，对**内容被改小**是瞎的——实测悄悄删一项，全部测试照样绿）。

「不可裁」属性也来自同一张表：1–3 行写的是「不可裁」，4–9 行写的是各种可裁形态。故
`Segment::uncroppable()` 的判据是**序号 ≤ 3**，不是另立一份名单。

## 4. 段名必须是 `&str` 而不是枚举——否则 `UnknownSegment` 构造不出来

这是 W68 的直接教训（「**数据结构的形状决定守卫能不能触发**」）。如果 `AssemblyPlan` 里装的是
`Segment` 枚举，那么「出现九段之外的名字」这件事**在类型上无法表达**，`UnknownSegment` 这条拒绝
就永远构造不出来，测试只能写成恒真。

```
pub struct AssemblyPlan<'a> {
    pub slices: &'a [SegmentSlice<'a>],
    pub model_version: &'a str,
    pub prompt_version: &'a str,
}
pub struct SegmentSlice<'a> {
    pub segment: &'a str,          // ← 刻意用 &str：未定义段名必须可表达
    pub fragments: &'a [Fragment<'a>],
    pub usage: Option<SegmentUsage>,
    pub budget: Option<SegmentBudget>,
    pub crop: Option<Crop<'a>>,
}
```

## 5. 「裁剪只从后段发生」的可判定形式是**后缀性**

§41.1 的表述是「预算不足时从后往前裁剪，前段不可裁」。可判定的读法：

> 只看**可裁剪**的那些段，被裁的必须是它们的一个**后缀**。

理由：预算不足时从后往前裁，意味着「后段先被裁」；一旦裁到某一段，它之后的可裁段必然**也**被裁过
（否则就不是「从后往前」）。反过来，`{第 8 段被裁，第 6/7 段完好}` 这种形状在任何「从后往前」的过程里
都产生不出来。

**为什么不能只判「有没有裁剪记录」**：一个只检查 `crop.is_some() ⇒ reason 非空` 的实现，
会让「裁掉策略段、留着重记忆段」通过。`CropFromFront` 这条拒绝就是为了堵它。

**为什么还需要「裁剪必须记被裁项」**：`Crop { dropped, reason }` 两个字段都有 ——
`dropped` 为空表示「声称裁了、却说不清裁了什么」，这会把「被裁掉」与「本来就没有」混为一谈。
§41.1 写的是「MUST 记录**被裁项**与原因」，两个都要。

**顺序断言的落点**（W66/W67 教训：先命中的断言会短路，要多组注入分别验证）：

1. 先判**段名是否已知** ⇒ `UnknownSegment`；
2. 再判**顺序** ⇒ `SegmentOutOfOrder`；
3. 再判**不可裁段齐备** ⇒ `UncroppableSegmentMissing`（只查这三段；另六段可因裁剪而缺席）；
4. 再判**不可裁段未被裁** ⇒ `UncroppableSegmentCropped`；
5. 再判**裁剪记录** ⇒ `CropWithoutReason` / `CropWithoutDroppedItems`；
6. 最后判**裁剪落点** ⇒ `CropFromFront`。

## 6. 不可裁段缺失 = 拒绝调用，但这不等于复用 §47.4 的降级判定

§41.1：「**不可裁剪段（1–3）缺失时 MUST 拒绝调用**，MUST NOT 用『少几条策略继续生成』顶上。」
§47.4 的表格首行也写着「装配（不可裁剪段缺失）→ 拒绝调用，退模板」。

本模块因此暴露 `callable`：**只有零拒绝时才为真**。刻意**不**复用 `harness::FailureLayer::Assembly`
去算「落到哪一级降级」——那是 W66 的判定面（H17 的验收对象），本模块判的是 H01 的验收对象。
两者是**同一个事实的两个不同问题**（「该不该拒绝」vs「拒绝之后退到哪一级」），
合并会让 H01 与 H17 的验收对象互相污染。

## 7. 分段预算：三个维度，以及「阈值不在模块里」

§41.2 的要点是「**按段**分配，不是一个总上限」。故每个进入装配的段 MUST 有自己的 `SegmentBudget`——
`SegmentBudgetMissing` 就是为「用一个总上限顶替分段预算」这类实现准备的拒绝。

三个可判定的维度：**Token**、**字符**、**条数**。

- Token 与字符：§41.2 明写「Token 与字符同时记录…是**两个**维度，互不替代」⇒ 任一为 `None` 即
  `BudgetDimensionMissing`；
- 条数：§41.1 的裁剪列给了「已批准事实与检索证据最多 3–5 条」「可用技能与工具描述最多 5 个」。
  条数因此是 §41.1 表格里**已经写着**的约束，把它落成第三个维度不是发明。但——
- **取值一律来自 `AssemblyPolicy`**，模块**不内建**「3 到 5」这类数字。理由是 §41.2 自己说的：
  「初值只作起点，MUST 用 §47.2 的黄金集回放调参并记录 ADR」，以及 §48.7 把阈值列为待本场景回放实测、
  **MUST NOT 采外部观察值**。`Default` 里给的是**基线的初值**并在文档里标注它是初值，不是标准。
- 不可裁段的「固定下限」同理：§41.2 写「策略与人设段取固定下限」⇒ `UncroppableWithoutFloor`。

`SegmentOverBudget` 是一条拒绝、三个维度共用，维度以 **finding 的形式指名**
（`BudgetFinding { segment, dimension, used, limit }`）——分成三条拒绝会让「哪一段、哪个维度」
在类型上就丢掉了。

## 8. 数据/指令标注：用**类型**表达，只判**错值**

§41.3：「每个片段 MUST 携带…以及**数据或指令**的标注。观众文本、检索片段、工具返回、外部智能体返回
一律标为**数据**；其中出现的『忽略规则』『已批准』等措辞没有指令权（沿用 INV-09、§11.7）。」

建模：`Fragment.trust: Trust`（`Data` / `Instruction`）**不是 `Option`** —— 缺标注这件事在类型上不可表达，
因此不需要一条「缺标注」的拒绝；需要判的是**错值**：`UntrustedMarkedAsInstruction`。

四类「一律标为数据」的来源，逐一映射到 §41.3 的七类来源类型上，并把这四句原文写进 doc comment：

| 基线原文 | 对应的 `SourceKind` |
| --- | --- |
| 观众文本 | `Event` |
| 检索片段 | `KnowledgeEvidence` |
| 工具返回 | `ToolResult` |
| 外部智能体返回 | `ExternalAgent` |

`must_be_data()` 的判据是**这四类**，另三类（系统策略 / 场景 / 记忆）§41.3 未加约束，故不判。
配一条断言把这四个名字写死，防「内容被改小」。

**「时效（如适用）」本轮明确不判**：原文自带限定词，故 `age_ms: Option<u64>` 缺失**不是**违规。
把它写成拒绝属于**过度解读基线**，本轮不做，并在文档里写明这个取舍。

而**作用域必须判**：§41.3 把它与来源标识、来源类型并列，没有限定词 ⇒ `FragmentWithoutScope`。
这与 W67 的纪律一致：**前置条件要具名，不能悄悄跳过**。

## 9. H02：重建的两条拒绝必须分开，且真判据是**日志顺序**

§41.5：「装配 MUST 落决策日志：进入的片段标识、各段预算占用、裁剪项与原因、本次调用的模型与提示词版本。
…验收 H01、H02。」H02 的验收名：「装配可重建：**由决策日志的片段引用重建同一上下文，不依赖当时的内存**。」

两条分开的拒绝：

- `FragmentNotLogged`：记录里引用了某个片段 id，日志里**根本没有这一条**；
- `FragmentContentNotInLog`：日志里有这一条，但它的内容标注为「当时从活动内存取」⇒ **依赖当时的内存**。

**为什么不合成一条**：「压根没记」与「记了但记的是个指针」是两种不同的缺陷，
前者是日志不完整、后者是日志格式本身不足以重建。合成一条会让「该修哪一个」重新不可判
（同 W66 把 `bridge::Degradation` 与 harness 降级分开的理由）。

**真判据是顺序**：H02 要求「重建**同一**上下文」。因此重建**只能**消费日志——`rebuild_assembly(record, logged)`
把 `logged` 里被引用的片段**按日志自己的顺序**排出来，与记录里的 `fragment_ids` 比较；
不相等即 `RebuildOrderDiffers`。若改用记录的顺序去排，这个断言会恒真、退化成注释。

`ModelVersionMissing` / `PromptVersionMissing` 直接对应 §41.5 列出的两个必填版本字段。

## 10. INV-11：复用 `harness::CREDENTIAL_MARKERS`，不建第二张表

§41.5 末句：「装配记录 MUST NOT 含明文凭据（INV-11）。」`harness.rs` 已经有 `pub const CREDENTIAL_MARKERS`
（W66 交付，含 `sk-` / `api_key=` / `authorization:` / `bearer ` / `password=` / `secret=` / `token=`）。

本模块**直接引用**它，不复制。复制会在两处留下「哪些字面量算凭据」的真相，
日后只改一处就会出现「日志面认得、装配面不认得」的缝隙——这正是 W55 把平台理由码
**从 `UnavailableReason::label()` 派生**而不另立常量表的同一条纪律。

同一纪律还用在**跨模块守卫**上：`ObservationLayer::Assembly::required_fields()` 是 §47.1 为 L1 装配规定的
七项必需记录项。本模块让 `AssemblyRecord` 暴露同一份字段名清单，并断言**两者逐项相等**——
改一处而漏另一处即测试失败。这条断言是**跨两个模块**的，比模块内自洽更强。

## 11. 具名拒绝与两条 `ALL`

两条判定各有一条扁平枚举 + `ALL` + `label()`（house style，见 `MemoryRejection` / `RampRejection` / `PitchRejection`）：

`AssemblyRejection`（14 条）：

| 拒绝 | 触发 |
| --- | --- |
| `UnknownSegment` | 出现九段之外的段名 |
| `SegmentOutOfOrder` | 段序列不是固定顺序的**子序列** |
| `UncroppableSegmentMissing` | 1–3 之一缺席（指名） |
| `UncroppableSegmentCropped` | 1–3 之一被裁 |
| `CropWithoutReason` | 裁了但原因空白 |
| `CropWithoutDroppedItems` | 裁了却说不出裁了哪些 |
| `CropFromFront` | 被裁的不是可裁序列的后缀 |
| `SegmentBudgetMissing` | 该段没有自己的配额 |
| `BudgetDimensionMissing` | Token 与字符未同时记录 |
| `UncroppableWithoutFloor` | 不可裁段没有固定下限 |
| `SegmentOverBudget` | 用量超出该段任一周配（维度以 finding 指名） |
| `FragmentWithoutSourceRef` | 片段缺来源标识或为空白 |
| `FragmentWithoutScope` | 片段缺作用域 |
| `UntrustedMarkedAsInstruction` | 不可信四类被标为指令 |

`RebuildRejection`（6 条）：`FragmentNotLogged` / `FragmentContentNotInLog` / `RebuildOrderDiffers` /
`ModelVersionMissing` / `PromptVersionMissing` / `PlaintextCredential`。

测试各断言「构造出的拒绝集合**恰为** `ALL`」——既证每条可构造，也防未登记的名字。

## 12. 边界：本轮**不**声称什么

| 不声称 | 理由 |
| --- | --- |
| 装配已可运行 | 真正喂给模型是 runtime 装配器的事；本模块判的是合法性 |
| 装配记录已进哈希链 | 追加进 `nai-storage` 的哈希链是 runtime 接线，与 W66 对 H15 的口径一致 |
| 阈值已验证 | §41.2/§48.7：初值待黄金集回放实测，MUST NOT 采外部观察值 |
| 黄金集装配回归已可执行 | §47.2 的六类回归执行体是能力包 **H16**；本模块只提供可被它消费的判定（INV-33：判定面与证据生产分离） |
| H01/H02 已达标 | 交付的是**判定面**；`status` 保持 `pending`，`notes` 说明理由（同 W57/W66/W67/W68 惯例） |
