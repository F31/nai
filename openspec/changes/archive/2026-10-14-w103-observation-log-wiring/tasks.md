# w103 任务

## 1 建立 OpenSpec 变更

- [x] 写 `proposal.md` / `design.md` / tasks / 两条 ADDED 需求 / 一条 MODIFIED 需求
- [x] `openspec validate 2026-10-14-w103-observation-log-wiring --strict --no-interactive` 报 `is valid`

## 2 实现 runtime 接线（H15）

- [x] `crates/nai-runtime/src/observation.rs`：`ObservationRefusal` / `ObservationOutcome`
- [x] `Composition::record_observation`：判定 → （可落日志）追加脱敏载荷 / （不可落日志）具名拒绝
- [x] 载荷恰为 `verdict.redacted` + 层标签 + `redacted_viewer`；**不带** `record.subject`、**不取** `record.fields`
- [x] `composition.rs`：`next_trace` 可见性放宽到 `pub(crate)`（**不**新增第二个 trace 生成器）
- [x] `lib.rs`：`pub use observation::{ObservationOutcome, ObservationRefusal};` 并挂 `mod observation;`

## 3 测试与非空洞守卫

- [x] `crates/nai-runtime/tests/observation.rs`：真实 store 端到端 + `verify_chain`
- [x] 五类各一条可落日志记录被追加，载荷 `layer` 与层标签一致
- [x] 载荷是判定材料而非原字段（超长字段 ⇒ `[truncated]`，非原值）
- [x] 凭据值 ⇒ 具名拒绝 `observation.plaintext_credential`，日志**未新增**条目，且字面量不出现在任何已落盘行
- [x] 缺必需项 ⇒ `observation.missing_required_field`；观众原文未脱敏 ⇒ `observation.viewer_text_unmasked`；两者都不追加
- [x] 正向量：字段名含 `key` 而值干净 ⇒ 可落日志（不是字段名黑名单）
- [x] 五类断言遍历 `OBSERVATION_LAYERS` 自身，MUST NOT 手写层名清单
- [x] 另加：被 **未审字段**（`subject`）里的凭据 MUST NOT 落盘（runtime 不做加法）

## 4 非空洞注入验证

- [x] I1 载荷改取 `record.fields` ⇒ 实测 1 条红（载荷形状）
- [x] I2 拒绝分支不再早退 ⇒ 实测 4 条红（三条拒绝 + 可区分性）
- [x] I3 只接一类 layer ⇒ 实测 5 条红（含五类完整性守卫）
- [x] I4 载荷被 `record.subject` 富化 ⇒ 实测 1 条红（未审字段）
- [x] 备份拷回 + `sha256` 核对（基准 = **过完 `cargo fmt --all` 之后**的那一份）

## 5 登记与文档

- [x] `tests/registry.yaml` 的 H15 追加 `notes`（接线已落地，验收仍 `pending`；**`status` 未改**）
- [x] `docs/runtime.md` 记录观测落链入口与其载荷不变式
- [x] `docs/kernel.md` 把 W66 的「runtime 接线未做」指向新落点（就地 1:1）
- [x] `docs/development-plan.md` 加 W103 条目；`AGENTS.md` 加 W103 轮次段
- [x] `docs/adr/0081-observation-log-wiring.md`
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增 §6.26，并更新 §10 计数索引
- [x] `docs/reports/2026-10-14-w103-observation-log-wiring.md`（含 §八 遗留）
- [x] 接线核实：Rust 集成测试由 `cargo test --workspace` 自动覆盖 ⇒ **无需**新增登记（不写第二处清单）

## 6 每轮闭环（流程项，**不自我勾选**）

归档与提交按 `AGENTS.md` §0 的闭环执行：门控通过 → tasks 归档前全勾 → `archive --yes`（系统日期戳）
→ 两条 validate 都跑 → 一次 commit + push → 单次低频 CI 观测 → 回填报告（回填提交自身不回填）。
