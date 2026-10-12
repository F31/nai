# 提案（w77：一个预算一份定义——定义归属执行者，而非首个受害者）

## Why

`BRINGUP_OPERATION`（30 s）由 W70 立起，落点是 `crates/nai-runtime/src/composition.rs`。
当时的注释写下了它的目的：

> Sized here once so the engine **cannot hold a second, drifting copy**.

规则本身是对的。W76 在**第二个消费者**（`crates/nai-runtime/tests/common/mod.rs`）上踩了坑并修好。
本轮在**第三个消费者**上遇到同一族问题的**结构性版本**：

`crates/nai-control-api/src/tests.rs::seed_approval_store`（`:349-353`）自造
`Operation::new(Duration::from_secs(10))` 去跑 `Store::open`（全新数据根 + 全链迁移）——
**又是「预算与工作量不匹配」**，而 **W75 在 CI 上的失败正落在该文件 `:364`**。

要修它，必须先回答「那份唯一的定义从哪儿拿」。核实结论是**不能从 `nai-runtime` 拿**：

| 事实 | 证据 | 后果 |
| --- | --- | --- |
| `nai-runtime` **依赖** `nai-control-api` | `crates/nai-runtime/Cargo.toml:17` | 反向引用构成**依赖环** |
| `nai-control-api` 与 `nai-runtime` 都依赖 `nai-storage` | 两者 `Cargo.toml` | `nai-storage` 是共同可达点 |
| `BRINGUP_OPERATION` 只定义在 `nai-runtime` | `composition.rs:31` | shared 层拿不到它 |
| `Operation` 的 30 s 上界是**字面量** | `nai-security/src/lib.rs:36` | 「取上界」不可判定 |

⇒ **定义点放错了层**。bring-up 的**执行者**是 `Store::open`，它在 `nai-storage`；
`nai-runtime` 只是**首个遇到问题的消费者**，不是**这类工作的主人**。

## What Changes

1. **`nai-security`**：把 `Operation::new` 的 30 s 上界提为 **`Operation::CEILING`**（public const），
   `new()` 改用它。⇒「bring-up 取上界」从**数值巧合**变成**可判定表达式**。
2. **`nai-storage`**：新增 **`pub const BRINGUP_OPERATION: Duration = Operation::CEILING;`**
   —— bring-up 工作量的执行者在这里，定义也放这里。
3. **`nai-runtime`**：`composition.rs` 的 `pub const BRINGUP_OPERATION` 改为 **re-export**
   （`pub use nai_storage::BRINGUP_OPERATION;`）⇒ `nai_runtime::BRINGUP_OPERATION` 保持可用
   （`apps/engine/src/lib.rs:27` 无需改动），注释订正为「Sized in `nai-storage` once…」。
4. **消费者改引**（消除数值副本，不引入任何依赖边）：
   - `nai-storage/src/tests.rs:10`（**定义所在的 crate 自己也有副本**）
   - `nai-storage/src/upgrade.rs:183`
   - `nai-control-api/src/tests.rs::seed_approval_store`（**本轮主角**：10 s → bring-up 预算）
   - `nai-upgrade-core/tests/{plan,migration,lifecycle,gates}.rs` 的 `Operation::new(Duration::from_secs(30))`
   - `nai-provider-core/examples/openai_smoke.rs` 的两处字面量，以及
     `nai-provider-core/examples/live_provider_smoke.rs` 与 `xtask/examples/golden_eval.rs` 里
     **各一处私有的 `const OPERATION_CEILING`**（实现期全仓扫描发现；正是注释所禁的
     *"a second, drifting copy"* 的**活标本**，一并删除并改引 `Operation::CEILING`）
5. **两条 Rust 断言 + 四条可判定守卫**（Node 侧并入既有 `tests/registry.test.mjs`，
   `engineering_checks` **零改动**）：
   ① `pub const BRINGUP_OPERATION` 全仓**恰好 1 处**且在 `crates/nai-storage/src/lib.rs`；
   ② 它的值表达式 MUST **引用 `CEILING`**（不许写回字面量）；
   ③ `Operation::new(Duration::from_secs(30))` 字面量在全仓**恰好 0 处**；
   ④ `const OPERATION_CEILING`（私有影子）在全仓**恰好 0 处**。
   外加两条 Rust 断言：`nai-security` 的 `CEILING == 30s` / `new(CEILING)` 成功 / `new(CEILING+1ms)` 失败；
   `nai-storage` 的 `BRINGUP_OPERATION == Operation::CEILING`。

## 范围

只动预算的**定义位置**与**消费者引用**：**不改任何预算数值**、**不改任何行为**、不改基线正文、
不改 `acceptance` / registry / schema、不新增 xtask 步骤、**不新增依赖边**（只删副本）。

`nai-control-api` 的 `:617` / `:736` 两处 5 s 是**在已打开的 store 上读**（逐次调用）⇒ **不动**
（与 `apps/engine/src/pipeline.rs` 同型，W70 已裁定原样保留）。
`SHORT_OPERATION` **留在原地**：它没有第二个消费者，为不存在的问题造公共定义即「没人消费的契约」。
