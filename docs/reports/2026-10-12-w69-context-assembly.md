# W69：上下文装配（V3.9 §41.1–§41.5、§47.1、§47.4；验收 H01/H02）

- 日期：2026-10-12
- 变更：`openspec/changes/archive/2026-10-12-w69-context-assembly/`
- 规格：`openspec/specs/agent-harness/spec.md`（**MODIFIED** 既有两条需求，**非**新建能力、**非** `ADDED`）
- 代码：`crates/nai-kernel/src/evaluate/assembly.rs`、`crates/nai-kernel/tests/assembly.rs`
- 登记：`tests/registry.yaml` 的 **H01/H02** `notes` 更新（`status` 保持 `pending`；**本轮未新增条目**）

## 一、判断

本轮**没有**按上一次建议去清点 §55／§52——核实后先发现那两条的章号所指与我说的内容对不上
（§55 是「决策日志的责任再限定」、§52 是「平台准入独立成轨」），再按「取 R1 定义句 ∩ `registry.yaml` 清单」
的方法把 R1 定义句逐项对完，**没有第二处 W68 型缺口**。

真正的缺口在另一处：**H01/H02 至今没有判定函数**。而我此前（W67 收尾与工作记忆里）把
「`harness.rs` 里出现过 `Assembly`」当成了「已经有判定面」。

| 我此前的说法 | 核实结果 |
| --- | --- |
| 「R1 保留的七条（H01–H04、H15、H17、H18）**全部有判定面**」 | **不准确**。`harness.rs` 里 `Assembly` 出现十余次，**每处都是层名或字段名** |
| W56 交付了 H01/H02 吗 | **没有**。§47.6 的 W56 行目标写的正是「上下文装配器」，但 W56 只交付**边界与登记**（零实现） |
| H01/H02 需要新建能力规格吗 | **不需要**。`agent-harness` 里**已有**这两条需求（W56 建立）⇒ 本轮是 `MODIFIED` |
| 代码里有装配判定吗 | **没有**。`evaluate/` 下无任何函数判「装配顺序／分段预算／可重建」 |
| `tests/registry.yaml` 里有条目吗 | **有**（H01/H02 自 W56 起在册），但 `notes` 仍写「未开始」⇒ 与代码状态一致，**条目本身不缺** |

### 1.1 「名字出现过」≠「判定存在」（一类可复用的自查）

W68 补的是「**门槛定义里有它、验收清单里没有它**」（清单侧缺口，查法 = 定义句 ∩ 清单）。
本轮补的是另一类：**清单里有它、代码里没有判定函数**（实现侧缺口）。
这类缺口**特别容易被「符号出现过」掩盖**——`harness.rs` 里 `Assembly` 出现十余次，
`grep -c Assembly` 看起来「已经有了」。

统一直查法：**不看符号是否出现，只看「有没有一个函数在判它」**。
`grep -n 'fn ' <模块>` 里没有相应函数，就是没有判定面。

## 二、方法：先核实再裁定

| 核实项 | 实测结果 |
| --- | --- |
| H01/H02 的判据在哪 | **§41.1–§41.5**：九段固定顺序（含不可裁 1–3）、预算不足时**从后往前**裁剪、分段预算、片段来源标注、可判定性（H01/H02 的定义句在 §41.5） |
| §41.1 有几段、顺序是什么 | **9** 段：系统策略 → 人设 → 输出契约 → 事件摘要 → 证据 → 记忆 → 技能 → 最近播完句 → 工具结果 |
| 顺序存在的理由 | §41.1 原句：**约束不被挤掉** ⇒ 前三是不可裁、且裁剪倒着走 |
| §41.3 的来源类型有几类 | **7**：系统策略／场景／事件／知识证据／记忆／工具结果／外部智能体 |
| 「一律标为数据」点名了哪几类 | **4**：观众文本（→事件）／检索片段（→知识证据）／工具返回（→工具结果）／外部智能体返回（→外部智能体） |
| 预算记录几维 | §41.2「**Token 与字符同时记录**」⇒ 至少两维；§41.1 的裁剪列还涉及条数 ⇒ 三维 |
| 阈值能给具体数字吗 | **不能**。§41.2 的初值只是起点、须经 §47.2 黄金集回放调优并记入 ADR；§48.7 禁止采外部观察值 |
| `harness.rs` 的 `Assembly` 是什么 | **全是层名与字段名**：`ObservationLayer::Assembly`（观测五类之一）、`FailureLayer::Assembly`（六类之一）、必需字段 `segment_budget`／`cropped_items`／`crop_reason` |
| `agent-harness` 规格里有这两条吗 | **有**：第 14 行「Context is assembled in a fixed order under per-segment budgets」、第 31 行「Every assembled fragment is sourced, scoped and reconstructible」 |
| 与 `selling`／`show` 会重叠吗 | **不会**。`show` 判**单个 `ShowPlan` 自身**，`selling` 判 §53 的**四段讲品**（另一条轴），本模块判 §41 的 **L1 信息边界** |
| INV-11 的凭据标记表要新建吗 | **不要**。`harness::CREDENTIAL_MARKERS` 已有，建第二张会让两处判定分叉 |

## 三、交付

新增 `crates/nai-kernel/src/evaluate/assembly.rs`（纯逻辑、零新增依赖、零新增契约名、只依赖 contracts），
在 `evaluate/mod.rs` 注册。**两个半边、两个互不相通的扁平拒绝枚举**——合并会让「哪条规则拒了这次调用」重新不可判定：

### 3.1 H01 半边：`evaluate_assembly`

判定顺序 = §41.1 表格的阅读顺序：**段名 → 顺序 → 不可裁齐备 → 不可裁未被裁 → 裁剪记录 → 裁剪落点 → 预算 → 标注**。
**任何一步都不短路**——「又缺段又乱序」的计划两条都要报，因为问「为什么被拒」的人不是只想知道第一个原因。

### 3.2 顺序：判序列，不判集合

```rust
let sequence: Vec<Segment> = known.iter().map(|(segment, _)| *segment).collect();
let mut expected = sequence.clone();
expected.sort();
expected.dedup();
if sequence != expected {
    refuse(&mut rejections, AssemblyRejection::SegmentOutOfOrder);
}
```
排序后比较集合的实现会放行「规格在卖点之前」——注入已验证它会被 `the_right_segments_in_the_wrong_order_are_refused` 抓住。

### 3.3 裁剪：只判「有记录」不够，还要判「成后缀」

§41.2 说预算不足时**从后往前**裁剪 ⇒ 被裁的**可裁段**必须构成一个**后缀**。
「证据被裁、而更靠后的技能与播完句完好」这种形状**不可能**由倒序行走产生：

```rust
let mut seen_cropped = false;
for (segment, cropped) in &known {
    if segment.uncroppable() { continue; }
    if *cropped { seen_cropped = true; }
    else if seen_cropped { refuse(&mut rejections, AssemblyRejection::CropFromFront); }
}
```
另外，裁剪须**同时**记录「被裁了什么」与「原因」——缺任一半各自具名。

### 3.4 分段预算：三维互不替代，且报出**是哪一维**

`SegmentBudget { tokens, chars, items, floor }`：`tokens` 与 `chars` 是 §41.2 明文要求的两维，
`items` 来自 §41.1 裁剪列的条数，`floor` 给不可裁段。缺 `tokens` 或 `chars` 判 `BudgetDimensionMissing`；
超限报 `BudgetFinding { segment, dimension, used, limit }`，**指明维度**——只说「超预算」无法定位。
阈值**由调用方给**，模块**不内建任何数字**。

### 3.5 §41.3 的三处细微处

| 条款 | 原文措辞 | 本模块的处置 |
| --- | --- | --- |
| 作用域 | 与来源类型、来源标识、数据/指令标注并列 | **必填** ⇒ 缺失判 `FragmentWithoutScope` |
| 时效 | 「**如适用**」 | 缺失**不算违规、也不判**（既不放行也不禁止） |
| 数据/指令标注 | 「数据或指令」 | `Trust` **不是** `Option` ⇒ 「缺标注」在类型上不可表达，于是只有**标错**才需要拒绝 |

`SourceKind::must_be_data()` 恰为 §41.3 点名的四类；其余三类 §41.3 无此约束，本模块**不判**。

### 3.6 H02 半边：`rebuild_assembly`

只消费**决策日志**（`AssemblyRecord` + `LoggedFragment`），按**日志自身顺序**排出被引用片段再与记录比较：

```rust
for fragment in logged {          // ← 日志顺序，不是记录顺序
    let Some(body) = fragment.content else { /* FragmentContentNotInLog */ };
    ...
    rebuilt.push(RebuiltFragment { id, source_ref });
}
// 顺序比较**以“每条都被重建出来”为前置**，且该前置在代码里具名
if rebuilt.len() == record.fragment_ids.len() { /* 比较 rebuilt 与 record.fragment_ids */ }
```

三处刻意的设计：

1. **按日志顺序排**。若先按记录顺序排序再比较，「日志顺序与记录不同」这条断言**恒真**——
   注入 6 正是这个坏实现，被 `a_log_whose_order_differs_from_the_record_is_refused` 抓住。
2. **两个原因分开具名**。`FragmentNotLogged`（根本没记）与 `FragmentContentNotInLog`
   （记了，但正文没落盘、只存了指向当时内存的指针）是两件事——**H02 要禁的正是后者**。
3. **顺序比较的前置条件具名**。片段缺了就不比顺序，理由是缺片段时比较顺序会报出**第二个误导性原因**；
   这个前置写在代码注释里，而不是静默 `continue`。

明文凭据复用 `harness::CREDENTIAL_MARKERS`（INV-11，不建第二张标记表）。

### 3.7 具名拒绝

| 枚举 | 条数 | 内容 |
| --- | --- | --- |
| `AssemblyRejection` | **14** | `UnknownSegment`、`SegmentOutOfOrder`、`UncroppableSegmentMissing`、`UncroppableSegmentCropped`、`CropWithoutReason`、`CropWithoutDroppedItems`、`CropFromFront`、`SegmentBudgetMissing`、`BudgetDimensionMissing`、`UncroppableWithoutFloor`、`SegmentOverBudget`、`FragmentWithoutSourceRef`、`FragmentWithoutScope`、`UntrustedMarkedAsInstruction` |
| `RebuildRejection` | **6** | `FragmentNotLogged`、`FragmentContentNotInLog`、`RebuildOrderDiffers`、`ModelVersionMissing`、`PromptVersionMissing`、`PlaintextCredential` |

两者都是扁平的、`label()` 互异、带 `ALL`。**不复用** `harness::FailureLayer` 的降级计算——
「一次装配是否合法」与「装配失败该落到 §12.2 哪一级」是两件事，合并会让后者重新不可判定。

## 四、验证

| 项 | W68 | W69 | 增量 | 说明 |
| --- | --- | --- | --- | --- |
| `cargo xtask check --profile full` | EXIT=0 | **EXIT=0** | — | 第 **6** 次尝试通过（其中 2 次是本轮自己的代码缺陷，4 次是环境性 `LNK1104`） |
| 测试目标 | 90（72 + 18） | **91（73 `Running` + 18 `Doc-tests`）** | **+1** | 新增 `crates/nai-kernel/tests/assembly.rs` |
| 测试项 | 817 | **853 passed / 0 failed** | **+36** | 恰等于新增的 36 项测试 |
| `dependency boundaries` | 19 包 | **19 包** | 0 | 无新增依赖 |
| `admission registry check` | passed | **passed** | — | 只改两条 `notes`，三向断言仍成立 |
| 前端门控 | 22 项 | **22 项** | 0 | 未触碰前端 |

**两个增量互相对照**：目标 +1 与项 +36 恰好对应新增的那一个测试目标与其 36 个用例，
这是「未误改别处」的旁证。

**一处计数口径的自我纠正**：本轮首次统计时我的 grep 得到 74 个 `Running` 行，与 W68 的 72 相比像是 **+2**。
复核后发现其中一行是
`Running \`E:/nai-build-target\debug\nai-xtask.exe check --profile full\``
——**这是 cargo 的包装行、不是测试目标**（W65/W66 各踩过一次的同一个坑）。
排除后为 **73**，+1 成立。⇒ 「目标增量」与「项增量」必须互相对照，**否则这个坑不会自己暴露**。

**关于 6 次尝试**：前 2 次失败是**本轮代码自己的问题**——第 1 次 `cargo fmt --all --check` 报新文件格式不合，
第 2 次 `cargo clippy -- -D warnings` 报 3 处 `collapsible_if`（我写的 `if let … { if … }` 嵌套）。
两者都是**真实的、必须修的缺陷**，不是环境噪声；修完并单独验证 `fmt` 与 `clippy` 均干净后才继续。
第 3–5 次失败是**环境性的 `LNK1104`**（无法打开待写入的 `.exe`），且**每次失败的目标都不同**
（第 3 次 `nai_kernel-…`、第 4 次散布在 `duty`/`metrics`/`golden`/`accounting`/`evaluate`/`memory`/`nai-budget-core`
等**本轮未触碰**的 crate、第 5 次（`JOBS=1`）落在 `nai-engine` 的 `pipeline`）。
判据：**逐包能过**（`cargo test -p nai-kernel` 全绿，含 `tests/assembly.rs` 36 项）⇒ **不是代码缺陷**。
处置：先用 `--no-run` 循环把全部测试目标逐个链接就位（每轮推进一个，共 7 轮收敛），
再跑门控时链接步成为 no-op ⇒ 第 6 次 **EXIT=0**。

**OpenSpec 两条口径**（互不替代，都跑）：

| 口径 | W68 | W69 | 增量 |
| --- | --- | --- | --- |
| `validate --all --strict`（规格） | 43 | **43 passed / 0 failed** | **0**（本轮**不新增能力规格**，只改既有 `agent-harness` 的两条需求） |
| `validate --all --strict --archived`（已归档变更） | 66 | **67 passed / 0 failed** | **+1** |

**一处与 W68 相反的观察**：W68 因为新建能力 `basic-selling-loop` 而让规格口径 **+1**；
本轮走 `MODIFIED`，规格条目数**不变**，只有已归档变更数 **+1**。
⇒ 两个口径的增量本身就是「本轮是新增能力还是修改既有能力」的旁证。

**归档时的一条非阻断告警**：`openspec archive` 打印
`⚠ Why section should not exceed 1000 characters`（本变更 `proposal.md` 的 `Why` 段偏长）。
它**只是告警**——`validate --all --strict` 与 `--archived` 两条口径随后均 **0 failed**。
未修改归档后的 `proposal.md`：归档目录属**历史记录**，按仓库约定保留原文（如需订正应追加勘误段，
而本次并无事实性错误，仅篇幅偏长）⇒ 记在此处备查。

`tests/assembly.rs` **36 项**，含：

- 两条「**构造出的拒绝集合恰为 `ALL`**」总账（14 组 + 6 组，既证每条可构造，也防未登记的名字）；
- 四条**内容**守卫——九段顺序、七类来源、不可裁序号、四类「一律为数据」**逐项按名写死**
  （W66 的教训：只校验「N 条」对**内容被改小是瞎的**）；
- **跨模块**守卫：`AssemblyRecord::logged_field_names()` == `ObservationLayer::Assembly.required_fields()`
  （改一张不动另一张即失败；两张表分处两个模块是有理由的——§47.1 定日志形状、§41.5 定记录）；
- 正向不变量（合规装配零拒绝且 `callable` 为真）——否则「全都拒」也能让测试变绿；
- 顺序断言「又缺段又乱序时两条都报」。

### 4.1 七组注入，各自被抓住

每组都**从原始副本出发**（`--snapshot` / `--restore`），跑完即复原并核对内容哈希：

| 注入 | 改动 | 被抓住的测试 |
| --- | --- | --- |
| 1 `A_set_compare` | 顺序判定改成集合比较 | `the_right_segments_in_the_wrong_order_are_refused`、`a_plan_that_is_short_and_out_of_order_reports_both`、`every_rejection_…`（3 项） |
| 2 `B_only_first_missing` | 缺段循环加 `break` | `two_missing_uncroppable_segments_are_both_named`（1 项） |
| 3 `C_swallow_unknown` | 未定义段名不判 | `an_undefined_segment_name_is_refused`、`every_rejection_…`（2 项） |
| 4 `D_drop_suffix_check` | 删掉「裁剪须成后缀」检查 | `cropping_that_skips_a_front_croppable_segment_is_refused`、`every_rejection_…`（2 项） |
| 5 `E_merge_log_reasons` | 把 `FragmentContentNotInLog` 并成 `FragmentNotLogged` | `a_fragment_whose_body_was_not_logged_is_refused`、`every_rebuild_rejection_…`（2 项） |
| 6 `F_sort_by_record` | 重建时先按记录顺序排序 | `a_log_whose_order_differs_from_the_record_is_refused`、`every_rebuild_rejection_…`（2 项） |
| 7 `G_edit_field_table` | 把字段表里 `fragment_source` 改成 `source_ref` | `the_assembly_record_fields_match_the_harness_l1_table`（1 项） |

**注入 4 与注入 7 是设计上必须的**：注入 4 说明「只判有裁剪记录」的坏实现会被抓住，
而那正是「把 §41.2 读成『只需记录』」的常见误读；注入 7 说明 W66 补的那类**内容**守卫在这里同样必要——
删/改一个字段名不会有其它测试失败。

注入全部回滚，源码与原始快照**逐字节一致**（`diff` 无输出），前后两次基线均 **36 通过 / 0 失败**。

### 4.2 本轮踩到的一个新坑：`copyFileSync` 会连 mtime 一起复制

第一次跑注入验证时，脚本用 `fs.copyFileSync(PRISTINE, SRC)` 复原源码。
**Windows 的 `CopyFile` 默认连同时间戳一起复制** ⇒ 复原后的源码 mtime 比注入构建**更旧**，
cargo 判定「源未变更」而**复用上一次注入的二进制**。

后果：两次「基线」跑出的其实是**上一组注入**的结论——

| 现象 | 当时的误读 | 实际原因 |
| --- | --- | --- |
| 循环开头的基线报 `34 passed; 2 failed` | 「基线有真缺陷」 | 复用了 502 中断时残留的**注入 6** 二进制 |
| 单独重跑基线报 `the_assembly_record_fields_match_the_harness_l1_table` 失败 | 「源码与测试不一致，是真缺陷」 | 复用了**注入 7** 的二进制（`left` 里出现 `source_ref` 正是注入 7 的改动） |

**决定性证据**：把源码的 `logged_field_names()` 打出来是 `fragment_source`（原始值），
而失败输出的 `left` 却是 `source_ref`——**失败信息里的源码状态与磁盘上的源码不一致**。

处置：改用 `fs.writeFileSync(SRC, 内容)`（会刷新 mtime），并核对**内容哈希**。
此后前后两次基线都是 **36 通过 / 0 失败**，七组注入也各自被抓住且失败集合**互不相同**
（若二进制陈旧，失败集合会与上一组雷同）。**教训写进 §七.2。**

### 4.3 契约零漂移的取证方式

`cargo xtask schema --check` **在本机跑不了**（`generate.mjs` 内 `spawnSync(process.execPath, …)` 报
`EBUSY`，managed 与 system node 都复现 ⇒ 主机对「node 自旋 node」的限制）。**故不宣称该检查已通过**，
改用两条互补证据：

1. **结构性**：生成器输入只有 `schemas/catalog/capabilities.json` 与 `schemas/{config,plugin}/*.schema.json`。
   本轮改动为 `crates/nai-kernel/src/evaluate/{mod,assembly}.rs`、`crates/nai-kernel/tests/assembly.rs`、
   `tests/registry.yaml` 的两条 `notes`（**未新增条目、未新增契约名**）与文档 ⇒ 漂移不可能发生；
2. **CI 独立步骤**：`.github/workflows/foundation.yml` 的
   「Check generated contracts without modifying consumers」在 Linux 与 Windows 上各跑一次，push 后核对。

#### 实测结论（push 后低频单查询）

提交 `a9f9aea` 触发的 `foundation` 工作流 run **`38200252090`** 总体结论为 **failure**，
两个 job **结论相反**：

| job | 结论 | 失败步骤 |
| --- | --- | --- |
| foundation (ubuntu-24.04) | **failure** | 步骤 10「Complete engineering foundation（portable steps + frontend workspace）」 |
| foundation (windows-latest) | completed success | — |

**定位路径**（匿名可读）：`GET /repos/F31/nai/actions/runs/38200252090/jobs` 看到步级结论
→ `GET /repos/F31/nai/commits/a9f9aea/check-runs` 拿到 check-run id `114651849160`
→ `GET /repos/F31/nai/check-runs/114651849160/annotations` 读到失败内容。

ubuntu 上唯一失败的是 **`apps/engine/tests/engine.rs` 的
`killed_keeper_is_halted_by_the_engine_and_the_chain_survives`**
（`engine.rs:289`，`test result: FAILED. 5 passed; 1 failed`）：
`assert!(revoked, "a killed keeper must revoke output authority")` 超时——该测试 `kill -9` 掉 keeper 后，
用 **10 秒**窗口轮询引擎是否把 `generation_epoch` 由 `"0"` 置为 `"1"`（`engine.rs:273–289`）。

**为什么判定它**不是**本轮引入**（四条独立证据）：

1. `git diff --name-only a9f9aea~1 a9f9aea -- apps/` **无输出**——本轮 14 个改动文件全在
   `crates/nai-kernel/`、`openspec/`、`tests/registry.yaml` 与文档，**没有一行落在 `apps/`**；
2. **同一提交**在 `windows-latest` 上 **completed success**（含 `apps/engine` 的全部测试）；
3. 本机（Windows）`check --profile full` **EXIT=0**，该测试也在其中运行并通过；
4. 失败的是**时序断言**（10 秒窗口内等异步撤权），与「装配判定面」无因果关系。

⇒ 这是**既已登记的 ubuntu 间歇失败族**（`docs/reports/` 记录 W48 修过 halt/drill 竞态）的又一次出现，
不是本轮回归。**但本报告不宣称「CI 已绿」**——`a9f9aea` 的 CI **确实红了**，
本机门控绿不等于 CI 绿（这是本轮第二次被这条纪律救到）。

- **待办（本轮新登记，不在 W69 范围内）**：`killed_keeper_is_halted_by_the_engine_and_the_chain_survives`
  在 ubuntu runner 上可超时（10 秒撤权窗口）⇒ W48 的修复**未完全**消除该类竞态。
  下一次触及时应按 W48 铁律「**构造使顺序成立**」加固，而不是放宽断言。
  本轮**不改 `apps/engine`**（超出 H01/H02 范围）。

**观测纪律**：取结论用了 2 次低频查询（`run` + `jobs`，各带唯一 `cb` 与 `Cache-Control: no-cache`，
回读 `head_sha` = `a9f9aea` 与本次推送一致）；随后为诊断又用 2 次（`check-runs` + `annotations`），
全程 **4 次请求**（匿名配额 60/小时）。

**观测轮终止约定**（照 W57 先例）：本条「回填 CI 结论」的提交**其自身 run 不再写入本报告**，
否则无限递推；其 run 只做一次核对，结果记入当日工作日志。

## 五、验收侧：为什么 H01/H02 是 `pending` 而不是 `implemented`

| 判定面已交付 | 完整验收仍缺 |
| --- | --- |
| 「一次装配是否合法、违反了哪几条」可判定 | **预算初值本身**：§41.2 的初值须经 §47.2 回放调优并记入 ADR；§48.7 禁止采外部观察值 |
| 「顺序／不可裁段／裁剪落点／标注是否合规」可判定 | **真正把模型喂上**：`AssemblyPlan` 是计划，不是实际 prompt 载荷 |
| 「日志是否足以重建同一序列」可判定 | **记录真正落盘**：追加进 `nai-storage` 的只追加哈希链属 storage 侧 |
| 「哪些重建失败原因存在」可判定 | **§47.2 的六类回归执行体**：那是能力包 **H16**，本轮只判「是否已记录为通过」（INV-33） |

判定面交付 ≠ 验收达标。故本报告**不出现**「装配器已接入」「已不依赖当时内存」这类措辞。

## 六、未做与遗留

1. **基线偏差**：本轮**未新增 registry 条目**（H01/H02 早已在册）⇒ **不产生新的基线偏差**；
   W68 登记的 B07 偏差（§48.5「B | B01–B06」、§57.4 计数行「= 87」）**仍未收录**，仍留待下一轮升基线。
2. **运行时接线**：把预算初值回放调优成定值并写 ADR；把装配记录追加进哈希链；把 §47.2 六类回归接进 H16。
3. **未触碰**：A12（卡外部投入）、O01／O02／O05（需真实候选来源与生产接线）、
   B01／O03／O04（需 100 题集、真机、干净 Windows 安装等外部投入）、M 系列与 H05–H14／H16（能力包）。
4. **未改基线文本**、**未改任何既有 `status`**、未改 `docs/直播数字人 + 智能体系统技术方案 V3.9.md`、
   **未改 `apps/` 下任何文件**。
5. **CI 侧新登记一项间歇失败**：`apps/engine/tests/engine.rs` 的
   `killed_keeper_is_halted_by_the_engine_and_the_chain_survives` 在 ubuntu runner 上可超时
   （详见 §4.3 的实测结论）。**本轮只登记、不修**（超出 H01/H02 范围），
   列入下一轮候选；处置原则仍按 W48 铁律「构造使顺序成立」，**不放宽断言**。
6. **W67 的两项待确认仍在**：影子模式在 R1 无落点；「通过 §47.2 的回归」是全六类还是相关类。

## 七、方法论教训

1. **「名字出现过」≠「判定存在」**。我此前把 `harness.rs` 里出现十余次 `Assembly` 当成了 H01/H02 已有落点；
   实际每处都是**层名或字段名**。自查方法：**只看有没有一个函数在判它**（`grep -n 'fn '`），
   不看符号出现次数。这条与 W68 的「门槛定义里有它、验收清单里没有它」互补——
   W68 缺在**清单**侧，本轮缺在**实现**侧，两者都不能靠检索符号名发现。
2. **验证脚本自身也要被验证**（§4.2）。`fs.copyFileSync` 在 Windows 上连 mtime 一起复制，
   于是「复原」之后的 cargo 跑的是**旧二进制**，把上一组注入的结论当成基线的结论。
   从今往后每一步注入验证都要问：**这次跑的二进制，真的是我刚写下的那份源码编出来的吗？**
   证据是**内容哈希**，不是「我刚改过文件」的直觉。
3. **先想清楚坏实现的形状，再定数据结构**（§3.5）。`Trust` 不设 `Option`、
   `Checkpoint` 留 `carried_text`、`PitchPlan::segments` 用 `&[&str]`——都是为了让违规**可表达**。
4. **前置条件要具名**（§3.6）。缺片段时不比较顺序是对的，但「不比较」的理由要写在代码里，
   否则「跳过」与「通过」在日志里长得一样（W68 §3.4 的同一教训，本轮在另一个模块复现）。
5. **不要复用属于另一个维度的既有物**（§3.7）。不复用 `harness::FailureLayer`、
   不新建第二张凭据标记表、不并入 `selling` ——三次「不合并」都写进了模块头注释，
   这样下一个人（包括我自己）在扩展时能看见理由。
