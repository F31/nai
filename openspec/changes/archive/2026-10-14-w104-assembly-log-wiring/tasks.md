# w104 任务

## 1 建立 OpenSpec 变更

- [x] 写 `proposal.md` / `design.md` / tasks / 三条 ADDED 需求 / 一条 MODIFIED 需求
- [x] `openspec validate 2026-10-14-w104-assembly-log-wiring --strict --no-interactive` 报 `is valid`

## 2 实现 runtime 接线（H02）

- [x] `crates/nai-storage/src/decision.rs`：新增 `DecisionKind::Assembly`（`as_str()` ⇒ `"assembly"`；
      `criticality()` **不**并入 Critical 表 ⇒ 归 Telemetry）
- [x] **新增迁移** `migrations/sqlite/0007-decision-log-assembly-kind.sql`：逐列重建 `decision_log`
      使 `kind` CHECK 收 `assembly`，逐字复制既有行，重建索引与两个只追加触发器
- [x] `crates/nai-storage/src/migration.rs`：`VERSION` 6 → 7、`SQL` 数组 +1 段
- [x] 版本号连带面同步：`schemas/config/backup.schema.json` const 6 → 7、
      `packages/contracts-ts/src/generated/config/backup.ts` 6 → 7、
      `tests/contracts/schema.test.mjs` 样本 6 → 7 且负例由 5 改 6、
      `crates/nai-storage/src/tests/knowledge_publication.rs` 6 → 7
- [x] 新增迁移**升级路径**测试（v6 库上先证旧 CHECK 拒绝 `assembly`，升级后证既有行逐字保留 + 新种类被接受 +
      触发器仍在）
- [x] `crates/nai-runtime/src/assembly.rs`：`AssemblyRefusal` / `AssemblyOutcome` / `AssemblyRebuild`
- [x] `Composition::record_assembly`：判定 → （可重建）追加完整片段材料 / （不可重建）具名拒绝
- [x] 载荷按**记录进入顺序**落 `id`/`source_ref`/**字面正文** + `model_version` + `prompt_version`；**不带**指针
- [x] `Composition::rebuild_assembly_from_log`：只读真实链 → 按**日志自身顺序**解出片段 → **调用**既有
      `rebuild_assembly` → 映射为拥有所有权的 `AssemblyRebuild`
- [x] `lib.rs`：`pub use assembly::{AssemblyOutcome, AssemblyRebuild, AssemblyRefusal};` 并挂 `mod assembly;`
- [x] 复用装配根既有的单 owner `record()` 与 `next_trace`；**不**新增第二个 trace 生成器；
      `composition.rs` 仅把 `short()` 放宽为 `pub(crate)`（**不**新增第二份逐次调用预算）

## 3 测试与非空洞守卫

- [x] `crates/nai-runtime/tests/assembly.rs`：真实 store 端到端 + `verify_chain`（**12 项**）
- [x] 一次可重建装配被追加为 `assembly` 条目，且载荷含**字面正文**与两个版本
- [x] 往返：`record_assembly` 落链 → `rebuild_assembly_from_log` **只读链**重排 ⇒ `rebuildable == true`
      且 `rebuilt` 的 (id, source_ref) 序列与记录一致
- [x] **非升序**进入顺序的往返（`frag-c, frag-a`）⇒ 落盘顺序 == 进入顺序（**I3 逼出来的用例**）
- [x] 顺序判据：落一条记录顺序 `[A,B]`，用记录 `[B,A]` 重建 ⇒ `RebuildOrderDiffers`（片段须齐全）
- [x] 缺内容：`LoggedFragment.content == None` ⇒ `rebuild.fragment_content_not_in_log`，且不追加
- [x] 缺日志：记录引用不在日志里的 id ⇒ `rebuild.fragment_not_logged`，且不追加
- [x] 无装配可重建：空日志上用非空记录重建 ⇒ 全部 `unlogged_ids`（不新增「无装配」理由）
- [x] 超大载荷 ⇒ `Err`，不是静默截断
- [x] **非空洞守卫**：遍历 `RebuildRejection::ALL`，逐个构造 runtime 场景并断言拒绝名出现在返回值里；
      样本构造函数对拒绝枚举**穷尽 match**（kernel 加变体 ⇒ 编译失败）
- [x] 被拒绝的装配 MUST NOT 追加任何条目；凭据字面量 MUST NOT 出现在任何已落盘行

## 4 非空洞注入验证

- [x] I1 `content` 落成 `null` ⇒ 实测 **5 红**（正文真落盘 / 往返重建 / 顺序 / 非升序顺序 / 超大载荷）
- [x] I2 拒绝分支照旧追加 ⇒ 实测 **7 红**（五条拒绝 + 可区分性 + 非空洞守卫）
- [x] I3 片段按 id 排序落盘 ⇒ 实测 **1 红**（`the_log_preserves_the_entry_order_rather_than_re_sorting_it`；
      **该用例是注入纪律逼出来的**——原来的往返用例用升序 id，此注入会**零信号**）
- [x] I4 载荷漏 `model_version` ⇒ 实测 **1 红**（载荷形状；**断言是注入纪律逼出来的**）
- [x] I5 迁移的 `kind` CHECK 去掉 `assembly` ⇒ 实测 storage **1 红** + runtime **4 红**
      （证明「枚举 + 表 CHECK 两处定义」的守卫承重）
- [x] 备份拷回 + `sha256` 核对（基准 = **过完 `cargo fmt --all` 之后**的那一份；三份文件逐字一致，复跑基线 4/4 与 12/12 全绿）
- [x] 注入临时目录已删除（**MUST NOT** 被 commit）

## 5 登记与文档

- [x] `tests/registry.yaml` 的 H02 追加 `notes`（接线已落地，验收仍 `pending`；**`status` 未改**）
- [x] `docs/kernel.md` 把 W69 的「runtime 与证据生产侧」边界指向新落点（就地 1:1）
- [x] `docs/runtime.md` 新增「装配落链与只读重建（W104，验收 H02）」段
- [x] `docs/storage.md` 的 telemetry 种类枚举 1:1 加 `assembly`；并**就地订正陈旧的**迁移段（`至 0005`／schema 5）
      与 `db_schema_version=5` → 7
- [x] `VERSION = 6，6 个脚本` 三处（`docs/kernel.md`／`docs/development-plan.md`／`AGENTS.md`）1:1 改为 7/7
      （**全仓扫描确认无残留旧值**）
- [x] `docs/development-plan.md` 加 W104 条目；`AGENTS.md` 加 W104 轮次段
- [x] `docs/adr/0082-assembly-log-wiring.md`
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增 §6.27，并更新 §10 计数索引
- [x] `docs/reports/2026-10-14-w104-assembly-log-wiring.md`（含 §八 门控实测 / §十 遗留）
- [x] 接线核实：Rust 集成测试由 `cargo test --workspace` 自动覆盖 ⇒ **无需**新增登记（不写第二处清单）

## 6 每轮闭环（流程项，**不自我勾选**）

归档与提交按 `AGENTS.md` §0 的闭环执行：门控通过 → tasks 归档前全勾 → `archive --yes`（系统日期戳）
→ 两条 validate 都跑 → 一次 commit + push → 单次低频 CI 观测 → 回填报告（回填提交自身不回填）。
