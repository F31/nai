## 0. 前置核实

- [x] 确认两族 CI 失败的内容与落点（`engine.rs:289`／`pipeline.rs` 的 `DEADLINE_EXCEEDED`），并确认两次提交只差一个 `.md`
- [x] 确认 W48 报告 §5 第 2 条把「nai-runtime 的时序脆弱」明确留给单独一轮
- [x] 确认 `ControlApi` 与装配根共享同一 `Arc<Mutex<Session>>`（`apps/engine/src/lib.rs:314`、`control-api/src/lib.rs:204`）
- [x] 确认两侧都 fail-fast 取锁（`composition.rs:196`、`control-api/src/lib.rs:434`）
- [x] 确认 `live_epoch()` 每 tick（`DRILL_POLL`=5ms）调 `session_lock()` 且以 `?` 上抛（`apps/engine/src/lib.rs:561`、`:582`）
- [x] 确认 `Session::revoke` 自身只剩 `actor_allowed` 前置（`nai-kernel/src/session.rs:296`）⇒ 丢撤权只有取锁一条通道
- [x] 确认 `Store::open` 含全链迁移（`store.rs:142` → `migration.rs:6` 的 6 个脚本）且连接以 `synchronous=FULL` 打开
- [x] 确认 ADR 0004 第 13 行（最长 30s）与 ADR 0038 第 76 行（安全层不变量）给出上界
- [x] 确认同仓同类迁移操作既有取值为 30s（`nai-storage/src/tests.rs:10`、`nai-upgrade-core/tests/*`）

## 1. 缺口一：瞬态锁竞争不得夺走安全决定

- [x] `crates/nai-runtime/src/composition.rs` 加 `SESSION_REVOKE_BUDGET`（250ms）与 `SESSION_POLL_BUDGET`（5ms）
- [x] 加 `session_lock_within(budget)`：有界等待重试，耗尽仍返回显式 `Conflict`（不静默跳过）
- [x] `observe` / `control` / `enforce_duty` 改用有界等待
- [x] 加 `Composition::live_epoch() -> Result<Option<u64>>`：`Some(e)` / `None`（竞争）/ `Err`（仅锁中毒）
- [x] `apps/engine/src/lib.rs`：`tick` 的 epoch 检查改为「未观测到则跳过本 tick，`observe` 仍执行」
- [x] `supervise` 错误出口改用 `.ok().flatten().unwrap_or(floor)`
- [x] 监督循环终止只允许来自 HALT / REVOKED / 真实故障

## 2. 缺口二：bring-up 预算按工作量标定

- [x] `composition.rs` 加 `pub const BRINGUP_OPERATION: Duration = 30s`（ADR 0004 上界）
- [x] `Composition::prepare` 的 `Store::open` 使用 `BRINGUP_OPERATION`
- [x] `SHORT_OPERATION` 保持 5s（逐次调用，不含迁移）
- [x] `apps/engine/src/lib.rs` 的 `--provision` 复用同一常量（一处定义）
- [x] `store open` 失败文案指名**步骤**与**已耗时**
- [x] `apps/engine/src/pipeline.rs` 的两处 5s 读取**保持不变**（不含迁移），并在报告中写明这是判据结果而非遗漏

## 3. 测试

- [x] `crates/nai-runtime/src/composition.rs` 加 `#[cfg(test)]`：钉死三个常量及其关系（poll ≤ revoke ≤ 30s；bring-up > 逐次且 ≤ 30s）
- [x] `crates/nai-runtime/tests/composition.rs`：**持锁**时 `control` 仍在预算内落地（epoch 递增）
- [x] 同上：**持锁超过 poll 预算**时 `live_epoch()` 返回 `Ok(None)`，**不是** `Err`
- [x] 正向前置：无竞争时 `live_epoch()` 返回 `Ok(Some(0))`（否则「恒报未观测到」也能让测试变绿）
- [x] 用 mpsc 握手确保持锁**先于**被测调用（不用 sleep 竞猜）
- [x] 非空洞自检：注入（把有界等待改回 `try_lock`／把 `Ok(None)` 改回 `Err`／把 bring-up 改回 5s）各自被抓住后回滚
- [x] **不新增测试目标**（断言加在既有 `crates/nai-runtime/tests/composition.rs` 内）

## 4. 文档

- [x] `docs/reports/2026-10-12-w70-transient-host-conditions.md`
- [x] `docs/kernel.md`：W70 段（含「nai-runtime 时序脆弱」与 W48 §5.2 遗留项的衔接）
- [x] `docs/development-plan.md`：W70 轮次条目 + 下一主线 + 更新日期
- [x] `AGENTS.md`：W70 轮次段
- [x] 如实登记：family 1 在 CI 上的**触发时点**未能还原；本机不能证明 Linux 侧成立

## 5. 闭环

- [x] `cargo xtask check --profile full` 通过（记录目标数与通过项数，与 W69 对照）
- [x] `openspec validate w70-transient-host-conditions --strict` 通过（需求正文 < 500 字符）
- [x] 勾选本文件全部任务
- [x] `openspec archive w70-transient-host-conditions --yes`（核对归档目录日期戳）
- [x] 两条 validate 口径都跑（`--all --strict` / `--all --strict --archived`）
- [x] 一次 commit + push
- [x] 单次低频查询核对 CI 结论（避开匿名限流）并回填报告
