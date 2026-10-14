# w107 任务

## 1 建立 OpenSpec 变更

- [x] 写 `proposal.md` / `design.md` / tasks / 两条 ADDED 需求
- [x] `openspec validate 2026-10-14-w107-memory-write-gate-wiring --strict` 报 `is valid`

## 2 实现存储层访问器（`crates/nai-storage/src/store.rs`）

- [x] `Store::record_memory(&mut self, op, workspace, actor, record: &MemoryRecord) -> Result<()>`：
      `self.start(op)` + `id()` 校验作用域标识 + `access(..., "session.manage", "*")` + 写九列
- [x] 九列**逐字**取自 `record` 的九字段（`viewer_scoped_id` → `viewer_scope`、
      `expires_at_utc_ms` → `expires_at_ms`），MUST NOT 重排、MUST NOT 补默认值
- [x] 主键冲突 ⇒ 具名 `Conflict`；**不**在存储层重跑来源判定（§5.1）
- [x] **不**在存储层加写入面的时效判定（§5.2）
- [x] `Store::memory_records(&self, op, workspace, actor, session) -> Result<Vec<MemoryRecord>>`：
      按作用域读回、顺序确定性、逐列还原；MUST NOT 施加 §42.3 判定（§9）
- [x] 既有方法一行不改；`migrations/**` 一行不改

## 3 实现 runtime 写入口（`crates/nai-runtime/src/memory.rs`）

- [x] 新增模块 `memory.rs`：`MemoryWriteRefusal`（拒绝名 + 记录标识）与
      `MemoryWriteOutcome`（**恰两态** `Written` / `Refused`）
- [x] `Composition::write_memory(&mut self, candidate, policy) -> Result<MemoryWriteOutcome>`：
      **先** `admit_write`；拒绝 ⇒ `Ok(Refused)` 且**一列不落**；放行 ⇒ 落库并返回 `Ok(Written)`
- [x] **判定 + 落库是一处**：MUST NOT 拆成 `evaluate` / `persist` 两个出口
- [x] 落库走既有的单 owner 存储路径（`self.store()` 的 `try_lock`），MUST NOT 另开写入路径
- [x] `lib.rs`：导出 `MemoryWriteOutcome` / `MemoryWriteRefusal`
- [x] 被审对象（`admit_write` / `read_records` 及其它 kernel 判定规则）**一行不改**

## 4 测试与非空洞守卫

- [x] 新增 `crates/nai-runtime/tests/memory.rs`（新测试目标，由 `cargo test --workspace` 自动覆盖）
- [x] 放行 ⇒ 恰一行，且九列与判定输入**逐字相等**
- [x] 拒绝 ⇒ **零行**，且拒绝名在场
- [x] **非空洞守卫**：遍历 `MemoryRejection::ALL`，`refusal_sample` 对拒绝枚举**穷尽 `match`**；
      写面**恰八条**（`WRITE_FACE`）逐个可达（kernel 加变体 ⇒ 编译失败）
- [x] **能力开关的写侧**：观众长期记忆关闭 ⇒ viewer 候选在写入面被拒且零落库；
      同一候选在开关打开后**确实落库**（证明拒的是开关，不是别的缺陷）
- [x] **读取面访问器**：落库后按作用域读回，九列**逐字往返相等**（含 `viewer_scope` 映射）
- [x] **读回不判定**：已过期记录仍被读回（时效判定不由取数入口施加）
- [x] **主键冲突具名**：同一记录写两次 ⇒ 第二次 `Err(Conflict)`，且**不**产生第二行
- [x] 落点核实：一次记忆写入到达 `memory_records` 而**决策日志零新增**（不是第二处定义）

## 5 非空洞注入验证

- [x] I1 写入入口不跑 `admit_write`、直接落库 ⇒ **3 红**
- [x] I2 拒绝分支照旧落库（先落库、再返回 `Refused`）⇒ **3 红**
- [x] I3 `purpose` 列由 `content` 重算 ⇒ **4 红**
- [x] I4 `viewer_scoped_id` → `viewer_scope` 映射被常量填空 ⇒ **2 红**
- [x] I5 读回入口丢掉会话过滤（`(session_id=?2 OR 1=1)`）⇒ **1 红**
- [x] I6 读回入口加上时效过滤（变成 §42.3 形状）⇒ **1 红**
- [x] I7 写入面把能力开关答成「恒开」（第二个应答点）⇒ **2 红**
- [x] 逐字节还原：备份拷回 + `sha256` 核对（基准 = **过完 `cargo fmt --all` 之后**的那一份）
- [x] 注入临时目录已删除（**MUST NOT** 被 commit）

## 6 登记与文档

- [x] `tests/registry.yaml`：H03 追加 `notes`（写入面接线已落地、验收仍 `pending`）；
      H04 追加 `notes`（**具名开放问题**：`revoked` 无承载列）。**`status` 未改**
- [x] `docs/storage.md` 新增记忆表段（九列映射 + 三项不落库的如实登记）
- [x] `docs/runtime.md` 新增「记忆写入判定接线（W107，验收 H03）」段
- [x] `docs/kernel.md` 把记忆面的 runtime 边界句就地订正并指向新落点
- [x] `docs/development-plan.md` 加 W107 条目；`AGENTS.md` 加 W107 轮次段
- [x] `docs/adr/0085-memory-write-gate-wiring.md`
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增条目，并更新计数索引
- [x] `docs/reports/2026-10-14-w107-memory-write-gate-wiring.md`（含门控实测 / 遗留 / H04 开放问题）

## 7 每轮闭环（流程项，**不自我勾选**）

归档与提交按 `AGENTS.md` §0 的闭环执行：门控通过 → tasks 归档前全勾 → `archive --yes`（系统日期戳）
→ 两条 validate 都跑 → 一次 commit + push → 单次低频 CI 观测 → 回填报告（回填提交自身不回填）。
