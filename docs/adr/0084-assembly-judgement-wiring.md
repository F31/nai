# ADR 0084：§41 装配判定的**两半**都必须在 runtime 上发生（H01 runtime 接线）

- 状态：已接受
- 日期：2026-10-14
- 轮次：W106
- 关联：[ADR 0082](0082-assembly-log-wiring.md)（H02 的 runtime 接线，本轮的模板与直接前驱）、
  [ADR 0083](0083-ramp-enablement-log-wiring.md)（H18 的同型接线）、
  [ADR 0081](0081-observation-log-wiring.md)（H15 的同型接线）、
  ~~ADR 0027~~（trace 生成器一处定义；原链接 `0027-trace-id-single-definition.md` 从未存在，见文末「勘误（W151，指针）」）、
  [ADR 0019](0019-composition-root-boundary.md)（critical 与 telemetry 的分级）

## 背景

V3.11 §41.5 要求一次上下文装配落链的记录含着**四项**：

1. **进入的片段标识**；
2. **各段预算占用**；
3. **裁剪项与原因**；
4. **模型与提示词版本**。

W104（ADR 0082）把其中**三项**接上了真实哈希链——`{fragments, model_version, prompt_version}`。
两项**没有承载物**：第 2 项与第 3 项住在 `AssemblyPlan` 上，而写入口 `record_assembly(record, fragments)`
**不接收 plan**。同一处缝还让 §41.1 的一条硬规则**在 runtime 上不可表达**：

> §41.1：不可裁段（1–3）缺失时 MUST 拒绝调用。

`evaluate_assembly(&AssemblyPlan) -> AssemblyVerdict` 判的正是这件事，而它在**生产代码里零调用方**：

```
$ grep -rn '\bevaluate_assembly\b' --include=*.rs crates apps | grep -v /tests/ | grep -v src/evaluate/
（空）
```

于是两条今天**无法被判**的命题：

1. 一次装配**缺了不可裁段**，它在 runtime 上**会不会**被拒（不是「判定面说该拒」，而是「真有调用方拒了」）？
2. §41.5 第 2、3 两项**会不会**真的落盘？

这与 W103（H15）、W104（H02）、W105（H18）**完全同型**，是**判定面存在、执行体不存在**在本仓连续
**第四轮**现身。

## 决策

### 决策 1：接线落在 `crates/nai-runtime`（与 W104/W105 同一条推法）

接线必须同时看见**判定**（`nai-kernel`）与**只追加哈希链**（`nai-storage`）。`nai-runtime` 同时依赖两者，
且它本来就是「把存储、会话、决策日志、媒体门控装配成一个受监督整体」的那一层。

- 放进 `nai-kernel` ⇒ 引入 kernel→storage 依赖（`cargo xtask check-deps` 会红）；
- 放进 `nai-storage` ⇒ 让存储层认识 kernel 的领域类型（`AssemblyPlan`/`AssemblyVerdict`）。

⇒ 落点是**推出来的**，不是偏好。**零新增依赖**。

### 决策 2：入口收下 **plan**，而不是新增第二个入口

```rust
pub fn record_assembly(
    &mut self,
    plan: &AssemblyPlan<'_>,          // ← 本轮新增的第一个参数（H01 的输入）
    record: &AssemblyRecord<'_>,      // H02 的记录侧
    fragments: &[LoggedFragment<'_>], // H02 的日志侧
) -> Result<AssemblyOutcome>;
```

`record` 与 `fragments` 各自只描述§41.5 四项里的**一部分**；第 2、3 项只在 plan 上。若为 plan 另开一个
入口（或加 setter），§41.5 的「四项描述**同一次**装配」这条不变量的责任会**再一次推给调用方**——
与 W104 决策「判定与追加合并为一个入口」同一条理由：**不变量要由结构保证，而不是由纪律保证**。

### 决策 3：结局是**四态**，不是两态或三态

两次判定各有真假，组合**恰为四种**，而每一种的后果**不同**：

| 情形 | `callable` | `rebuildable` | 结局 | 调用方 MUST NOT |
|---|---|---|---|---|
| ① | `true` | `true` | `Logged(ack)` | —— |
| ② | `false` | `true` | `NotCallable(refusal)` | **用这份装配去调模型** |
| ③ | `true` | `false` | `NotRebuildable(refusal)` | **声称这行日志可重建** |
| ④ | `false` | `false` | `Neither(refusal)` | 两件都不能做 |

把 ④ 折进 ② 或 ③ 都会**丢掉另一侧的拒绝名**：`NotCallable` 说「不许调用」，`NotRebuildable` 说
「不许声称可重建」——是两个不同的禁令，不是一个禁令的强弱。`AssemblyRefusal` 因此同时携带
`rejections`（`assembly.*`）与 `rebuild_rejections`（`rebuild.*`）**两张名字表**。

⚠️ **两次判定 MUST 各自读一次、MUST NOT 由一侧推导另一侧**。判定面自己写明了这条纪律——
`evaluate_assembly` 对**它自己的**各项检查不短路，理由原文是「a refusal that reported only the first
half would hide the second half's reasons」。runtime 是**调用方**，同一条理由逐字适用。

### 决策 4：两份输入**必须描述同一次装配** ⇒ 不一致是**调用方错误**，不是一个结局

```rust
if plan_fragment_ids(plan).as_slice() != record.fragment_ids {
    return Err(err(ErrorCode::InvalidArgument, "assembly.input_ids_differ: …"));
}
```

比的是**序列**不是集合：进入顺序**本身就是**§41.5 第 1 项的内容，而 H02 也按它判 `RebuildOrderDiffers`。

为什么是 `Err` 而不是第五个结局：§41.5 的四项**是同一个东西的四个视图**。若允许两份互相矛盾的输入
落成一条记录，那条记录会**自相矛盾**（payload 说「进入顺序是 A」，而 `rebuild_assembly` 按另一份输入判
它的可重建性）。没有任何一个结局能诚实承载它——`Logged` 是谎，任一种 `NotCallable`/`NotRebuildable`
也**指错了拒绝者**（没有任何判定拒绝了它；是**调用方把两份输入搞错了**）。

### 决策 5：载荷补齐 §41.5 的四项，取值**照抄判定输入**

```json
{
  "fragments":      [ { "id": …, "source_ref": …, "content": … } ],   // 按记录进入顺序
  "segment_usage":  [ { "segment": …, "tokens": …, "chars": …, "items": …, "budget": { … } } ],
  "crops":          [ { "segment": …, "dropped": [ … ], "reason": … } ],
  "model_version":  …,
  "prompt_version": …
}
```

- `segment_usage` 与 `crops` **照抄** `plan.slices` 里判定**看过**的那一份：不定额补零、不由 `content`
  重算、不补判定没输出的段。§41.2 的纪律是「判定是在**调用方给的数**上做出的」；一个悄悄把数改对的
  传输者，写的已经不是被审过的那本账（注入 I5 实测该重算让 **3** 个用例变红）。
- **MUST NOT 夹带**片段的 `scope`/`age_ms`/`trust`/`source`，也 MUST NOT 夹带判定的诊断清单
  （`missing_segments`/`over_budget`/`budget_missing`/… ）。判定**看过**它们是为了**决定**，
  §41.5 **没有**为落盘列它们。
- `content` MUST 落**字面正文**（W104 已立的规则，本轮不变）：H02 判的就是「日志自己能不能重排出同一段」。

### 决策 6：拒绝 ⇒ **具名**，且**一个字节都不落盘**

`AssemblyOutcome` 的三种拒绝臂各带 `AssemblyRefusal`：两张拒绝名表 + `missing_segments`（§41.1
「按名列出缺了哪几段」）+ 既有的 `unlogged_ids`/`contentless_ids`/`credential_ids`。
返回类型是枚举 ⇒「已落账」与「已拒绝」**类型上**可分（铁律二）。拒绝路径**不调用** `self.record(...)`。

### 决策 7：复用 `DecisionKind::Assembly` ⇒ 本轮**零迁移**

种类在**两个闭世界**里都已存在（W104 已付过这次迁移的代价）：Rust 枚举
（`crates/nai-storage/src/decision.rs`）与 `migrations/sqlite/0005-decision-log.sql` 的
`kind CHECK(kind IN(…))`。⇒ `migration::VERSION` 保持 **7**，不改 `schemas/**`、不改任何生成物。

**代价如实披露**：`Assembly` 属 **Telemetry** 类（`criticality()` **不**并入 Critical）⇒ 写入失败
**静默落入有界缓冲**（`ack.persisted == false`，不报错）。W105 选 `approval` 时正是因为这一点而避开了
telemetry 类；本轮**不重复那次论证**，因为 §41.5 要的是「记录已备」而装配记录本就是**可丢遥测**
（它不是授权，也不是「要么落地、要么被报告」的决定）。这个分级是 W104 的既有事实，本轮不改。

### 决策 8：逐行读回 + 历史行兼容；池化缺陷**具名登记、本轮不修**

- 新增只读入口 `read_logged_assemblies(&self, limit) -> Result<Vec<LoggedAssembly>>`：**每行一条**，
  不可解析 ⇒ `ErrorCode::Internal` 具名失败（静默跳过会让「读回 N 行」和「链上就有 N 行」**不可区分**）。
- **历史行兼容**：W104 已落的行**没有** `segment_usage`/`crops` 两键 ⇒ 读回时两项为**空列表**、**不是**解析失败。
  区分「键不存在」（旧行）与「键存在但不是数组」（损坏）是解码器必须做的事。
- ⚠️ `rebuild_assembly_from_log` 的片段池是**跨行合并**的（W104 语义，本轮**只统一解析路径、不改语义**）。
  它比 §41.5 的「日志能否重建**这一次**装配」**弱**：一个由两行片段拼出来的 record 仍可能读作可重建。
  反例：record 声明 `[A, B]`、链上第 1 行只有 `A`、第 2 行只有 `B` ⇒ 池化报可重建，而单行视角下
  **没有任何一行**能重建它。⇒ 具名登记、在 `docs/runtime.md` 与报告中披露，**留给新的一轮**。

### 决策 9：为什么 H01 现在**可以**自主推进（本轮的关键论证）

W105 的 ADR 0083 决策 7 判过「H01 卡外部前置」，并把这一轮先给了 H18。那条判定**依然成立**——
但它只对**被判的那一件事**成立，而它的依据是 `assembly.rs` 模块文档里那**一个句子**的**标点**：

> … feeding the model, **appending the record to the hash chain in nai-storage**, and executing the six
> §47.2 regression classes (capability pack H16) are runtime work.

三件事是**逗号并列**的：(i) 喂模型、(ii) 追加进链、(iii) 跑六类回归（H16）。「H01 卡外部前置」说的是
**(i)**——喂模型需要一个真实模型。

而 **H01 的判定面不消费模型**：`evaluate_assembly(&AssemblyPlan) -> AssemblyVerdict` 的输入**全部来自
调用方**（九段、每段用量与配额、裁剪记录、片段的来源标识/作用域/标注）。⇒ **(ii) 与 (i) 是可分的**，
W106 做的正是 (ii)，外加 §41.5 补上的两个承载物。

**边界仍然诚实**：`H01` 中「把九段片段真正**喂给模型**」这一半**未接线**（属 **H16**/仓库外），
故 registry 的 H01 **保持 `pending`**。

## 后果

- `crates/nai-runtime/src/assembly.rs`：`record_assembly` **签名 +1 参数**；新增四态 `AssemblyOutcome`、
  扩 `AssemblyRefusal`、`read_logged_assemblies` 与 6 个解码/取值辅助；`rebuild_assembly_from_log`
  改为复用同一解码路径（**语义不变**）。
- `crates/nai-runtime/src/lib.rs`：`assembly::*` 导出**增补**（新记录类型），既有导出不变。
- `crates/nai-runtime/tests/assembly.rs`：**25** 项（W104 的 12 项在四态下同步改写 + 本轮新增 13 项，
  其中 **2** 项是非空洞守卫，分别遍历 `AssemblyRejection::ALL`（**14**）与 `RebuildRejection::ALL`（**6**））。
- **零新增依赖、零迁移、零契约改动** ⇒ **不产生** W104 那种「本机不可再生的 TS 生成物」面。
- 规格：`runtime-composition` **+4 added / 1 modified**（**不改** `agent-harness`：其 H01 需求正文已
  **472** 字符，受 strict 的 `>500` 约束，余量太薄）。
- `evaluate_assembly` / `rebuild_assembly` 的**判定规则一行未改** ⇒ 判定面与执行面**同轮可分别验证**。
- **非空洞注入八组**（真改文件、真跑、逐字节还原、`sha256` 一致，复跑基线 **25/25** 全绿）：

| # | 注入 | 实测红数 | 拦的是什么 |
|---|---|---|---|
| I1 | 只跑 H02、不跑 H01 | **4** | 缺段拒绝 / 两侧理由 / 四态可分 / H01 非空洞守卫 |
| I2 | 载荷仍只落三键 | **4** | 键集 / 裁剪落账 / 逐行往返 / 逐行不池化 |
| I3 | H01 拒绝时提前 `return`（短路 H02） | **2** | 两侧理由 / 四态可分 |
| I4 | 把 `Neither` 折进 `NotCallable` | **2** | 两侧理由 / 四态可分 |
| I5 | `segment_usage` 由正文重算 | **3** | 键集 / 裁剪段用量 / 逐行往返 |
| I6 | 入口一致性前置删掉 | **1** | 调用方错误 |
| I7 | 逐行读回改成池化 | **1** | 逐行不池化 |
| I8 | 历史行（缺两键）被当成解析失败 | **1** | 历史行兼容 |

- **宿主机实测**：`LNK1104`（`link.exe` exit 1104）在注入期间命中 **6** 次；判据＝**重试即可过**
  ⇒ **非代码缺陷**，处置是**重试、绝不改代码**（与 KNOWLEDGE §11 既有记录一致）。
- **诚实边界**：`H01` 保持 **`pending`**。本轮可测的是「§41.1 的拒绝**在 runtime 上有表达**」与
  「§41.5 的四项真的落盘且能逐行读回」；**喂模型**这一半（**H16**）与 **§41.2 分段预算初值标定**
  （§47.2 黄金集回放，§48.7 禁采外部观察值）**仍未做**；`rebuild_assembly_from_log` 的**池化弱化**
  已具名登记（决策 8）⇒ 报告 MUST NOT 出现「装配器已接入运行期」「已不依赖当时内存」这类运行期断言。

## 替代方案

1. **只加 `segment_usage`，不接 H01**：能把 §41.5 四补到三加一，但 §41.1 的「缺不可裁段 ⇒ 拒绝调用」
   仍是**不可表达**的，而这正是 H01 的核心条款；且会把缝留成「判定面零调用方」的第六轮。⇒ 不采纳。
2. **两态（`Logged`/`Refused`）或三态**：省枚举变体，但丢掉另一侧的拒绝名（决策 3），
   注入 I3/I4 实测各让 **2** 个用例变红。⇒ 采纳四态。
3. **把 `plan` 并进 `AssemblyRecord`**：让 record 同时携带判定输入与记录侧——但那样 `record` 就
   **描述两次装配**（判定输入的 plan + 日志记录的 fragments），§41.5 的四项会指向不同对象。
   ⇒ 不采纳（保持三参数）。
4. **一致性前置做成第五个结局**（如 `InputsDisagree`）：可以让调用方也走同一个 `match`——但会让
   「**调用方错误**」与「**判定拒绝**」看起来是同一类事（决策 4）。⇒ 不采纳。
5. **`segment_usage` 由 `content` 重算**：一行更短，但写出的不是被审过的那本账（决策 5），
   I5 实测 **3** 红。⇒ 不采纳。
6. **顺手把 `rebuild_assembly_from_log` 的池化改成逐行**：语义更贴 §41.5，但会**改掉 W104 的既有行为**
   且把两件事混进一轮（本轮是「H01 接线」，那是「H02 read 侧语义修正」）。⇒ 不做，具名登记（决策 8）。
7. **新增 `DecisionKind::AssemblyUsage` + 迁移 `0008`**：语义更细，但要付 W104 那条
   `db_schema_version` 连带面，而 `cargo xtask schema --check` 在宿主机 `EBUSY`、**本机跑不了**
   ——**本机验不了的面**正是「可自主推进」要避开的。⇒ 不采纳。

## 关联

- 规格：`openspec/specs/runtime-composition/spec.md` 的 4 条 added + 1 条 modified
- 变更：`openspec/changes/archive/2026-10-14-w106-assembly-judgement-wiring/`
- 报告：[`docs/reports/2026-10-14-w106-assembly-judgement-wiring.md`](../reports/2026-10-14-w106-assembly-judgement-wiring.md)
- 前序：W69（判定面）、W103／W104／W105（连续三轮同型接线）——同型的「缺失对象」轮次

## 勘误（W127，2026-10-16）

**决策 6（「顺手把 `rebuild_assembly_from_log` 的池化改成逐行 ⇒ 不做」）与决策 8 所具名登记的弱化，
已由 W127 闭合。** 本文档其余内容按当时状态保留，MUST NOT 就地下改。

- 当时判断「那是『H02 read 侧语义修正』、属另一个对象」——**这个判断成立**，W127 就是那个独立对象。
- **更正口径来源**：当时把弱化描述成「比 §41.5 的『日志能否重建**这一次**装配』弱」。
  §41.5 原文（V3.11:1971）**只说**「装配 MUST 落决策日志……」＋「装配记录 MUST NOT 含明文凭据」，
  **未**明文要求「单个日志条目」。⇒ 收紧的登记依据是 **`tests/registry.yaml` 的 H02 标题**
  「……重建**同一**上下文」，MUST NOT 把依据写成「§41.5 要求单条目」。口径链见 ADR 0105 决策 1。
- **更正一处可能的误读**：决策 6 与决策 8 里「改它＝改掉 W104 的既有行为」**不是**说 W104 的行为是错的——
  W104 的行为在当时**被具名登记**、没有冒充达标；W127 是它登记的后续对象。
- W127 的落地形状：逐行作为独立**条目**判定（一个条目就是一次装配）；承载判据 = 片段标识**多重集**
  （含重复、**不计顺序**）；判定为**存在性**；无条目承载 ⇒ 具名 `rebuild.not_in_one_entry`，
  且 `unlogged_ids` 对**整条链**重新派生。见 [ADR 0105](0105-rebuild-in-one-entry.md)。

## 勘误（W151，指针）

关联列表里的 `ADR 0027` 原链接到 `0027-trace-id-single-definition.md`，**该文件从未存在**。
`git log --follow docs/adr/0027-deployment-lifecycle-boundary.md` 显示 ADR 0027 自创建起就是
「部署生命周期的边界」，与 trace 生成器无关。W151 新增的链接判据（ADR 0125）把它报成断链，
处置是**降级为纯文本**，不是改指一个编号：

- 「trace 生成器一处定义」的实际落点是 **ADR 0081 决策 6**——「`next_trace` 放宽到 `pub(crate)`，
  **不**新增第二个 trace 生成器」，它应用的是 ADR 0064「一处定义、两处使用」；
- **编号不可复原**：原文写的 `0027` 与任何 trace 相关的 ADR 都不对应 ⇒ MUST NOT 猜一个上去。
  猜错会把一条断链换成一个**看起来能点、内容不对**的指针，那比断链更坏；
- 正文里同源的「ADR 0027『一处定义』」写法（本 ADR 的正文段）指涉同一件事，同样不可复原，
  保留原文不改写——权威表述是本段。
- `ADR 0015` 原链接到 `0015-decision-log-criticality.md`，同样**从未存在**：ADR 0015 是
  「V3.6 升为唯一实现基线」。「critical 与 telemetry 的分级」的正文出处是 **ADR 0019 决策 5**
  （`docs/adr/0019-composition-root-boundary.md:25` 逐字：「决策日志分级：critical
  （cancellation/duty_lease 等）失败即错误；telemetry 允许缓冲」）⇒ 该处已**就地订正**为
  ADR 0019。这是两类处置的差别：**指错了但指得回来 ⇒ 订正**；**指的东西不存在 ⇒ 降级 + 勘误**。
