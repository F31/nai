# W127 报告：从决策日志重建装配以**条目**为单位（H02 口径收紧）

- 轮次：W127｜日期：2026-10-16｜归属：`tests/registry.yaml` **H02**（装配可重建）
- 关联：[ADR 0105](../adr/0105-rebuild-in-one-entry.md)、ADR 0082／0084、
  `openspec/changes/archive/2026-10-16-w127-rebuild-in-one-entry/`、
  基线 `docs/直播数字人 + 智能体系统技术方案 V3.11.md` §41.5（第 **1971** 行，`grep -n` 现测）

## 一、缺口（现测）

**验收口径在 registry，不在 §41.5 的散文。** `tests/registry.yaml` 的 H02 标题逐字是
「装配可重建：由决策日志的片段引用重建**同一**上下文，不依赖当时内存」；
而 §41.5 原文（V3.11:1971）只说「装配 MUST 落决策日志：进入的片段标识、各段预算占用、裁剪项与原因、
本次调用的模型与提示词版本。装配记录 MUST NOT 含明文凭据（INV-11）。验收 H01、H02。」
——**未**明文要求「单个日志条目」。故收紧的依据链是 **H02 标题里的「同一」**，
本报告与 ADR 0105 MUST NOT 把结论写成「§41.5 要求单条目」。

**缺口形状（代码位置现测）**：`crates/nai-runtime/src/assembly.rs` 的 `rebuild_assembly_from_log`
把读回的多行用 `flat_map(|row| row.fragments)` **合并成一个片段池**再交给 `rebuild_assembly`。
该弱化在 W104 的注释、W106 的报告、`docs/runtime.md`、`AGENTS.md`、`docs/development-plan.md`
与 ADR 0084 决策 6/8 里**逐处具名登记**（ADR 0084 明确「改它属 W104 既有行为的变更，需要独立一轮」）。

**立项判据首跑实测（红）**：链上两个条目分别只承载 `frag-a` 与 `frag-b`（两次合法装配），
记录声明 `[frag-a, frag-b]`。旧实现返回：

```
thread 'an_assembly_split_across_two_entries_is_not_rebuildable_from_the_log' panicked:
no single entry assembled this context: []
```

⇒ `rebuildable = true` 且 `rejections` 为**空**。没有任何条目曾装配过这个上下文，
而「这两个标识没落日志」是**假话**——它们都在链上。

**为什么这不算「合法用例的假阴性」**：写路径的 `ordered_fragments` 是**按 `record.fragment_ids`
逐项**在素材里找并写出（`crates/nai-runtime/src/assembly.rs:388`）⇒ **任何真的被记录过的记录，
其自己那一行必然承载它**（标识多重集相同）。于是逐条目判定与池化判定对**真实记录**结论相同，
收紧消除的**只有假阳性**。这一条是「不为收紧付出假阴性代价」的正面证据，不只是安慰。

## 二、判据（真实证据）

| 位置 | 现测计数 | 新增/改造 |
| --- | --- | --- |
| `crates/nai-kernel/tests/assembly.rs` | **36 → 40** | 新增 4 条（多重集/顺序无关、分裂被具名拒且不得报未落日志、存在一个承载条目即足够、空链两名并存）；`ALL` 总账用例补新变体的构造 |
| `crates/nai-runtime/tests/assembly.rs` | **25 → 27** | 新增 2 条（跨两条目分裂、承载条目是超集）；非空洞守卫改造（见 §三） |

新判据各钉住什么：

- `an_entry_carries_the_assembly_by_multiset_and_ignores_order`：承载判据**不计顺序**（否则
  `rebuild.order_differs` 会失去唯一覆盖）；**多重集**而非集合——用**等长不同重数**
  （记录 `[f1, f1]` vs 条目 `[f1, f2]`）才是真正考到一对一配对的那一条（见 §三 I7）。
- `a_split_across_two_entries_is_refused_by_name_and_nothing_is_called_unlogged`：分裂时具名
  `rebuild.not_in_one_entry`，且 `unlogged_ids` **为空**（不得把链上存在的标识报成未落日志）。
- `one_carrying_entry_that_reproduces_the_record_is_enough`：判定是**存在性**的——
  一个承载条目能重建即可；且只有错误顺序的条目时，报的仍是**具体的** `order_differs`
  而**不是**被吞成更笼统的 `not_in_one_entry`。
- `an_empty_chain_names_every_id_unlogged_and_not_in_one_entry`：空链上两条原因**并存**
  （`fragment_not_logged` + `not_in_one_entry`），互不短路。
- runtime `the_chain_alone_reproduces_the_logged_assembly` / `the_logs_own_order_decides_and_a_difference_is_named` /
  `the_log_preserves_the_entry_order_rather_than_re_sorting_it` / `rebuilding_from_an_empty_log_reports_every_id_unlogged`
  **全部保持绿**：它们是收紧**不得**破坏的既有行为（尤其「拒绝时 `rebuilt` 仍等于日志自己的顺序」，
  它决定了失败分支必须取**第一个承载条目**的读数而不是清空，见 ADR 0105 决策 4）。

## 三、注入验证（每条守卫都要证明它会响）

编辑工具注入 + 真跑 + `cp` 逐字节还原 + `sha256` 核对。两个源文件基线：

```
crates/nai-kernel/src/evaluate/assembly.rs  5f19f5eb5e09f086425d568bf910cddfb2aeff2c477acca95f2534cf0cc751dd
crates/nai-runtime/src/assembly.rs          b72eae03cb5db916da59c52b42218f3591ff842c5a3289874ab6b67e5c3b50ad
```

七组注入，各自变红且可归因：

| # | 注入 | 变红用例 | 条数 |
| --- | --- | --- | --- |
| I1 | `entry_carries_assembly` 去掉长度闸门（超集即视为承载） | `an_entry_carries_the_assembly_by_multiset_and_ignores_order` | 1 |
| I2 | 承载判据改用**确切顺序**（`zip().all()`） | 同上 + `one_carrying_entry_that_reproduces_the_record_is_enough` | 2 |
| I3 | 失败分支把 `rebuilt` 清空 | runtime `the_logs_own_order_decides_and_a_difference_is_named` | 1 |
| I4 | 无承载分支的 `unlogged_ids` 对**空池**派生 | `a_split_across_two_entries_is_refused_by_name_and_nothing_is_called_unlogged` | 1 |
| I5 | 去掉 `refuse(NotInOneEntry)` | 分裂用例 + 空链用例 + **`ALL` 闭世界总账**（左值恰缺 `NotInOneEntry`） | 3 |
| I6 | 存在性退化为「只看第一条承载条目」 | `one_carrying_entry_that_reproduces_the_record_is_enough` | 1 |
| I7 | 匹配改为**可复用**（多重集退化为集合） | `an_entry_carries_the_assembly_by_multiset_and_ignores_order` | 1 |

### ⚠️ I7 首跑**假绿**（本轮最重要的判据纪律收获）

I7 第一次注入后测试**全绿**（40 passed / 0 failed）——**没有一条判据在响**。原因不是注入写错，而是
**判据本身不具判别力**：最初那条多重集断言是 `!entry_carries_assembly(&record(&["f1","f1"]), &[f1])`，
而记录长 2、条目长 1 ⇒ **长度闸门先返回 `false`**，一对一配对逻辑根本没被走到。
改成**等长不同重数**（记录 `[f1, f1]` vs 条目 `[f1, f2]`）后，同一注入**立刻变红**。

⇒ 「守卫都要注入验证」不是形式：**不注入就不知道哪条断言真的在拦。**
这条与 W93 的「探针值 MUST 先确认它不存在」同族，但**方向不同**——那次是注入自身坏了，
这次是**判据自身退化**，且**首次运行看不出来**。已在 registry H01 与 ADR 0105 如实登记。

### 一处测试失败的性质判定（`DeadlineExceeded`）

全量跑 `cargo test -p nai-runtime --test assembly` 时，`every_row_comes_back_with_its_own_usage_and_crops`
在 `crates/nai-runtime/tests/common/mod.rs:118`（`open_store`，用 `bringup()`）报
`StorageError{code: DeadlineExceeded, reason: "operation stopped"}`。判**非代码缺陷**：

- 错误类与位置都在**夹具/bring-up**，且该文件上方注释**已具名记载**同一现象
  （默认并行测试下多个数据根同时建库、迁移输掉磁盘竞争 ⇒ 夹具里随机 `DeadlineExceeded`）；
- 失败集合**不绑定**本轮改动的领域（该用例只读 `read_logged_assemblies`，与 `rebuild_assembly_from_log` 无交集）；
- 同一份代码在**限测试线程**（`--test-threads=2`）下 **27 passed / 0 failed**。

⇒ 处置 = **限并发，不改代码**；报告如实登记，不把它算作本轮的红。

## 四、registry 判定（**H01／H02 均维持 `pending`**）

- `H01`：新增段落记「该弱化已闭合」，**不改 `status`**。H01 的剩余缺口**未变**——
  §41.2 分段预算初值须由 §47.2 黄金集回放实测（§48.7 禁采外部观察值），喂模型属 **H16**。
- `H02`：新增段落记口径来源（**本条目标题**里的「重建**同一**上下文」）、立项判据的实测红、
  以及「合法用例零假阴性」的论证；**不改 `status`**——本轮闭合的是**判定面口径**，
  「本系统里真实装配一次上下文并喂给模型」**仍不是事实**。
- ⚠️ 报告 MUST NOT 出现「装配器已接入运行期」「已不依赖当时内存」这类运行期断言。
