# w106 任务

## 1 建立 OpenSpec 变更

- [x] 写 `proposal.md` / `design.md` / tasks / 四条 ADDED 需求 / 一条 MODIFIED 需求
- [x] `openspec validate 2026-10-14-w106-assembly-judgement-wiring --strict --no-interactive` 报 `is valid`

## 2 实现 runtime 接线（H01）

- [x] `crates/nai-runtime/src/assembly.rs`：`record_assembly` 签名加**第一**个参数
      `plan: &AssemblyPlan<'_>`（H01 输入），与既有 `record` / `fragments` 并列
- [x] **两项判定都跑、都不短路**：`evaluate_assembly(plan)` 与 `rebuild_assembly(record, fragments)`
      各自独立调用；H01 拒绝时 H02 照跑，反之亦然
- [x] `AssemblyOutcome` 扩为**四态**：`Logged` / `NotCallable` / `NotRebuildable` / `Neither`
- [x] `AssemblyRefusal` 扩为携带两侧拒绝名（`rejections` + `rebuild_rejections`）+ `missing_segments`
      + 既有的三类可疑 id
- [x] **入口一致性前置**：`plan` 各段片段标识序列 == `record.fragment_ids` 序列，不同 ⇒ 具名拒绝
      `assembly.input_ids_differ`，零新增
- [x] 载荷**补齐** §41.5 的四项：保留 `fragments`／`model_version`／`prompt_version`，
      新增 `segment_usage`（按段 tokens/chars/items + 自有 budget）与 `crops`（按段 dropped + reason）
- [x] 载荷取值**照抄判定输入**：MUST NOT 由正文重算、MUST NOT 补零、MUST NOT 改写
- [x] 载荷**不夹带**未审字段：片段 `scope`/`age_ms`/`trust`/`source` 与判定诊断清单一律不落
- [x] 新增 `Composition::read_logged_assemblies(&self, limit: u32) -> Result<Vec<LoggedAssembly>>`：
      **逐行**读回、不可解析 ⇒ 具名失败、只读不追加
- [x] `rebuild_assembly_from_log` 的 payload **解析**改为复用 `read_logged_assemblies`
      （**不**改其池化判定语义）
- [x] `lib.rs`：导出 `LoggedAssembly`（及新增的记录类型）与既有 `assembly::*` 保持一致
- [x] 复用装配根既有的单 owner `record()` 与 `next_trace`；**不**新增第二个 trace 生成器

## 3 测试与非空洞守卫

- [x] `crates/nai-runtime/tests/assembly.rs`：既有 ~15 处 `record_assembly(...)` 调用点补 `plan` 夹具
- [x] 既有 12 项用例在四态下全部仍绿（`expect_logged` / `expect_refusal` 同步为按态断言）
- [x] 一次两侧都放行的装配被追加，载荷**键集恰为 5 键**、`fragments` 每项恰为 3 键、`crops` 每项恰为 3 键
- [x] 裁剪段的落账：该段的 tokens/chars/items 用量齐备，且 `crops` 同时含 dropped 与 reason
- [x] 用量**照抄**：构造「判定输入的用量 ≠ 正文长度」的样本，断言落账值与判定输入一致
- [x] **不可调用 ⇒ 具名拒绝且链零新增**（缺不可裁段；断言拒绝名 + 链无新条目）
- [x] **两侧都拒 ⇒ 两类拒绝名都在**（`assembly.*` 与 `rebuild.*` 同时出现）
- [x] **四态两两可分**：三种拒绝与「已落账」四种结局互相不可混同
- [x] **入口一致性前置**：id 序列不一致 ⇒ 具名拒绝；**顺序不同但集合相同** ⇒ 同样拒绝
- [x] **往返**：一次装配落链后用只读入口逐行读回，四项**逐字相等**
- [x] **历史行兼容**：手工落一条不含 `segment_usage`/`crops` 的旧形状行 ⇒ 读回成功且两项为空
- [x] **逐行不池化**：两次装配各落一行 ⇒ 读回两行且互不合并
- [x] **不可解析行 ⇒ 具名失败**（不是静默跳过）
- [x] **非空洞守卫（H01 侧）**：遍历 `AssemblyRejection::ALL` 十四条，逐个构造运行期 plan 并断言拒绝名
      出现在返回里；样本构造函数对拒绝枚举**穷尽 match**（kernel 加变体 ⇒ 编译失败）
- [x] W104 既有的 `every_rebuild_rejection_is_reachable_through_the_runtime`（`RebuildRejection::ALL` 六条）仍在
- [x] 被拒绝的装配 MUST NOT 追加任何条目；凭据字面量 MUST NOT 出现在任何已落盘行

## 4 非空洞注入验证

- [x] I1 只跑 H02、不跑 H01 ⇒ 记录红条数（**4 红**）
- [x] I2 载荷仍只落三键 ⇒ 记录红条数（**4 红**）
- [x] I3 H01 拒绝时提前 `return`（短路 H02）⇒ 记录红条数（**2 红**）
- [x] I4 把 `Neither` 折进 `NotCallable` ⇒ 记录红条数（**2 红**）
- [x] I5 `segment_usage` 由正文重算而非照抄 ⇒ 记录红条数（**3 红**）
- [x] I6 一致性前置删掉 ⇒ 记录红条数（**1 红**）
- [x] I7 逐行读回改成池化 ⇒ 记录红条数（**1 红**）
- [x] I8 旧形状行被当成解析失败 ⇒ 记录红条数（**1 红**）
- [x] 逐字节还原：备份拷回 + `sha256` 核对（基准 = **过完 `cargo fmt --all` 之后**的那一份）
- [x] 注入临时目录已删除（**MUST NOT** 被 commit）

## 5 登记与文档

- [x] `tests/registry.yaml` 的 H01 追加 `notes`（接线已落地，验收仍 `pending`；**`status` 未改**）
- [x] `docs/kernel.md` 把 W69 的「runtime 与证据生产侧」边界句指向新落点（就地 1:1）
- [x] `docs/runtime.md` 新增「装配判定接线与四项落链（W106，验收 H01）」段
- [x] `docs/development-plan.md` 加 W106 条目；`AGENTS.md` 加 W106 轮次段
- [x] `docs/adr/0084-assembly-judgement-wiring.md`
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增条目（§6.29），并更新计数索引（§10）
- [x] `docs/reports/2026-10-14-w106-assembly-judgement-wiring.md`（含门控实测 / 遗留 / 池化缺陷登记）
- [x] 接线核实：Rust 集成测试由 `cargo test --workspace` 自动覆盖 ⇒ **无需**新增登记（不写第二处清单）

## 6 每轮闭环（流程项，**不自我勾选**）

归档与提交按 `AGENTS.md` §0 的闭环执行：门控通过 → tasks 归档前全勾 → `archive --yes`（系统日期戳）
→ 两条 validate 都跑 → 一次 commit + push → 单次低频 CI 观测 → 回填报告（回填提交自身不回填）。

## 勘误（W127，2026-10-16）

上文任务里「（**不**改其池化判定语义）」所登记的遗留，**已由 W127 闭合**（`rebuild_assembly_from_log`
改为逐行作为独立**条目**判定，具名 `rebuild.not_in_one_entry`）。本文档按当时状态保留，MUST NOT 就地改写。
见 [ADR 0105](../../../../docs/adr/0105-rebuild-in-one-entry.md)。
