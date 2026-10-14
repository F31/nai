# Design — W106 装配判定接上 runtime 装配入口（H01 runtime 接线）

## 1 缝在哪里

W69 交付了 `evaluate_assembly`（判定：一次装配可不可调用），并在 `assembly.rs` 的**模块文档**里
写死了边界：

> **Scope boundary.** Budget **values** are supplied by the caller, never built in here… Feeding the model,
> appending the record to the hash chain in `nai-storage`, and executing the six §47.2 regression classes
> (capability pack H16) are runtime work and are **not** claimed here.

W104 把这句里的「appending the record to the hash chain」补**了一半**。实测余下的一半：

```
$ grep -rn '\bevaluate_assembly\b' --include=*.rs crates apps | grep -v /tests/ | grep -v src/evaluate/ | wc -l
0
$ grep -rn "evaluate::assembly::" --include=*.rs crates apps | grep -v /tests/ | grep -v "crates/nai-kernel/src/evaluate/"
crates/nai-runtime/src/assembly.rs:4  ← 仅文档注释
crates/nai-runtime/src/assembly.rs:32 ← 仅 use（只引 rebuild_assembly 一族）
```

`evaluate_assembly` **零生产调用方**，而 W104 的 `record_assembly` 收到的只有
`AssemblyRecord`（= `fragment_ids` / `model_version` / `prompt_version`）与 `LoggedFragment[]`——
**它拿不到 `AssemblyPlan`**，也就没有参数能携带九段、裁剪与分段预算。于是：

| §41.5 要求落账的四项 | 今天在 runtime 上的承载物 |
|---|---|
| 进入的片段标识 | ✅ `payload.fragments[].id`（W104） |
| **各段预算占用** | ❌ **无** |
| **裁剪项与原因** | ❌ **无** |
| 模型与提示词版本 | ✅ `payload.model_version` / `prompt_version`（W104） |

以及一条**不可表达**的拒调用规则：§41.1 的「不可裁剪段（1–3）缺失时 MUST 拒绝调用」
（`AssemblyRejection::UncroppableSegmentMissing`）在 runtime 上根本无从触发——没有入口接收 plan。

本轮把这两条一并补上。

### 1.1 与 W105 design §1.1 的辨析：H01 卡不卡外部前置

W105 的 `design.md` §1.1 把 H01 与 H18 并列为同批「可自主推进」候选，并判定 **H01 卡外部前置**：

| | runtime 步是什么 | 卡不卡外部前置 |
|---|---|---|
| H01 | 把九段片段**喂给模型**（真实装配器） | **卡** —— 模型是 H16/外部 |
| H18 | 把判定**已经算好**的记录**追加进链** | **不卡** |

**那条判定只对被判的那一件事成立。** 依据是边界原文自己的**标点**：它用**逗号并列**了三件事，
而不是用同位语说一件事：

> Feeding the model, **appending the record to the hash chain in `nai-storage`**, and executing the six
> §47.2 regression classes (capability pack H16) are runtime work

即 **(i) 喂模型、(ii) 追加进链、(iii) 跑六类回归（H16）**，三项并列。H01 的判定面**不消费模型**：
`evaluate_assembly(&AssemblyPlan) -> AssemblyVerdict` 的输入全部来自调用方——片段、用量、预算、裁剪
都是**调用方给的**（这正是 §41.2 「阈值的取值由策略给定」的形状）；输出只有「可不可调用、为什么」。
⇒ **(ii) 与 (i) 可分**，W105 §1.1 那条判定 MUST NOT 被读作覆盖了 (ii)。

这与 W105 论证 H18 时用的依据同型（「判定面自证：`ramp.rs` 的 gate only consumes a declared `passed`
set and never runs a case, so H18 stays decidable with no capability pack installed」）——两轮的判据是
**同一条**：**判定面的输入不来自外部，则「判定之后那一步」在仓库内可判**。

本项目已记过这个形状：「**一条待裁定项的处置可以被它自己的推荐文本否证** ⇒ 核实 MUST 读它自己的原文」。
W105 §1.1 的原文只说了「喂模型」；本轮的原文依据是同一段里的**并列第二项**。

## 2 为什么落在 `crates/nai-runtime`

接线需要同时看见**判定**与**只追加哈希链**，而这两个能力分属两个 crate：

| 需要的面 | 位置 |
|---|---|
| `evaluate_assembly` / `AssemblyPlan` / `SegmentSlice` / `AssemblyVerdict` / `AssemblyRejection` | `crates/nai-kernel/src/evaluate/assembly.rs` |
| `Store::append_decision` / `DecisionKind` / `DecisionAck` | `crates/nai-storage/src/decision.rs` |

`crates/nai-runtime/Cargo.toml` 同时持有 `nai-kernel`（path）与 `nai-storage`（path）——**这是唯一
既能看到判定、又能看到追加的落点**（W103/W104/W105 已在此落过三次同型接线）。放在 kernel 里会引入
kernel→storage 的越界依赖，放在 storage 里会让存储层认识 kernel 的领域类型；两者都违反既有依赖方向
（`cargo xtask check-deps` 强制）。**零新增依赖**。

## 3 存储面：复用 `DecisionKind::Assembly`（零迁移）

W104 新增 `DecisionKind::Assembly` 的代价是**一次迁移**（`0007-decision-log-assembly-kind.sql`），
因为种类是**两处定义**：Rust 枚举 **和** `0005-decision-log.sql` 的 `kind CHECK(kind IN(…))`。

本轮**不付这个代价**：`assembly` 在**两个闭世界里都已存在**（Rust `decision.rs:32`；SQL `0005`）。
⇒ **不新增变体、不新增迁移、不改 `migration::VERSION`（保持 7）、不改 `db_schema_version`。**

### 3.1 如实披露：`assembly` 是 **telemetry** 类

`DecisionKind::criticality()` **不**把 `Assembly` 并入 Critical 表 ⇒ 它的写入失败**进有界遥测缓冲**
（`ack.persisted == false`），而不是原样报错。W104 的首跑就是被这一点抓出来的。

本轮沿用 W104 的选择，**不改分类**（改分类属存储面语义变更，且会让 W104 的既有行意义改变）。
但这条事实 MUST 留在文档里，且 W104 已有的测试
`an_oversized_assembly_payload_is_an_error_not_a_silent_truncation` 继续承重：
载荷超限走 `Err`，不是静默截断。⇒ **「要么落地、要么被报告」在本轮仍是可判的**，只是那条
「被报告」的路径是 `Err`（超限）而非「Critical 类失败即 `Err`」。

## 4 接口形状：两个判定合并为一处，且**四态**

```rust
// crates/nai-runtime/src/assembly.rs
pub struct AssemblyRefusal {
    pub rejections: Vec<&'static str>,          // §41.1–§41.3 的 assembly.* 标签，顺序同判定输出
    pub rebuild_rejections: Vec<&'static str>,  // §41.5 的 rebuild.* 标签，顺序同判定输出
    pub missing_segments: Vec<&'static str>,    // §41.1 缺席的不可裁段（segment label）
    pub unlogged_ids: Vec<String>,              // 引用了却不在日志里的 id
    pub contentless_ids: Vec<String>,           // 正文只在当时的 id
    pub credential_ids: Vec<String>,            // 正文含明文凭据的 id
}

pub enum AssemblyOutcome {
    Logged(DecisionAck),                 // 两侧都放行 ⇒ 已追加一条 assembly 条目
    NotCallable(AssemblyRefusal),        // 仅 §41.1–§41.3 拒绝
    NotRebuildable(AssemblyRefusal),     // 仅 §41.5 重建拒绝
    Neither(AssemblyRefusal),            // 两侧都拒绝
}

impl Composition {
    pub fn record_assembly(
        &mut self,
        plan: &AssemblyPlan<'_>,          // H01 输入
        record: &AssemblyRecord<'_>,      // H02 输入
        fragments: &[LoggedFragment<'_>],
    ) -> Result<AssemblyOutcome>;
}
```

### 4.1 为什么把两个判定与追加放在同一个方法里

与 W103/W104/W105 同理由：暴露「先 `evaluate_assembly` 再 `append`」两个出口，会把
「不许把未放行或不可重建的装配落进链」这条不变量的责任**推给每个调用方**——只要有一个人忘了先判定，
不变量就破了，而测试看不出来（W92/W93「把门槛交给被审者自己填」的形状）。
合并成一个入口后，**没有**一条路径能不经判定而追加。

### 4.2 为什么必须是**四态**（本轮的实质设计决定）

两次判定的结果有**四种组合**，每一种的后果都不同：

| 组合 | §41.1–§41.3（可调用） | §41.5（可重建） | 应当报成 |
|---|---|---|---|
| ① | ✅ 放行 | ✅ 放行 | **已落账** |
| ② | ❌ 拒绝 | ✅ 放行 | **不可调用**（不许拿它去调用模型） |
| ③ | ✅ 放行 | ❌ 拒绝 | **不可重建**（不许拿它当可重建的记录） |
| ④ | ❌ 拒绝 | ❌ 拒绝 | **两者皆非**（两类原因都要报） |

把 ② 与 ③ 折成同一个「拒绝」是**谎**：②的处置是「这次调用不许发生」（§41.1 的 MUST 拒绝调用），
③的处置是「这次调用的记录不许声称可重建」（§41.5）。把 ④ 折进 ②（或 ③）会**丢掉另一侧的拒绝名**
——而「拒绝类别不互相遮蔽」是本仓的既有纪律（`evaluate_assembly` 自己的文档就写着
"none of them short-circuits… a reader asking 'why was this refused' is not served by the first hit alone"）。

这正是本项目记过的形状：「**「记成什么」答不出 ⇒ 先问分类是否少一格**」；也是 W105 用三态的理由
（`NoRecordRequired` 塌向任何一边都是谎）的同一条教训。

⚠️ **实现纪律**：四态**不能**靠「先算 H01、拒绝就返回」反推。运行期 MUST **两个判定各自调用一次**、
各自读自己的 `callable` / `rebuildable`，再按上表分派。

### 4.3 两个判定都不短路

`evaluate_assembly` 本身**不短路**（它的文档原话：「a plan that is both short and out of order reports
both」）。本轮的**跨判定**层面沿用同一纪律：H01 拒绝时 H02 照跑，反之亦然。
⇒ `AssemblyOutcome::Neither` 的拒绝里**同时**含 `assembly.*` 与 `rebuild.*` 两类名字。

## 5 载荷的形状：五项键，逐键指名它由谁放行

两侧都放行时，追加的载荷**只有五项**：

```json
{
  "fragments":      [ { "id": …, "source_ref": …, "content": … }, … ],
  "segment_usage":  [ { "segment": …, "tokens": …, "chars": …, "items": …,
                        "budget": { "tokens": …, "chars": …, "items": …, "floor": … } | null }, … ],
  "crops":          [ { "segment": …, "dropped": [ … ], "reason": … }, … ],
  "model_version":  "…",
  "prompt_version": "…"
}
```

**逐键归属**（每个键 MUST 能指名到 §41.5 的一项，或指名到判定为该项结论所**审过**的输入）：

| 键 | §41.5 的哪一项 | 由哪条判定放行 |
|---|---|---|
| `fragments` | 进入的片段标识 | **H02**：`rebuild_assembly` 审过每个 id 有日志条目、有正文、顺序一致 |
| `segment_usage` | 各段预算占用 | **H01**：`SegmentUsage` 与 `SegmentBudget` 正是 `SegmentOverBudget`／`BudgetDimensionMissing`／`UncroppableWithoutFloor`／`SegmentBudgetMissing` 四条拒绝的直接输入 |
| `crops` | 裁剪项与原因 | **H01**：`Crop{dropped, reason}` 正是 `CropWithoutDroppedItems`／`CropWithoutReason`／`CropFromFront` 三条拒绝的直接输入 |
| `model_version` / `prompt_version` | 本次调用的模型与提示词版本 | **H02**：`ModelVersionMissing`／`PromptVersionMissing` 两条拒绝的直接输入 |

不变式：

- **载荷 MUST 恰为判定放行的那一份材料**。runtime 是**传输者**，不是加工者：
  `segment_usage` 与 `crops` 的值**照抄**判定输入里的 `usage` / `budget` / `crop`，
  **MUST NOT** 由片段正文重新计算、MUST NOT 补零、MUST NOT 改写。
- **MUST NOT 夹带判定没审过的字段**：片段的 `scope`／`age_ms`／`trust`／`source` 一律**不落**
  （§41.5 的清单里没有它们，把它们落账就是把「传输者」改成「加工者」）；
  判定产出的**诊断清单**（`missing_segments`／`over_budget`／`budget_missing`／
  `unprovenanced_fragments`／`unscoped_fragments`／`untrusted_as_instruction`）只用于**拒绝路径**
  的具名回报，MUST NOT 落进载荷。
- 测试用**键集断言**钉前两条（顶层恰为 5 键；`fragments` 每项恰为 3 键；`crops` 每项恰为 3 键）。

## 6 入口一致性前置：两份输入 MUST 描述同一组片段

`record_assembly` 收**两份独立**输入：`plan` 的九段（各段带自己的 `fragments`）与 `record` 的
`fragment_ids`。**它们描述的是同一次装配的两个视角**，§41.5 的四项也因此是并列的同一份记录。

⇒ 入口 MUST 校验：**把 `plan.slices[].fragments[].id` 按段序拼接得到的序列**，与
`record.fragment_ids` **逐项相等**；不等 ⇒ 具名拒绝（`assembly.input_ids_differ`），零新增。

**为什么必需**：否则链上会出现**自相矛盾**的一行——「各段预算占用」说这一段进了 X，
「进入的片段标识」说进的是 Y，读者无法知道哪一份是真的。这等于把「同一件事 MUST NOT 两处定义」
破在一个更难看的位置上（两处定义、且互斥）。

⚠️ **为什么是序列相等而不是集合相等**：进入顺序本身就是 §41.5 第一项的一部分，且 H02 用
`RebuildOrderDiffers` 把「顺序」钉成了可判事实。集合相等会让「A、B」与「B、A」都被放行，
与该纪律不一致。

⚠️ **这条前置不是「第二份判定」**：它不重复任何 kernel 结论，只是把「两份输入必须指同一件事」
这个**入口前置条件**摆到明面上（W93 教训：新增必填字段 MUST 问「它钉的那个可判事实，是否就是这个态
在声明的那件事」）。判定规则仍**只有** kernel 那一处。

## 7 逐行读回、池化重建与历史行的兼容

新增只读入口：

```rust
pub struct LoggedAssembly {              // 一行 = 一次装配
    pub fragments: Vec<(String, String, Option<String>)>,  // (id, source_ref, content)
    pub segment_usage: Vec<LoggedSegmentUsage>,            // (segment, tokens, chars, items, budget?)
    pub crops: Vec<LoggedCrop>,                            // (segment, dropped[], reason)
    pub model_version: String,
    pub prompt_version: String,
}
pub fn read_logged_assemblies(&self, limit: u32) -> Result<Vec<LoggedAssembly>>;
```

三条约束：

1. **逐行，不池化**。把多行合并成一个池会让「各段预算占用」失去可归属的行——W104 的
   `rebuild_assembly_from_log` 正是池化（`owned.extend(decode_fragments(...))`）。
2. **不可解析 ⇒ 具名失败，MUST NOT 静默跳过**。跳过会让「读回了 N 行」与「链上有 N 行」无法区分
   （沿用 W104 `decode_fragments` 的既有纪律：「A row that is not decodable is a **classified failure**,
   never a silent skip」）。
3. **历史行兼容**：W104 已落的行**没有** `segment_usage` / `crops` 两键。读回时它们缺席 ⇒
   两个列表**为空**，**不是**解析失败。这一条 MUST 有专门用例（否则「新老行都能读」是空话）。

`rebuild_assembly_from_log` 的 **payload 解析**改为复用本入口（消除第二份解析逻辑），
其**池化判定语义保持不变**——见 §9 的披露。

## 8 非空洞守卫与注入表

沿用 W54/W66/W103/W104/W105 的做法：**真改 `crates/nai-runtime/src/assembly.rs` + 真跑 +
逐字节还原 + `sha256` 一致**（基准 = 过完 `cargo fmt --all` 之后的那一份）。

**非空洞守卫（H01 侧，本轮新增）**：一个测试遍历 `AssemblyRejection::ALL`（**14** 条），对每个拒绝
**构造一个运行期 plan** 并断言拒绝名出现在 `AssemblyOutcome::NotCallable`／`Neither` 的 `rejections` 里。
守卫的机械性来自**穷尽 `match`**：样本构造函数 `match rejection { … }` 覆盖全部变体 ⇒ kernel 往
`ALL` 加一个变体而 runtime 未处理，**编译即失败**。这与 W104 已有的
`every_rebuild_rejection_is_reachable_through_the_runtime`（遍历 `RebuildRejection::ALL` 六条）并存，
两者覆盖**两个不同的枚举**。

⚠️ **样本定位 MUST NOT 按「态」定位**（W92 教训）：样本按**被断言对象的属性**构造
（如「某不可裁段的 `usage` 缺一个维度」），而非遍历一张按结局分组的清单。

**注入表**（每组 MUST 让某个用例变红）：

| # | 注入 | 拦的是什么 |
|---|---|---|
| I1 | 只跑 H02、不跑 H01（把 H01 判定从入口里删掉） | §41.1 的「不可裁段缺失即拒调用」是否真的承重 |
| I2 | 载荷仍只落三键（不落 `segment_usage`／`crops`） | §41.5 的四项是否真的补齐 |
| I3 | H01 拒绝时提前 `return`（短路掉 H02） | §4.3 的「两类都报」与非短路 |
| I4 | 把 `Neither` 折进 `NotCallable`（丢掉 `rebuild.*` 名字） | 四态可分性（§4.2 的要害） |
| I5 | `segment_usage` 由正文重新计算而非照抄判定输入 | 「传输者 vs 加工者」：取值是否真来自判定审过的输入 |
| I6 | 一致性前置删掉（plan 与 record 的 id 序列不同也放行） | §6 的入口前置 |
| I7 | 逐行读回改成池化（多行合并） | §7 的可归属行；且「历史行兼容」用例会不会跟着变红 |
| I8 | 历史行（缺两键）被当成解析失败 | 「新老行都能读」是否真的被断言 |

## 9 诚实边界

- 本轮的判据是「**判定之后**的那一步真的发生」：两侧都放行才落账、任一拒绝即具名且零新增、
  §41.5 的四项真的进链、且能逐行读回。
- **「把九段片段喂给模型」的装配器仍未接线**（§41.1 的「每次调用付费模型前」）——
  模型与真实装配器是能力包 **H16**／外部。故 H01 的 `status` 保持 `pending`。
  W106 只做 (ii)「追加进链」，MUST NOT 被读作 (i) 已完成。
- **§41.2 的「用 §47.2 黄金集回放调参并记录 ADR」仍未做**：判定面只**消费**调用方给的预算值，
  从不产出预算初值（这正是它能保持可判定的原因）。故「各段预算占用」落的是**调用方给的那一份**，
  不是「调优后的那一份」。
- **已披露的既有缺陷（本轮不修）**：`rebuild_assembly_from_log` 的判定是**池化**的——它把所有
  `assembly` 行的片段并成一个池，再按 `record.fragment_ids` 判定顺序。反例：
  record 声明进入 `[A, B]`，链上第 1 行只有 `A`、第 2 行只有 `B` ⇒ 池化会报「可重建」，
  但那两次装配**都不是** record 描述的那一次。逐行判定才是 §41.5 的语义。
  本轮**只统一解析路径**，不改该语义（改它属 W104 既有行为的变更，需要独立一轮），
  故在此**具名登记为遗留**，并给出上面这个可复现的反例构造。
- **不改任何验收 `status`**：H01 仍 `pending`（判定面 + 落链 + 读回已可测，但「本系统里真实装配一次
  上下文并喂给模型」还不是事实）。**如实披露不是缺陷，不实声明才是。**
