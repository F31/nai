## Why

V3.11 §41.5 把装配的可判定性钉死为**四项**落账内容：「进入的片段标识、**各段预算占用**、
**裁剪项与原因**、本次调用的模型与提示词版本」。

W69 交付了装配判定（`crates/nai-kernel/src/evaluate/assembly.rs` 的 `evaluate_assembly`），并在该模块的
**模块文档**里写死了边界：

> Feeding the model, appending the record to the hash chain in `nai-storage`, and executing the six
> §47.2 regression classes (capability pack H16) are runtime work and are **not** claimed here.

W104 把其中**一半**补上了：`record_assembly` 把一次装配落进链。但它落的是**三项**，不是四项。

实测这条缝（可复现）：

```
$ grep -rn '\bevaluate_assembly\b' --include=*.rs crates apps | grep -v /tests/ | grep -v src/evaluate/ | wc -l
0
```

`evaluate_assembly` 在**生产代码里零调用方**。于是在 runtime 上有两条今天无法判的命题：

1. §41.5 的「各段预算占用」「裁剪项与原因」**没有任何承载物**。W104 的载荷只有
   `{fragments, model_version, prompt_version}` 三键（`crates/nai-runtime/src/assembly.rs:94-98`）。
2. §41.1 的「**不可裁剪段（1–3）缺失时 MUST 拒绝调用**」在 runtime 上**不可表达**：
   `record_assembly(&mut self, record: &AssemblyRecord<'_>, fragments: &[LoggedFragment<'_>])`
   根本不接收 `AssemblyPlan`，没有任何参数能携带九段、裁剪与预算。

⇒ 本轮是「**判定面存在、执行体不存在**」的又一次复现（W103/H15、W104/H02、W105/H18 同型）。

### 本轮不卡外部前置：与 W105 design §1.1 的辨析

W105 的 `design.md` §1.1 判过 **H01 卡外部前置**（「把九段片段**喂给模型**（真实装配器）——模型是 H16/外部」）。
那条判定只对被判的那一件事成立：上引边界原文把 runtime 侧用**逗号并列**了三件事——

> Feeding the model, **appending the record to the hash chain** in `nai-storage`, and executing the six
> §47.2 regression classes (capability pack H16)

即 **(i) 喂模型、(ii) 追加进链、(iii) 跑六类回归**。三者是并列项，不是同一件事的同位语。
H01 的判定面**不消费模型**：`evaluate_assembly(&AssemblyPlan) -> AssemblyVerdict` 的输入全部来自调用方
（片段、用量、预算），输出只有「可不可调用、为什么」。故 **(ii) 与 (i) 可分**，W105 那条判定
MUST NOT 被读作覆盖了 (ii)。本轮做的是 (ii)。

（这正是本项目记过的形状：「**一条待裁定项的处置可以被它自己的推荐文本否证** ⇒ 核实 MUST 读它自己的原文」。）

## What Changes

- **改** `crates/nai-runtime/src/assembly.rs` 的 `Composition::record_assembly`：
  - 签名加**第一**个参数 `plan: &AssemblyPlan<'_>`（H01 判定输入），与既有的 `record`（H02 输入）、
    `fragments` 并列——**判定 + 追加仍是一处**（与 W103/W104/W105 同形，理由同：分开两个出口会把
    不变量「不许把未放行的装配落链」的责任推给每个调用方）。
  - **两项判定都跑、都不短路**：`evaluate_assembly(plan)` 与 `rebuild_assembly(record, fragments)`
    各自独立调用，拒绝名**两类都报**（本仓既有纪律：拒绝类别不互相遮蔽）。
  - **`AssemblyOutcome` 扩为四态**（放行 + 三种拒绝）：`Logged(DecisionAck)` /
    `NotCallable(AssemblyRefusal)`（仅 §41.1–§41.3 拒绝）/ `NotRebuildable(AssemblyRefusal)`
    （仅 §41.5 重建拒绝）/ `Neither(AssemblyRefusal)`（两侧都拒绝）。
    **为什么必须分开**：把「不可调用」与「不可重建」折成同一个结局会说谎——前者是「不许拿它去调用模型」
    （§41.1），后者是「不许拿它当可重建的记录」（§41.5），后果不同；而「两侧都拒」若折进任一侧，
    另一侧的拒绝名就丢了。这与 W105 的三态是同一条教训：「**「记成什么」答不出 ⇒ 先问分类是否少一格**」。
  - 载荷**补齐 §41.5 的四项**：保留 `fragments`（进入顺序的 id／来源标识／字面正文）、`model_version`、
    `prompt_version`，新增 `segment_usage`（按段的 Token／字符／条数用量与该段自有配额）与
    `crops`（按段列出被裁项标识与原因原文）。取值**全部**来自判定审过的输入，runtime **不推算、不补零、
    不改写**；判定未审过的字段（片段的时效与数据/指令标注、判定产出的诊断清单）**不落账**。
  - **入口一致性前置**：`plan` 各段的片段标识序列与其记录声明的进入片段标识序列 MUST 相同，
    不同 ⇒ 具名拒绝（`assembly.input_ids_differ`）。**为什么必需**：否则同一条链记录会自相矛盾——
    「各段预算占用」说这一段进了某个片段，「进入的片段标识」说进的是另一个。
- **新增** `Composition::read_logged_assemblies(&self, limit: u32) -> Result<Vec<LoggedAssembly>>`：
  把链上的 `assembly` 条目**逐行**读回，每行携带该行落账的四项。只读，MUST NOT 追加／修改／重排。
  **为什么逐行而非池化**：把多行合并成一个池会让「各段预算占用」失去可归属的行。
  `rebuild_assembly_from_log` 的 payload **解析**改为复用本入口，其（池化的）判定语义**不变**。
- **改** `crates/nai-runtime/tests/assembly.rs`：既有 ~15 处调用点补 `plan` 夹具；新增
  「不可调用 ⇒ 具名拒绝且链零新增」「两侧都拒 ⇒ 两类拒绝名都在」「四态两两可分」
  「载荷键集恰为五项且逐键归属 §41.5 的一项」「往返：落链的四项逐行读回逐字相等」「入口一致性前置」
  「遍历 `AssemblyRejection::ALL` 十四条的**非空洞守卫**（穷尽 `match` 的样本构造器）」。
- **ADDED** `runtime-composition` 四条需求；**MODIFIED** 一条（W104 的载荷需求由「恰为三份材料」
  改为「恰为 §41.5 的四项」）。
- **不新增** `DecisionKind`、**不加迁移**、**不改任何契约**：复用既有的 `DecisionKind::Assembly`
  （该值在 Rust 枚举与 `0005-decision-log.sql` 的 `kind CHECK` 两个闭世界里都已存在），故
  `migration::VERSION` 与 `db_schema_version` **保持 7**——W104 的版本号连带面落在 CI-only 的
  `cargo xtask schema --check` 内（宿主机 `EBUSY`，本机跑不了），本轮**不产生**这样的面。
- **登记**：`tests/registry.yaml` 的 H01 追加 `notes`，说明 runtime 接线已落地而**验收仍 `pending`**
  （喂模型侧的装配器仍未接线，卡能力包 **H16**）。**不改任何验收 `status`**。

### Modified Capabilities

- `runtime-composition`：新增「不可调用的装配具名拒绝且不进链」「载荷含各段预算占用与裁剪项与原因」
  「两份输入必须描述同一组片段」「落链的装配行逐行读回」四条；把 W104 的载荷需求由「恰为三份材料」
  改为「恰为 §41.5 的四项」。
- **不改** `agent-harness`：其 H01 需求正文（472 字符）本就不含任何「落链」声明，无需澄清；
  且该正文受 `MAX_REQUIREMENT_TEXT_LENGTH = 500` 约束（余量 28 字符），追加一句澄清不可能容纳。

## Impact

- 代码：`crates/nai-runtime/src/{lib,assembly}.rs`、`crates/nai-runtime/tests/assembly.rs`；
  **零新增依赖**（`nai-kernel` 与 `nai-storage` 均已是 `nai-runtime` 的既有依赖）。
- **零迁移、零契约改动**：不改 `DecisionKind`、不改 `migrations/**`、不改 `migration::VERSION`（保持 **7**）、
  不改 `schemas/**`、不改任何生成物、不改 `db_schema_version`。这正是本轮满足「可自主推进」的关键。
- 存储读取面**零污染**（沿用 W104 实测）：`assembly` 条目的既有读者只有
  `Composition::rebuild_assembly_from_log` 一处（按 `kind == 'assembly'` 过滤）；
  没有任何守卫枚举 `DecisionKind`（`grep -rln 'DecisionKind' scripts/ tests/contracts/` 空结果）。
- **载荷形状变更的读回兼容**：本次保留 W104 的三键、**新增**两个键，故 W104 已落的历史行仍可被
  `read_logged_assemblies` 读回（其预算与裁剪两键缺席 ⇒ 读回为空列表，**不是**解析失败）。
  这一条 MUST 被如实登记在需求与测试里（见 design §7）。
- 门控：测试进既有 `cargo xtask check --profile full`（`portable` 覆盖 runtime），**不新增第二个门控入口**；
  不新增独立 Node 测试文件 ⇒ 无需 `engineering_checks` 登记。
- 文档：`docs/kernel.md`（W69 段的边界句指向新落点）、`docs/runtime.md`（新增 W106 段）、
  `docs/development-plan.md`、`AGENTS.md`、`docs/adr/0084-*.md`、报告、`tests/registry.yaml`。
- **明确不做**：不改 `evaluate_assembly`／`rebuild_assembly` 的任何判定规则；不新增种类／迁移／契约；
  不接线「把九段片段喂给模型」的装配器（卡能力包 **H16**）；**不修** `rebuild_assembly_from_log`
  的池化判定语义（本轮只统一其解析路径；该语义的缺陷如实披露并登记为遗留，见 design §9）；
  不改任何验收 `status`（H01 仍 `pending`）。

## 勘误（W127，2026-10-16）

上文「**不修** `rebuild_assembly_from_log` 的池化判定语义（本轮只统一其解析路径；……登记为遗留，
见 design §9）」所登记的遗留，**已由 W127 作为独立一轮闭合**（逐行作为独立**条目**判定、
具名 `rebuild.not_in_one_entry`）。本文档按当时状态保留，MUST NOT 就地改写。
见 [ADR 0105](../../../../docs/adr/0105-rebuild-in-one-entry.md)。
