# Design: W128 — 运行时升级激活调用点

## 缝（现测）
`grep -rn "UpgradeTransaction" crates apps xtask` 本轮前命中：`nai-upgrade-core`（定义 + 文档）与其
`tests/{plan,gates}.rs`；`nai-runtime`/`apps/engine` **零命中**。核心的 `UpgradeTransaction::activate`
与 `SessionGate::activation` 谁都没在装配根里接起来。

## 决策

### 1. 两处入口，不是一处
`stage_upgrade`（暂存）与 `activate_staged`（激活边界）分开，因为**暂存在任何时候都是安全的，只有切换需要静默**。
核心刻意把 `prepare`/`activate` 分成两调就是为了这个；运行时若合成一个方法，就会把「被推迟」压成「失败」，
而被门控的包恰恰不该失效。

### 2. 门只被问一次，且在激活边界
`stage_upgrade` 体内**不出现** `SessionGate` / `activation` / `require_idle`（注入 I1 证明这条分离性被测定）。
「无人活跃」这一判定只此一处 ⇒ 不会出现第二份要跟着改的定义。

### 3. 门的事实来自真实会话，来自装配根自己
复用 W123 的 `impl ActiveSessionGate for Composition`（读**自己持有的** `Session`，活跃 = 在播）。
两个方法都取 `&self`：核心刻意不持有会话状态，而**不可变借用使「缓存一个活跃计数」在类型层面不可能**
——这是「同一件事 MUST NOT 两处定义」在此处的构造性保证，不需要再加一条断言去守。

### 4. 推迟是状态、错误是错误，两者 MUST NOT 折叠
- 门答 `Deferred { active }` ⇒ 返回 `UpgradeActivation::Deferred { active }`（**不是** `Err`）。
- 门答 `Allowed` ⇒ 跑事务，返回 `Ran(Outcome)`（`Committed` 或带成因的 `RolledBack`）。
- 门**报错**（会话锁不可观测）⇒ 原样上抛 `Conflict`。读作「无人活跃」会让版本切换在开播中发生；
  读作「推迟」则把一次**未知**伪装成一次**已知的等待**。按「不可解析 ⇒ 失败并分类，不静默降级」。
- `Ran` 而非 `Activated`：回滚也是一次**跑完**的运行，用「已激活」命名它就是对回滚的不实陈述。

### 5. `Operation` 预算由调用方给
运行时不铸预算：只有调用方知道这个包的迁移要多久，而在此铸一个预算就是**第三份会漂移的策略**
（ADR 0099 的两种具名语义是 bring-up 与「用满上界」，升级激活不是其中任何一种）。
判据里用 `Operation::CEILING`——正是 ADR 0099 给演练的「用满上界」用法。

### 6. 夹具共享化的落点是 `src/rehearsal.rs`，不是 `tests/common/`
`nai-runtime` 的 dev-dependencies **不能引用另一个 crate 的 `tests/`**；而 `Prepared` 在核心之外
**无法字面构造**（无构造器、`PreflightReport::checks` 私有）⇒ 运行时的调用点判据**必须**走一次真的
`prepare`，也就必须拿到那份 demo 包。故把三份重复夹具提到 `src/rehearsal.rs`（该文件本就是「放替身」的地方）。
**代价已付**：core crate 全量 **99 passed / 0 failed**，三处 `fn facts()`/`fn manifest(` 命中数均为 **0**，
三个测试文件的用例数逐项不变。

### 7. 明确不做
- 不在 `apps/engine` 新建升级入口：真实安装器不存在，引擎侧入口只能是演练路径，而「演练不得表示生产就绪」
  是本仓硬规矩。`apps/engine/src` 至今对 `upgrade` **0 命中**，如实登记为未竟之工。
- 不做降档侧调用点（`downgrade`/`require_rollback_compatible` 仍只在核心测试里被调用）——同类缺口的另一半，
  另立一轮。
- 不改 `Operation` 预算、不改会话生命周期机、不改契约。

## registry 判定：**维持 `implemented`**
三条各自独立的理由（逐项核实，非「同族」推断）：
1. **同类缺口的另一半仍在**：`downgrade` / `require_rollback_compatible` 在核心之外**恰 0 个调用点**
   （现测；只在 `crates/nai-upgrade-core/tests/gates.rs` 里被调用）⇒「降档先排空后停用」同样没在真实链路上跑过。
2. **第④环真值缺口未解**：真实安装器（staging/快照/rename 切换）与真实分发端点仍是外部注入点，
   本轮所有世界效果都经 rehearsal 替身。
3. **ADR 0100 的 `tested` 门槛**：`tested` 需**可核验的归档证据**且不得因「补个守卫」而升级；
   `A03` 的判例是「结构守卫通过但剩余缺口是外部真值 ⇒ 不具备可核验证据」。R10 同形。

## 验证命令
- `cargo test -p nai-runtime --test upgrade`（5 条）
- `cargo test -p nai-runtime -p nai-upgrade-core`（99 + 96 全绿）
- `cargo fmt --all --check`、`cargo clippy -p nai-runtime -p nai-upgrade-core --all-targets --all-features`
- `cargo xtask check --profile full`（全量门控）
- 注入验证 4 组（见 tasks.md），每组用「恰能走到它那一层」的注入并记录红的是哪几条。
