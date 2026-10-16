# ADR 0106：装配根驱动升级事务——暂存不问门、激活先问门

- 状态：Accepted｜日期：2026-10-16｜归属：W128（R10 收口）
- 关联：ADR 0101（装配根回答升级激活门）、ADR 0100（§tested 证据可核验）、ADR 0099（§Operation 有界预算）、
  ADR 0027（一件事一个定义）、`runtime-composition` spec、`upgrade-transaction` spec、R10

## 背景
W123（ADR 0101）让装配根**能答**升级激活门，但 registry `R10` 的剩余缺口逐字写着「运行时仍无升级激活调用点
（`nai-runtime`/`apps/engine` 内无 `UpgradeTransaction` 使用处）」。现测：本轮前 `UpgradeTransaction` 在
`nai-runtime` / `apps/engine` **0 命中**，只出现在 `nai-upgrade-core` 的定义与其自己的测试里
⇒ 核心有 `activate`、装配根有 `SessionGate`，**没有一个地方把两者接起来**，「开播中只暂存不激活」
从未在真实链路上执行。

前置障碍：`Prepared` 在核心之外**无法字面构造**（无构造器，`PreflightReport::checks` 私有），
故调用点只能通过一次**真的** `prepare` 得到它，也就必须拿到 demo 包；而 `nai-runtime` 的
dev-dependencies **够不到另一个 crate 的 `tests/`**。

## 决策
1. **两处入口，不是一处**：`Composition::stage_upgrade`（暂存）与 `Composition::activate_staged`（激活边界）。
   暂存在任何时候都安全、只有切换需要静默，这是核心把 `prepare`/`activate` 分成两调的**唯一理由**；
   运行时若合成一个方法，就会把「被推迟」压成「失败」，而被门控的包恰恰不该失效。
2. **门只被问一次，位置固定在激活边界**：`stage_upgrade` 体内不出现任何门调用。同一件事不在两处定义。
3. **门的事实来自装配根自己持有的真实会话**：复用 ADR 0101 的 `impl ActiveSessionGate for Composition`。
   两个方法都取 `&self`——核心刻意不持有会话状态，而**不可变借用使「缓存一个活跃计数」在类型层面不可能**，
   这条不变量由构造保证，不需要再补一条断言去守。
4. **推迟是状态、错误是错误**：门答 `Deferred{active}` ⇒ `UpgradeActivation::Deferred{active}`（非 `Err`）；
   门答 `Allowed` ⇒ `Ran(Outcome)`；门**报错**（会话不可观测）⇒ 原样上抛 `Conflict`。
   把「未知」表述为推迟，等于把一次未知伪装成一次已知的等待；表述为放行则会让切换在开播中发生。
5. **`Ran` 而非 `Activated`**：回滚也是一次**跑完**的运行，用「已激活」命名它是对回滚的不实陈述。
   运行后的终局 MUST 原样携带核心的 `Outcome`（成因码 + 导出计数），运行时侧不重写、不补零。
6. **预算由调用方给**：运行时**不铸** `Operation`。ADR 0099 的两种具名语义是 bring-up 与「用满上界」，
   升级激活不是其中任何一种，在此铸一个就是第三份会漂移的策略。判据用 `Operation::CEILING`
   （即 ADR 0099 给演练的用法）。
7. **夹具共享化的落点是 `nai-upgrade-core/src/rehearsal.rs`**：该文件本就是「放替身」的地方，且把 demo 包
   从「三份逐字重复的副本」收敛成一处定义。这是**纯增加**（`serde_json`/`sha2` 已在 `[dependencies]`），
   三个测试文件的用例数逐项不变（plan 12 / gates 7 / upgrade 8）⇒ 判据未增减。

## 后果
- 正向：`UpgradeTransaction` 在装配根有了**生产调用点**；「开播中只暂存不激活」这条命题首次在真实会话链路上
  被执行，且有 4 组注入可归因地验证（I1 暂存先问门 ⇒ 4 红／I2 推迟分支照样激活 ⇒ 3 红／I3 吞掉门错误 ⇒ 1 红／
  I4 抹掉导出计数 ⇒ 1 红）。
- **R10 仍不升 `tested`**，三条各自独立的理由（见 design.md）：① 同类缺口的另一半仍在——`downgrade` /
  `require_rollback_compatible` 在核心之外**恰 0 个调用点**（现测），「降档先排空后停用」同样没在真实链路上跑过；
  ② 第④环真值缺口未解——真实安装器与真实分发端点仍是外部注入点，本轮世界效果全经 rehearsal 替身；
  ③ ADR 0100 的 `tested` 门槛（`A03` 判例：剩余缺口是外部真值 ⇒ 不具备可核验证据）。
- 生产代码：`nai-runtime` 只增一个模块、一个 `From` 实现与一次 re-export，不改既有行为路径；无契约/迁移/schema 改动。
- 如实披露：`apps/engine/src` 至今对 `upgrade` **0 命中**（引擎侧升级入口未建）。本轮**不**新建该入口——
  真实安装器不存在，引擎侧入口只能是演练路径，而「演练不得表示生产就绪」是本仓硬规矩。
