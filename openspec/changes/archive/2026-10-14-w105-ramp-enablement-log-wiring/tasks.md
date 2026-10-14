# w105 任务

## 1 建立 OpenSpec 变更

- [x] 写 `proposal.md` / `design.md` / tasks / 两条 ADDED 需求 / 一条 MODIFIED 需求
- [x] 实测 MODIFIED 后 §47.3 需求正文长度（改前 **436** → 改后 **446**，门限 500）
- [x] `openspec validate 2026-10-14-w105-ramp-enablement-log-wiring --strict --no-interactive` 报 `is valid`

## 2 实现 runtime 接线（H18）

- [x] `crates/nai-runtime/src/ramp.rs`：`RampRefusal` / `RampOutcome`（**三态**）
- [x] `Composition::record_promotion(&PromotionRequest<'_>)`：**先读 `verdict.allowed`、再读
      `verdict.record_to_append`**（MUST NOT 用 `record_to_append.is_some()` 反推 `allowed`）
- [x] 放行且有记录 ⇒ 追加一条 `DecisionKind::Approval`（**复用既有种类，零迁移**）
- [x] 放行但目标阶段在门控之前 ⇒ `RampOutcome::NoRecordRequired { from, to }`，**不追加、非拒绝**
- [x] 拒绝 ⇒ `RampOutcome::Refused`（逐条拒绝名 + **全部**缺失类别 + 凭据泄露**字段名**），**一字节不落盘**
- [x] 载荷恰为四项：`mechanism`/`from`/`to`/`fields`；**不含** `req.touched` / `req.passed`
- [x] 复用既有 `self.record(...)` 与 `next_trace("ramp")`；**不**新增第二条写入路径与第二个 trace 生成器
- [x] `lib.rs`：`pub use ramp::{RampOutcome, RampRefusal};` 并挂 `mod ramp;`

## 3 测试与非空洞守卫

- [x] `crates/nai-runtime/tests/ramp.rs`：真实 store 端到端 + `verify_chain`（**9 项**）
- [x] 放行晋升落成 `approval` 条目（`persisted=true`、链校验通过、链上恰 +1 行）
- [x] 键集断言：载荷**不含**请求携带的两份回归类别集（**I4 逼出来的用例**）
- [x] 目标阶段在门控之前 ⇒ `NoRecordRequired`，链上零新增，且**不是** `Refused`（**I3 逼出来的用例**）
- [x] 三态可区分性：`Logged` / `NoRecordRequired` / `Refused` 三者互异且可判定（**I3 逼出来的用例**）
- [x] 八条具名拒绝各自的运行期场景：每条都返回 `Refused` 且链上零新增
- [x] `ramp.regression_missing` 列出**全部**缺失类别（不只第一个）
- [x] **非空链**上的拒绝零新增（避免「不变」与「本来就没有」不可区分）
- [x] 凭据字面量 MUST NOT 出现在任何已落盘行；字段名像密钥而值干净 ⇒ 照旧落盘（INV-11 按值判）
- [x] **非空洞守卫**：遍历 `RampRejection::ALL`（**8** 条）逐个构造运行期场景并断言拒绝名出现在返回值里；
      样本构造函数对拒绝枚举**穷尽 match**（kernel 加变体 ⇒ 编译失败）
- [x] `cargo fmt --all` 后跑基线并记录**现测**计数（`ramp` 9/9 全绿；全量门控计数见 §6）

## 4 非空洞注入验证

- [x] I1 拒绝分支照旧追加 ⇒ 实测 **4 红**（非空洞守卫 / 缺失类别 / 非空链 / 凭据）
- [x] I2 载荷漏 `record.fields` ⇒ 实测 **2 红**（键集 / 干净值命名）
- [x] I3 目标阶段在门控之前也追加 ⇒ 实测 **2 红**（`NoRecordRequired` / 三态可区分）
- [x] I4 载荷夹带 `touched`/`passed` ⇒ 实测 **1 红**（键集）
- [x] I5 把 `allowed` 折进 `record_to_append.is_some()` ⇒ 实测 **5 红**
      （凭据 / 三态可区分 / 非空链 / 非空洞守卫 / 缺失类别）
- [x] 备份拷回 + `sha256` 核对（基准 = **过完 `cargo fmt --all` 之后**的那一份；三份文件逐字一致，
      复跑基线 9/9 全绿）
- [x] 注入临时目录 `.w105-probe` 已删除（**MUST NOT** 被 commit；`git status` 无该目录）

## 5 登记与文档

- [x] `tests/registry.yaml` 的 H18 追加 `notes`（接线已落地，验收仍 `pending`；**`status` 未改**）
- [x] `docs/kernel.md` 把 W67 的「runtime 接线」边界指向新落点（就地 1:1，两处：启用记录段 + 边界段）
- [x] `docs/runtime.md` 新增「爬坡启用记录落链（W105，验收 H18）」段
- [x] `docs/storage.md` 的 `approval` 类别补一段：它同时承载 §47.3 的启用记录（两种载荷形状，如实披露）
- [x] `docs/development-plan.md` 加 W105 条目；`AGENTS.md` 加 W105 轮次段
- [x] `docs/adr/0083-ramp-enablement-log-wiring.md`
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增 §6.28，并更新 §10 计数索引（**现测** 94/892 / 13 / 317 / 0）
- [x] `docs/reports/2026-10-14-w105-ramp-enablement-log-wiring.md`（含 §八 门控实测 / §十 遗留）
- [x] 接线核实：Rust 集成测试由 `cargo test --workspace` 自动覆盖 ⇒ **无需**新增登记（不写第二处清单）

## 6 每轮闭环（流程项，**不自我勾选**）

归档与提交按 `AGENTS.md` §0 的闭环执行：门控通过 → tasks 归档前全勾 → `archive --yes`（系统日期戳）
→ 两条 validate 都跑 → 一次 commit + push → 单次低频 CI 观测 → 回填报告（回填提交自身不回填）。
