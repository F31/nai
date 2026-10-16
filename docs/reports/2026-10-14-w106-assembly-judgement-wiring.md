# W106 轮次报告：H01 的 runtime 接线——§41 装配判定的**两半**都必须真的发生

- 日期：2026-10-14
- 变更：`openspec/changes/2026-10-14-w106-assembly-judgement-wiring/`
- ADR：[0084](../adr/0084-assembly-judgement-wiring.md)
- 改写模块：`crates/nai-runtime/src/assembly.rs`
- 测试：`crates/nai-runtime/tests/assembly.rs`（**25** 项：本轮新增 **13**，W104 的 **12** 项在四态下同步改写）
- **零迁移、零契约改动**（复用 `DecisionKind::Assembly`；不改 `schemas/**` 与任何生成物）
- 前置：W69（判定面）、W104（H02 同型接线，本轮的模板）、W105（H18）

## 一、目标

R1 的未达成验收项里，按「瓶颈是否在仓库外」拆出一组**可自主推进**项。W103 做了 **H15**、W104 做了 **H02**、
W105 做了 **H18**，本轮做 H01——§41 装配判定的**另一半**。W105 的 ADR 0083 决策 7 曾判「H01 卡外部前置」
并因此先做了 H18；那条判定**依然成立**，但它只对**其中一件事**成立（见 §2.4）。

## 二、缝在哪

### 2.1 直接证据

`nai_kernel::evaluate::assembly::evaluate_assembly` 判 §41.1–§41.3，它此前在**生产代码里零调用方**：

```
$ grep -rn '\bevaluate_assembly\b' --include=*.rs crates apps | grep -v /tests/ | grep -v src/evaluate/
（空）
```

而 W104 落地的写入口 `record_assembly(record, fragments)` **不接收 plan**。后果是两条**同时**成立的缺口：

1. §41.1 的硬规则「不可裁段（1–3）缺失时 MUST 拒绝调用」**在 runtime 上不可表达**——没有任何运行期形状
   能承载它，因为判定需要的那份输入（九段序列）根本进不了入口。
2. §41.5 要求落链**四项**，而 W104 的载荷只有**三键**（`fragments`／`model_version`／`prompt_version`）
   ——第 2 项「各段预算占用」与第 3 项「裁剪项与原因」**住在 plan 上**，没有承载物。

### 2.2 缝型归属

**判定面存在、执行体不存在**——本仓**连续第四轮**（W103 H15 → W104 H02 → W105 H18 → W106 H01）。
本轮与前三轮的**不同点**：它不是「判定面完全没有调用方」，而是**同一个判定面上只有一半接了线**。
⇒ 这给出一条**新的识别判据**：一个模块若把**两件事**判在一处（这里：能不能调用 / 能不能重建），
接线时要问「**这两件事是不是都真有调用方**」——W104 接的是后一半，前一半**静默留在原地**。

### 2.3 落点是推出来的，不是偏好

| 需要的面 | 位置 | 谁依赖它 |
|---|---|---|
| `evaluate_assembly` / `rebuild_assembly` / `AssemblyPlan` / `AssemblyVerdict` | `crates/nai-kernel/src/evaluate/assembly.rs` | `nai-kernel` 导出 |
| `Store::append_decision` / `DecisionKind` / `DecisionAck` | `crates/nai-storage/src/decision.rs` | `nai-storage` 导出 |

`nai-runtime` 同时依赖两者 ⇒ 放进 kernel 会引入 kernel→storage 越界依赖，放进 storage 会让存储层认识
kernel 的领域类型。两者都被 `cargo xtask check-deps` 拦 ⇒ **零新增依赖**。

### 2.4 本轮的核心论证：H01 到底卡不卡外部前置

W105 的 ADR 0083 决策 7 判过「H01 卡外部前置」，并把那一轮先给了 H18。这条判定的依据是
`assembly.rs` 的**模块文档**里那**一个句子**：

> … feeding the model, **appending the record to the hash chain in nai-storage**, and executing the six
> §47.2 regression classes (capability pack H16) are runtime work.

三件事是**逗号并列**的：**(i)** 喂模型、**(ii)** 追加进链、**(iii)** 跑六类回归（H16）。
「H01 卡外部前置」说的是 **(i)**——喂模型需要一个真实模型，那在仓库外。

而 **H01 的判定面不消费模型**：`evaluate_assembly(&AssemblyPlan) -> AssemblyVerdict` 的输入**全部来自调用方**
（九段、每段用量与配额、裁剪记录、片段的来源标识／作用域／标注）。
⇒ **(ii) 与 (i) 可分**，W106 做的正是 (ii) + §41.5 补上的两个承载物。

⇒ **方法论产出**：「一条『卡外部前置』的判定 MUST 指明它卡的是哪一件事」——**并列句里的一件卡外部，
不等于整句卡外部**（写进 `KNOWLEDGE.md` §6.29）。

## 三、设计（详见 [ADR 0084](../adr/0084-assembly-judgement-wiring.md)）

### 决策 1：入口**收下 plan**，而不是新增第二个入口

```rust
pub fn record_assembly(
    &mut self,
    plan: &AssemblyPlan<'_>,          // ← 本轮新增（H01 的输入）
    record: &AssemblyRecord<'_>,      // H02 的记录侧
    fragments: &[LoggedFragment<'_>], // H02 的日志侧
) -> Result<AssemblyOutcome>;
```

第 2、3 项只在 plan 上。若为 plan 另开入口或加 setter，「§41.5 的四项描述**同一次**装配」这条不变量的
责任会**再一次推给调用方**——与 W104「判定与追加合并为一个入口」同一条理由：**由结构保证，而不是由纪律保证**。

### 决策 2：结局是**四态**，且**必须**四态（本轮的实质设计决定）

| 情形 | `callable` | `rebuildable` | 结局 | 调用方 MUST NOT |
|---|---|---|---|---|
| ① | `true` | `true` | `Logged(ack)` | —— |
| ② | `false` | `true` | `NotCallable(refusal)` | **用这份装配去调模型** |
| ③ | `true` | `false` | `NotRebuildable(refusal)` | **声称这行日志可重建** |
| ④ | `false` | `false` | `Neither(refusal)` | 两件都不能做 |

把 ④ 折进 ② 或 ③ 会**丢掉另一侧的拒绝名**：`NotCallable` 说「不许调用」、`NotRebuildable` 说
「不许声称可重建」——是**两个不同的禁令**，不是一个禁令的强弱。故 `AssemblyRefusal` 同时携带
`rejections`（`assembly.*`）与 `rebuild_rejections`（`rebuild.*`）**两张名字表**。
⇒ 注入 **I3**（短路）与 **I4**（折态）实测各让 **2** 个用例变红（见 §七）。

### 决策 3：两份输入**必须描述同一次装配** ⇒ 不一致是**调用方错误**（`Err`），不是第五个结局

比的是**序列**不是集合（进入顺序**本身就是**§41.5 第 1 项的内容，H02 也按它判 `RebuildOrderDiffers`）。
为什么不做成结局：§41.5 的四项**是同一个东西的四个视图**；允许两份矛盾的输入落成一条记录，那条记录会
**自相矛盾**，且任何结局都会**指错拒绝者**（没有任何判定拒绝了它）。⇒ 返回
`Err(ErrorCode::InvalidArgument)`，消息带 `assembly.input_ids_differ`。
⇒ 注入 **I6** 实测 **1** 红。

### 决策 4：载荷补齐四项，取值**照抄判定输入**

```json
{
  "fragments":     [ { "id": …, "source_ref": …, "content": … } ],  // 按记录进入顺序
  "segment_usage": [ { "segment": …, "tokens": …, "chars": …, "items": …, "budget": { … } } ],
  "crops":         [ { "segment": …, "dropped": [ … ], "reason": … } ],
  "model_version": …,
  "prompt_version": …
}
```

- `segment_usage`／`crops` **照抄** `plan.slices` 里判定**看过**的那一份：不定额补零、不由 `content` 重算、
  不补判定没输出的段。§41.2 判的是**调用方给的数**；一个悄悄把数改对的传输者，写的已经不是被审过的那本账。
  ⇒ 注入 **I5** 实测该重算让 **3** 个用例变红。
- **MUST NOT 夹带**片段的 `scope`／`age_ms`／`trust`／`source`，也 MUST NOT 夹带判定诊断清单
  （`missing_segments`／`over_budget`／`budget_missing`／…）。判定**看过**它们是为了**决定**，§41.5 没为落盘列它们。
- `content` MUST 落**字面正文**（W104 已立的规则，本轮不变）。

### 决策 5：拒绝 ⇒ **具名**，且**一个字节都不落盘**

三种拒绝臂各带 `AssemblyRefusal`：两张拒绝名表 + `missing_segments`（§41.1「按名列出缺了哪几段」）
+ 既有的三类可疑 id。返回类型是枚举 ⇒「已落账」与「已拒绝」**类型上**可分。拒绝路径**不调用** `self.record(...)`。

### 决策 6：复用 `DecisionKind::Assembly` ⇒ 本轮**零迁移**

种类在**两个闭世界**里都已存在（W104 已付过这次迁移的代价）：Rust 枚举与
`migrations/sqlite/0005-decision-log.sql` 的 `kind CHECK(kind IN(…))`。⇒ `migration::VERSION` 保持 **7**，
不改 `schemas/**`、不改任何生成物、**不产生** W104 那种「本机不可再生的 TS 生成物」面。
**代价如实披露**：`Assembly` 属 **Telemetry** 类 ⇒ 写入失败**静默落入有界缓冲**（`ack.persisted == false`）。
本轮**不重复** W105 那次「必须落在 Critical」的论证：装配记录本就是**可丢遥测**（不是授权、
不是「要么落地要么被报告」的决定）。该分级是 W104 的既有事实，本轮不改。

### 决策 7：逐行读回 + 历史行兼容；池化缺陷**具名登记、本轮不修**

- `read_logged_assemblies(limit)`：**每行一条** `LoggedAssembly`；不可解析 ⇒ `ErrorCode::Internal` 具名失败
  （静默跳过会让「读回 N 行」与「链上就有 N 行」**不可区分**）。
- **历史行兼容**：W104 落的行**没有** `segment_usage`／`crops` 两键 ⇒ 读回时两项为**空列表**、**不是**解析失败。
  「键不存在」（旧行）与「键存在但不是数组」（损坏）被分开处置。⇒ 注入 **I8** 实测 **1** 红。
- ⚠️ `rebuild_assembly_from_log` 的片段池是**跨行合并**的（W104 语义，本轮**只统一解析路径、不改语义**）。
  反例：record 声明 `[A, B]`、链上第 1 行只有 `A`、第 2 行只有 `B` ⇒ 池化报可重建，而**没有任何一行**
  能单独重建它 ⇒ 它比 §41.5 的「日志能否重建**这一次**装配」**弱**。**具名登记**在 `docs/runtime.md` 与本报告 §十。

## 四、零迁移是怎么拿到的

`DecisionKind::Assembly` 在 Rust 枚举（`crates/nai-storage/src/decision.rs`）与
`migrations/sqlite/0005-decision-log.sql` 的 `kind CHECK(kind IN(…))` **两个闭世界里都已存在**
（W104 为它付过迁移 `0007` 的代价）。⇒ 本轮**不新增种类、不加迁移**。

## 五、实现

- `crates/nai-runtime/src/assembly.rs`（**整文件重写**）：
  - `record_assembly` 签名 **+1 参数**；入口一致性前置；两次判定各调一次；四态结局；
  - `read_logged_assemblies`（新）与 6 个解码／取值辅助（`decode_assembly`／`decode_fragments`／
    `decode_segment_usage`／`decode_crops`／`required_str`／`required_u64`／`optional_u64`）；
  - `rebuild_assembly_from_log` 的 payload **解析**改为复用同一路径（**判定语义不变**）；
  - 新类型 `LoggedAssembly`／`LoggedAssemblyFragment`／`LoggedSegmentUsage`／`LoggedSegmentBudget`／`LoggedCrop`。
- `crates/nai-runtime/src/lib.rs`：`assembly::*` 导出**增补**（新记录类型），既有导出不变。
- `crates/nai-runtime/src/composition.rs`：**一行未改**——`record()` 与 `next_trace("assembly")` 直接复用
  （ADR 0027「一处定义」，不新增第二条写入路径与第二个 trace 生成器）。
- `crates/nai-kernel/src/evaluate/assembly.rs`：**判定规则一行未改** ⇒ 判定面与执行面**同轮可分别验证**。

## 六、测试与非空洞守卫

`crates/nai-runtime/tests/assembly.rs` **25** 项（W104 的 **12** 项在四态下同步改写 + 本轮新增 **13** 项）：

**§41.5 四项落链与读回（本轮新增 12）**

1. `a_legal_plan_lands_all_four_items_of_the_payload`：键集**恰 5 键**、片段项**恰 3 键**、
   用量**每段一条**且**照抄判定输入**（`100+i`／`400+i`，与正文长度**无关**）、`crops` 存在且为空、链校验通过；
2. `the_payload_carries_no_field_the_judge_did_not_audit`：11 个未审字段名**逐个缺席**（`age_ms`／`trust`／
   `scope`／`source`／`missing_segments`／`over_budget`／`budget_missing`／…）；
3. `a_cropped_segment_lands_its_dropped_items_and_its_reason`：**同时**含 `dropped` 与 `reason`，
   且被裁段**照旧**报用量；
4. `an_assembly_missing_an_uncroppable_segment_is_refused_and_appends_nothing`：§41.1 的拒绝名 +
   `missing_segments == ["persona"]` + **链零新增**；
5. `an_assembly_refused_by_both_judgements_names_both_kinds_of_reason`：`Neither` 且**两族**拒绝名都在；
6. `the_three_refusals_are_pairwise_distinguishable`：三种拒绝**两两** `assert_ne!` 判别式；
7. `inputs_that_name_different_fragments_are_a_caller_error`：**集合不同**与**顺序不同**各报
   `assembly.input_ids_differ`；
8. `the_logged_items_round_trip_row_by_row`：读回后四项**逐字**相等；
9. `every_row_comes_back_with_its_own_usage_and_crops`：两次装配两行、**互不合并**；
10. `a_row_logged_before_the_two_items_existed_is_still_readable`：手工落**旧形状行**（三键）⇒ 读回成功且两项为空；
11. `an_unparsable_assembly_row_is_a_named_failure`：非数组 `fragments` ⇒ **失败**、不是零行；
12. `reading_the_log_appends_nothing`：读回前后**行集与内容**（`seq` + `payload_json`）逐项不变。

**非空洞守卫（本轮新增 1 + W104 保留 1）**

13. `every_assembly_rejection_is_reachable_through_the_runtime`：遍历 `AssemblyRejection::ALL`（**14** 条），
    逐条构造运行期 plan 并断言拒绝名出现在返回里；
14. `every_rebuild_rejection_is_reachable_through_the_runtime`（W104）：遍历 `RebuildRejection::ALL`（**6** 条）。

⚠️ 守卫的机械性来自**穷尽 `match`**：样本构造函数 `match rejection { … }` 覆盖全部变体 ⇒ kernel 往枚举
加一个变体而 runtime 未处理，**编译即失败**；反向遍历 `ALL` 使「加了变体却漏样本」不可能静默通过。
（**手写清单的盲区恰是清单的补集**，W95 教训。）

## 七、非空洞注入验证

**真改 `crates/nai-runtime/src/assembly.rs` + 真跑 + 备份拷回 + `sha256` 逐字节还原**
（基准＝**过完 `cargo fmt --all` 之后**的那一份：`ad0adc451d02df0c7c0c1afa183c5b8f2f7d546af95e47ab6b9ee6fcf8fb421b`；
还原后 `cmp` 逐字节相同、复跑基线 **25/25** 全绿）。

| # | 注入 | 实测红数 | 红条用例 | 拦的是什么 |
|---|---|---|---|---|
| I1 | 只跑 H02、不跑 H01 | **4** | 缺段拒绝 / 两侧理由 / 四态可分 / H01 非空洞守卫 | 「H01 判定真的在跑」 |
| I2 | 载荷仍只落三键 | **4** | 键集 / 裁剪落账 / 逐行往返 / 逐行不池化 | §41.5 第 2、3 项真的落盘 |
| I3 | H01 拒绝时提前 `return`（短路 H02） | **2** | 两侧理由 / 四态可分 | 「不短路」是被测试钉住的 |
| I4 | 把 `Neither` 折进 `NotCallable` | **2** | 两侧理由 / 四态可分 | 四态不是两态 |
| I5 | `segment_usage` 由正文重算 | **3** | 键集 / 裁剪段用量 / 逐行往返 | 「照抄判定输入」承重 |
| I6 | 入口一致性前置删掉 | **1** | 调用方错误 | 两份输入必须描述同一次装配 |
| I7 | 逐行读回改成池化 | **1** | 逐行不池化 | 读回是**每行一条** |
| I8 | 历史行（缺两键）被当成解析失败 | **1** | 历史行兼容 | 旧行可读、不被当成损坏 |

**共 18 条红**；每一组都**至少 1 红** ⇒ 八组注入**没有一组是空洞的**。注入临时目录 `.w106-inject`
（含 `assembly.rs.baseline` 与 `run.sh`）**已删除**，`git status` 无该目录。

### 7.1 本轮的方法论产出

- **I1 是「判定有没有真跑」的唯一直接证据**：若该组全绿，说明**没有任何用例**依赖 H01 的接线——
  那整轮改动就是**装饰**。
- **I3／I4 证明「不短路」与「四态」不是文风要求**：两态实现、或允许 H01 拒绝时提前 return，
  这一整类错误会**静默通过**其余 23 项。
- **I5 证明「照抄 vs 重算」可被区分**：夹具把用量按**段索引**取值（`100+i`），与正文长度**故意无关**
  ⇒ 重算实现必然被抓。
- **I6（1 红）与 I7（1 红）说明「少一条断言就少一个信号」**：这两条各自只被一个用例钉住，
  但**恰是**那一个用例不可省。
- 各注入的样本定位**按被断言对象的属性**构造（W92 教训），**不按「态」定位**。

## 八、门控与 validate

### 8.1 全量门控 `cargo xtask check --profile full`

**末次 EXIT = 0**。计数**现测**（非推断）：

- **Rust = 94 个目标 / 905 passed / 0 failed**（= **76** `Running` + **18** `Doc-tests`）。
  ⚠️ `Running` 原始 **77** 须减 **1**：门控会先打印一行**伪目标**
  ``Running `E:/nai-build-target\debug\nai-xtask.exe check --profile full` ``（**带反引号**、不带 `(…exe)` 括号，
  且**没有**对应的 `test result:` 行）⇒ `77 - 1 = 76`。**自洽校验**：`test result:` 行数 **94** == `76 + 18`。
- **与 W105（94/892）的差逐项对得上**：**目标数不变**（本轮**未新增测试文件**，只在既有
  `crates/nai-runtime/tests/assembly.rs` 内增用例）；`passed` **+13** 恰为该文件 **12 → 25** 的 **13** 条新增
  ⇒ **不是「口径变了」**。
- **Node：TAP 根 13 / 317 用例 / 0 fail**（`# pass 317`、`# fail 0`）。本轮**未新增/删除任何 Node 套件或用例**
  ⇒ 与 W105 逐项相同。
- `error[` / `LNK1104` / `out of memory` 命中 **0** 次。
- 门控步骤实测：`cargo fmt --all --check` → `cargo clippy --workspace --all-targets --locked -- -D warnings`
  （`Finished`、无 warning）→ `cargo test --workspace --locked --exclude nai-xtask`
  → `cargo test --locked --package nai-xtask --lib` → `node scripts/frontend-gate.mjs`。

### 8.2 ⚠️ 门控共跑 **3** 次：前两次红在**宿主瞬态**（**非代码缺陷**，且**已被登记过**）

| 次序 | 结果 | 失败点 |
|---|---|---|
| 第 1 次 | EXIT=1 | `nai-runtime --test composition`：`telemetry_is_buffered_when_persistence_fails_while_critical_is_reported` ⇒ `StorageError{DeadlineExceeded, "operation stopped"}` |
| 第 2 次 | EXIT=1 | **同一目标、不同用例**：`invalid_runtime_config_is_rejected_at_provision` ⇒ **同一类**错误 |
| 第 3 次 | **EXIT=0** | 加 `RUST_TEST_THREADS=1`（**限并发**）⇒ 全量绿 |

**根因（实测）**：`crates/nai-runtime/tests/common/mod.rs` 的 `store_operation()` =
`Operation::new(Duration::from_secs(5))`——**一个 5s 期限的 `Operation` 被整条用例复用**；该文件 12 条用例
默认**并行**（= CPU 数）运行、且紧跟在 `tests/assembly.rs`（本轮 **12 → 25** 条、门控内耗时二十余秒）之后
⇒ 单条用例的**累计墙钟**可越过 5s。

**判据（三步，全部实测）**：
① **逐目标能过**：`cargo test -p nai-runtime --test composition` **3/3 全绿**（6.77s / 7.75s / 7.69s）；
② **整包能过**：`cargo test -p nai-runtime --locked` **3/3 全绿**（56 passed / 0 failed）；
③ **该文件本轮一行未改**（改动面只有 `src/assembly.rs`／`src/lib.rs`／`tests/assembly.rs`）⇒ **非代码缺陷**。
**处置**：**限并发**、**绝不改代码**——与 `AGENTS.md` §0 的纪律一致。

⚠️ **这条瞬态在本仓已被登记过**：`KNOWLEDGE.md` §10 的 **W84** 条目原文即「`nai-runtime --test composition`
报 `DeadlineExceeded`（宿主瞬态，单跑即过）⇒ 按『单目标能过 ⇒ 非代码缺陷』处置，**不改代码**」
⇒ 本轮是**同一坑的再次现身**；且这次**换了一个用例**，说明它**随负载而变、不绑定某个用例**。

⚠️ **如实披露**：本机绿所用的 `RUST_TEST_THREADS=1` 是**限并发**、**不是**代码修正；CI 不带此旋钮
⇒ CI 侧仍可能遇上同一瞬态（W84/W90 的 CI 记录里已出现过 **windows-only、与提交内容无关**的失败）。

### 8.3 OpenSpec validate（归档前）

`openspec validate 2026-10-14-w106-assembly-judgement-wiring --strict --no-interactive` ⇒
`Change '…' is valid`（EXIT=0）。

### 8.4 规格差量的正文长度（**动笔时就实测**）

本轮**不改** `agent-harness`（其 H01 需求正文已 **472** 字符、门限 500、余量太薄）。改的是
`runtime-composition`：**4 条 ADDED**（332 / 277 / 167 / 266 字符）+ **1 条 MODIFIED**（**385** 字符）
—— 5 条 delta 全部 **< 500**、且 ≤ `MAX_DELTAS_PER_CHANGE`（10）⇒ 不重演 W104 那种「归档当场 1 红」。

### 8.5 归档与四条 validate（**归档后实测**）

```
$ openspec archive 2026-10-14-w106-assembly-judgement-wiring --yes
  ⚠ Why section should not exceed 1000 characters     ← 非阻塞告警（W104／W105 同）
Task status: ✓ Complete                                ← tasks 归档前已全勾
Specs to update:
  runtime-composition: update
    + 4 added
    ~ 1 modified
Totals: + 4, ~ 1, - 0, → 0
```

**档案路径**：`openspec/changes/archive/2026-10-14-w106-assembly-judgement-wiring/`
（`openspec/changes/` 下**只剩 `archive/`**）。

⚠️ **`archive` 不接受 `--no-interactive`**（本轮实测：`error: unknown option '--no-interactive'`）
——本仓既有记录即「`archive` 只接受 `--yes`」，本轮再次确认。

**归档后**四条 validate（**两条是 CI 独有的 `not-in-profile` 面，本机门控不跑 ⇒ 必须显式补跑**）：

| # | 命令 | 结果 |
|---|---|---|
| ① | `openspec validate --specs --strict` | **43 passed, 0 failed**（EXIT=0） |
| ② | `openspec validate --archived` | **104 passed, 0 failed**（EXIT=0；W105 为 103 ⇒ +1 为本次归档） |
| ③ | `openspec validate --all --strict`（= `pnpm spec:validate`） | **43 passed, 0 failed**（EXIT=0） |
| ④ | `openspec validate --changes` | `No items found to validate.`（EXIT=0，符合预期：无活跃变更） |

⇒ **四条一次全绿**，没有 W104 那种「归档当场 1 红」（§8.4 的前置实测生效）。

### 8.6 归档后**复跑门控**（归档改动了 `openspec/specs/**`）

按 W104 教训「改 `openspec/specs/**` 之后 MUST 复跑门控」（即便门控不读 spec ⇒ 让「EXIT=0」对应**真正的
提交树**）：复跑 `check --profile full` **attempt 1 EXIT = 0**，计数与 §8.1 **逐项一致**——
`Running` 原始 **77**（真目标 **76** + 伪目标 **1**）、`Doc-tests` **18**、`test result:` **94**、
`passed` **905** / `failed` **0**、TAP 根 **13** / `# pass` **317** / `# fail` **0**、
`error[`／`LNK1104`／OOM **0** ⇒ **归档与被改的那份 spec 都没挪动计数**。

⇒ **本轮门控共跑 4 次**（①首轮红、②重试红（均为宿主瞬态）→ ③限并发绿；④**归档后复跑绿**）。

## 九、CI（**回填**）

（本节由回填提交填写；回填提交**自身不回填**。）

- 提交：`cb410ef` `feat(w106)：把 §41 装配判定接上 runtime 装配入口（H01 runtime 接线）`（**15 文件**）；
  已 push（`119d5ec..cb410ef  main -> main`，SSH 443）。
- **run [#145](https://github.com/F31/nai/actions/runs/38566320199)（id `38566320199`，workflow
  `Engineering foundation`，event `push`）`completed / success`，`run_attempt 1`**；
  `head_sha cb410ef10d8fac1a4acfeee1f77b58be892e1692` == **推送 SHA**、`head_commit.message` 首行
  == **本提交标题**（**三件齐**：唯一查询参数 `&cb=<ns>` + 请求头 `Cache-Control: no-cache` + 回读 `head_sha`）。
- ⚠️ **run 序号现测 = `145`**（W105 为 `#142`）⇒ **不是 +1**，期间另有 run（re-run 或其它推送）
  ⇒ 又一次印证「**计数 MUST 现测、MUST NOT 靠上一轮 +1 推断**」（若按 +1 写 `#143` 就是**不实陈述**）。
- **矩阵双平台 success**：`foundation (ubuntu-24.04)` **success**（`12:45:57Z → 12:50:34Z`，≈**4.6 min**）
  + `foundation (windows-latest)` **success**（`12:45:57Z → 12:54:55Z`，≈**9.0 min**）。
- **观测 3 次、非轮询**：①发布时 run 列表（`in_progress`，`created 12:45:53Z`）→ ②本次 run 列表
  （`completed/success`，`updated 12:54:55Z`）→ ③本次 jobs 矩阵。
  ⚠️ **宿主瞬态（非 CI 侧问题）**：②的首次尝试所用 `curl` **漏了 `--max-time`** ⇒ 无界等待、挂起约 30 min；
  复跑加 `--max-time 45` 立即返回。**这是本机观测工具的缺陷，不是被观测系统的现象**，故不计入 CI 结论。
- ⚠️ **CI 真的跑了三条「本机门控不跑」的检查，且双平台都是 success**（**直接读 CI 步骤集**，非从
  workflow 文件推断）：

| CI 步骤 | 名称 | ubuntu | windows | 面 |
|---|---|---|---|---|
| 9 | `Check generated contracts without modifying consumers` | **success** | **success** | `schema --check` 面 |
| 21 | `Run pnpm spec:validate` | **success** | **success** | `not-in-profile`（§8.6 ③） |
| 22 | `Run pnpm exec openspec validate --archived --no-interactive` | **success** | **success** | `not-in-profile`（§8.6 ②） |
| 20 | `Full three-language contract suite (Linux C++ toolchain)` | success | **skipped** | 平台专属（正确） |
| 8 | `Install Linux C++ protobuf runtime` | success | **skipped** | 平台专属（正确） |
| 11 | `Publish the gate failure summary as annotations` | **skipped** | **skipped** | 正确：只在失败时发布 |

  ⇒ 本机 `--specs` / `--archived` / `--all` 的绿与 CI 的绿是**同向的独立复核**，而不是「本机绿代替 CI 绿」。
- ⚠️ **§8.2 的宿主瞬态在 CI 侧得到回答**：CI **不带** `RUST_TEST_THREADS=1` 这个限并发旋钮，
  而 `nai-runtime` 的集成测试（含本轮 **12 → 25** 的 `tests/assembly.rs`）在**双平台**均 **success**
  ⇒ 更强的证据指向「本机那次 `DeadlineExceeded` 确实是**本机负载下的瞬态**，**不是代码缺陷**」。
- 回填提交（`docs(w106)：回填报告 CI 结论`）**自身不回填**。

## 十、遗留与边界

- **接线核实**：Rust 集成测试由 `cargo test --workspace` 自动覆盖 ⇒ **无需**新增登记
  （不写第二处清单）；本轮**未新增独立 Node 测试文件** ⇒ **无需** `engineering_checks` 登记。
- **诚实边界**：**H01 保持 `pending`**。本轮可测的是「§41.1 的拒绝**在 runtime 上有表达**」与
  「§41.5 的**四项**真的落盘、且能逐行读回」；
  - **把九段片段真正喂给模型**这一半**未接线**（属 **H16**／仓库外）；
  - **§41.2 分段预算初值标定**属 §47.2 黄金集回放，§48.7 禁止采外部观察值 ⇒ 未做；
  - **§47.2 六类回归的执行体**是能力包 **H16**，本轮一行未碰。
  ⇒ 报告 MUST NOT 出现「装配器已接入运行期」「已不依赖当时内存」这类运行期断言。
  **如实披露不是缺陷，不实声明才是。**
- **既有弱化（具名登记、本轮不修）**：`rebuild_assembly_from_log` 的片段池**跨行合并** ⇒ 比 §41.5 的
  「日志能否重建**这一次**装配」**弱**（反例见 §三决策 7）。修它＝改 **H02 read 侧**的语义，属**另一个对象**，
  混进本轮会把证据面搞糊。
- 下一轮候选（A 组「可自主推进」剩余）：**B07 讲品循环运行时**（中）、**O02 审批面收尾**（中）、
  **A12 桌面壳主体**（大）、**H03+H04 记忆读写子系统**（大）；**H16（能力包）是 H01／H18 的共同外部前置**
  ——**H16 本身仍是「仓库外」**。

## 勘误（W127，2026-10-16）

本报告 §三决策 7 与 §十「既有弱化（具名登记、本轮不修）」所登记的 `rebuild_assembly_from_log`
**片段池跨行合并**，**已由 W127 闭合**。本文档按当时状态保留，MUST NOT 就地改写。

- **更正口径来源**：报告写「比 §41.5 的『日志能否重建**这一次**装配』弱」。§41.5 原文（V3.11:1971）
  **未**明文要求「单个日志条目」⇒ 收紧的登记依据是 **`tests/registry.yaml` 的 H02 标题**
  「由决策日志的片段引用重建**同一**上下文」。MUST NOT 把依据写成「§41.5 要求单条目」。
- W127 形状：逐行作为独立**条目**判定；承载判据 = 片段标识**多重集**（含重复、不计顺序）；
  **存在性**判定；无条目承载 ⇒ `rebuild.not_in_one_entry`，且 `unlogged_ids` 对整条链重新派生。
- 本报告末尾「下一轮候选」里的 **H02 read 侧语义**已不再是候选（W127 已完成）；该列表其余各项
  （B07／O02／A12／H03+H04／H16）**未变**。
- 见 [ADR 0105](../adr/0105-rebuild-in-one-entry.md)、[W127 报告](2026-10-16-w127-rebuild-in-one-entry.md)。
