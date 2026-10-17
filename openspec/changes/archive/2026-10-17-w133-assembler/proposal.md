# Proposal: W133 — H01/H02 的缺失生产者：把「装配」从测试里的手工构造变成一段真实路径

## Why

`tests/registry.yaml` 的 `H01`/`H02` 两条 notes 加起来近三千字，逐轮记着 W69 的判定面、W104/W106 的 runtime
接线、W127 的口径收紧。本轮开工前**现测**，这些记录是真的——但漏掉了一层：

```
grep -rn "AssemblyPlan" --include="*.rs" crates/ apps/ | grep -v tests
  nai-kernel/src/evaluate/assembly.rs:292   struct AssemblyPlan      ← 定义
  nai-runtime/src/assembly.rs:180           plan: &AssemblyPlan      ← 消费
```

**没有任何一行构造它**。`record_assembly` 的入参是一份计划，而计划在生产代码里**没有生产者**：判定面
（`evaluate_assembly`，40 项覆盖）能判，执行体（落链、读回、只靠链重建）能写能读，但**两端之间是空的**。
仓库里每一份 `AssemblyPlan` 都诞生在测试里、也死在测试里。所以「H01 已接线」这句话，扣掉修辞之后，
描述的是**一个没有调用方的函数体**——这正是 W107/W110 那两轮把记忆链称过的东西。

四环模型下的落点：①判定面（W69 已建，本轮**一行未改**）→ ②执行体（W104/W106 已建，本轮**一行未改**）
→ ③调用方（`record_assembly`，**本轮获得第一个真实入参**）→ ④真实产生者（把装配结果喂给模型的
调用点、§41.2 初值标定，**仍未接线**）。

## What Changes

1. **装配器**（`crates/nai-kernel/src/assemble.rs`，新建）：`plan_assembly` 把「按段分组的候选片段 +
   调用方给定的逐段预算」装配成一份计划。它只决定三件事：
   - **顺序**——把 §41.1 的固定序列**施加**在计划上，不回显调用方的输入顺序（顺序存在的理由是
     「约束不被挤掉」，回显恰好丢掉这一点）；
   - **在场**——调用方没给候选的段**不出现**，绝不补空切片。§41.1 让 1–3 段必需，而「必需却没出现」
     是**判定面**该拒的（`UncroppableSegmentMissing`）；补一个空切片会让那条拒绝在真实路径上**没有对象**；
   - **裁剪**——段内超预算从**尾部**裁，并同时记录被裁项与原因；1–3 段**任何预算都不裁**，
     由此产生的超预算段交给判定面拒。
2. **零业务常量**：没有第二份九段表、没有可裁性表、没有默认预算。九段与顺序来自 `SEGMENT_ORDER`，
   可裁性来自 `Segment::uncroppable`，每一个上限来自调用方的 `SegmentBudget`。§41.2 说初值只是起点、
   要用 §47.2 黄金集回放标定并记入 ADR——在这里发明一个数字，等于把 §41.2 答第二次，并把一个
   没有证据选过的数冻进仓库。
3. **只拒一件事**：§41.1 没定义的段名 ⇒ `AssembleError::UnknownSegment`，**不静默丢弃**。
   丢弃会让 `AssemblyRejection::UnknownSegment` 从这一侧**不可达**，并把「我不认识它」变成
   「没有东西需要认识」。
4. **接线证据**（`crates/nai-runtime/tests/assembly.rs`，+2）：候选 → `plan_assembly` → `record_assembly`
   → `read_logged_assemblies` → `rebuild_assembly_from_log` 一路走通；另一半是「装配器造出来但 §41.1
   仍拒的计划，**一个字节都不落链**」。
5. **刻意不做**：
   - **不把装配结果喂给模型**。那需要一个真实的模型调用路径（§47.2 的六类回归属 **H16**，Provider 余额
     历史记录为 Blocked）。本轮交付的是「计划产生了、能落链、能重建」，不是「模型收到了上下文」。
   - **不给 §41.2 填初值**。见上：初值要由黄金集回放实测确定并记 ADR（§48.7 禁止采外部观察值）。
   - **不让装配器做段间整段裁剪**。判定面的 `CropFromFront` 维度（被裁的可裁段须构成后缀）需要
     「总预算」概念，而 §41.2 明确「预算**按段**分配，不是一个总上限」⇒ 装配器不引入总预算，
     该维度在真实路径上仍只能由手工构造的计划触发，如实登记。
   - **不改判定面与执行体**。`evaluate_assembly` / `record_assembly` / `rebuild_assembly_from_*` 本轮
     **零改动**——本轮补的是它们之间的那一段，不是它们的语义。

## 具名登记（本轮**发现但不修**的弱化，MUST NOT 顺手处理）

1. **H01/H02 的 `status` 仍 `pending`**。本轮闭合的是「②执行体获得了真实入参」，**不是验收达标**。
   ④真实产生者（把装配结果喂给模型）与 §41.2 的分段预算初值标定仍未接线。这与 W106/W127 的
   处置一致：接线到 runtime ≠ 验收达标。
2. **H03/H04 本轮未纳入**。开工时的分组设想是「H01–H04 装配与记忆」，勘定后**放弃了**：四条 registry
   笔记都写着「真实产生者未接线」，但缺的东西**不同型**——H01/H02 缺一个**纯计算**的构造者（本轮
   能补），H03/H04 缺「一次对话轮次**产生**一条记忆候选」的规则（那需要模型输出解析，与 H16 同源）。
   硬凑成一组会让报告里出现「四条都推进了」这类不实陈述。H03/H04 的控制面链已经由 `apps/engine`
   注入 `MemoryAdapter` 接通（现测 `apps/engine/src/lib.rs:327`），缺的只有上游。
3. **`usage_counts_only_what_survived` 对裁剪位置不敏感**。K3 注入（改成取尾部而非头部）只让
   `items_over_the_cap_crop_the_tail_and_record_both_dropped_and_reason` 变红，usage 用例仍是绿的——
   它算的是数量与总和，不比身份。这不是缺陷（身份由前一条钉住），但 MUST NOT 被读作
   「usage 用例守住了裁剪方向」。

## Modified Capabilities

- `agent-harness`：**ADDED** 六条 Requirement（顺序、缺席、段内裁剪、不可裁段、未定义段名、预算不代编）。
  刻意**不 MODIFIED** 既有的 `Every harness layer failure lands on a named, ordered and visible
  degradation` 与 §41 的任何既有条目——它们声明的是**判定结论**，本轮新增的六条声明的是
  **被判定对象的产生**，是另一件事；「一条 Requirement 只声明一条行为」。
