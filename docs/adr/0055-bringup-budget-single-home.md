# ADR 0055：一个预算一份定义——定义归属执行者，而非首个受害者

- 状态：已接受
- 日期：2026-10-12
- 依据：ADR 0027（一个预算一份定义）、ADR 0004（`Operation` 上界 30 s）、
  ADR 0051（第三入参必需——恒真的断言比没有更坏）、W70（产品侧 bring-up 期限）、
  W76（夹具预算，第二个消费者）、W75（CI 失败落点）
- 影响：`nai-security` / `nai-storage` / `nai-runtime` / `nai-control-api` /
  `nai-upgrade-core` / `nai-provider-core` / `xtask` 的**预算定义位置与引用**；
  **不改变任何数值、不改变任何行为、不新增任何依赖边**

## 背景

`BRINGUP_OPERATION`（30 s）由 W70 立在 `crates/nai-runtime/src/composition.rs`，注释写下：

> Sized here once so the engine **cannot hold a second, drifting copy**.

W76 在**第二个消费者**（`crates/nai-runtime/tests/common/mod.rs`）上修好过一个副本。
本轮遇到这个问题结构的**结构性版本**：`crates/nai-control-api/src/tests.rs::seed_approval_store`
自造 `Operation::new(Duration::from_secs(10))` 去跑 `Store::open`（全新数据根 + 全链迁移）——
**预算与工作量不匹配**，而 **W75 在 CI 上的失败正落在该文件 `:364`**。

要修它，必须先回答「唯一的那份定义从哪儿拿」。核实结果否掉了最直觉的答案：

| 事实 | 证据 | 后果 |
| --- | --- | --- |
| `nai-runtime` **依赖** `nai-control-api` | `crates/nai-runtime/Cargo.toml:17` | 反向引用构成**依赖环** |
| `nai-control-api` 与 `nai-runtime` 都依赖 `nai-storage` | 两者 `Cargo.toml` | `nai-storage` 是共同可达点 |
| `BRINGUP_OPERATION` 只定义在 `nai-runtime` | `composition.rs:31`（W70 时点） | shared 层拿不到 |
| `Operation` 的 30 s 上界是**字面量** | `nai-security/src/lib.rs:36`（W77 时点） | 「取上界」不可判定 |

⇒ **规则没错，落点错了**。它被放在**首个遇到问题的消费者**（runtime 的 bring-up 路径）上，
而 bring-up 的**执行者**是 `Store::open`，在 `nai-storage`。

## 决策

### 决策 1：定义点归属「工作类别的执行者」

判据问题不是「谁最先遇到问题」，而是**「这份预算描述的是谁的工作」**：

| 工作类别 | 执行者 | 定义点 | 处置 |
| --- | --- | --- | --- |
| bring-up（全新数据根 + 全链迁移） | `nai_storage::Store::open` | **`nai-storage`** | 本轮下移 |
| `Operation` 的上界（拒绝阈值） | `nai_security::Operation::new` | **`nai-security`** | 本轮提取 |
| 逐次调用（`SHORT_OPERATION` 5 s） | 只有 `nai-runtime` 一个消费者 | `nai-runtime` | **留原地** |

`BRINGUP_OPERATION` 因此下移到 `nai-storage`；`nai-runtime` 改为
`pub use nai_storage::BRINGUP_OPERATION;` ⇒ **`nai_runtime::BRINGUP_OPERATION` 仍然可用**，
`apps/engine` 无需改动（这是一次**加法**，不是破坏性迁移）。

### 决策 2：`Operation::CEILING` 让「取上界」可判定

原状里「bring-up 取 ADR 0004 上界」**只写在注释里**：`BRINGUP_OPERATION = from_secs(30)`
与 `new()` 里的 `timeout > from_secs(30)` **数值恰好相等是巧合还是设计，机器看不见**。

现在 `Operation::CEILING` 是**公开常量**、同时是 `new()` 的拒绝阈值，
`BRINGUP_OPERATION = Operation::CEILING` ⇒ **「取上界」成为一个可判定的等式**，
而不是两处各自写下 30 的默契。

### 决策 3：只有出现第二个消费者，才提取公共定义

`SHORT_OPERATION`（5 s）**留在 `nai-runtime`**：它**只有一个消费者**。
为不存在的问题造公共定义，就是 W73 记下的「没人消费的契约会变假契约」。
**下移它没有证据支持，所以不下移。**

### 决策 4：同一个数值可以有两个语义名，但每个语义只有一个定义

`BRINGUP_OPERATION` 与 `Operation::CEILING` 数值相同、**语义不同**：
前者是「bring-up 这类工作的预算」，后者是「`Operation` 的上界」。因此：

- `Store::open` 的 bring-up ⇒ **`BRINGUP_OPERATION`**；
- 一次演练 / 一次冒烟「用满上界」⇒ **`Operation::CEILING`**。

两者都**不许写回字面量**。这条把「数值相同」与「语义相同」分了开来——否则
`nai-upgrade-core` 的演练夹具会被迫自称 bring-up，那是**语义错位**，比重复更坏。

### 决策 5：两条 Rust 断言 + 四条仓库级扫描

| 判据 | 形状 | 位置 |
| --- | --- | --- |
| G-A | `CEILING == 30s`；`new(CEILING)` 成功；`new(CEILING + 1ms)` 失败 | `nai-security` 单测 |
| G-B | `BRINGUP_OPERATION == Operation::CEILING` | `nai-storage` 单测 |
| G-C① | `pub const BRINGUP_OPERATION` 全仓**恰 1 处**、在 `crates/nai-storage/src/lib.rs` | `tests/registry.test.mjs` |
| G-C② | `Operation::new(Duration::from_secs(30))` 字面量全仓**恰 0 处** | 同上 |
| G-C③ | 定义行 MUST 引用 `CEILING` | 同上 |
| G-C④ | `const OPERATION_CEILING`（私有影子）全仓**恰 0 处** | 同上 |

**G-A 的第一条为什么必须写死 30 s**：只用 `new(CEILING).is_ok()` 是**自指**的——
`CEILING` 改小后拒绝阈值跟着变小，检查照样通过。外部锚点是 **ADR 0004 的 30 s**，
这正是 W66 教训：「照抄某张表的常量，必须另有一条把表逐项写死的断言」。

**扫描判据读 Rust 源码、不读自己**（判据在 `.mjs`，被扫的是 `.rs`）——
W73 的消费守卫正是把自己文件里的字面量算成「消费」而**自我满足**。
**整行注释被剔除**：注释里提一句常量名，既不是定义它也不是使用它。

## 后果

- **`nai-control-api` 不再自造 10 s**：W75 那类 CI 失败的结构性成因被移除。
- **删掉两处 `const OPERATION_CEILING` 私有影子**（`crates/nai-provider-core/examples/live_provider_smoke.rs`、
  `xtask/examples/golden_eval.rs`）—— 它们是注释里那句 "second, drifting copy" 的**活标本**。
- **共 14 个文件**改动，**零数值变化、零行为变化、零新增依赖边**（只删副本、只加常量与引用）。
- **不预造契约**：`SHORT_OPERATION` 不上移；不新增 schema、不新增 xtask 步骤
  （判据并入既有 Node 测试文件 ⇒ 随 `check --registry` 一起跑）。

## 实施

```
cargo fmt --all
cargo test -p nai-security -p nai-storage -p nai-runtime -p nai-control-api -p nai-upgrade-core
node --test tests/registry.test.mjs
cargo xtask check --profile full
```

`cargo xtask schema --check` 本机仍跑不了（宿主对「node 自旋 node」的 `EBUSY`）；
零漂移由「未改任何 schema 与生成器输入」＋ CI 的一个独立步骤兜底，**不写成「已通过」**。

## 遗留

1. **判据的边界如实登记**：G-C 只抓**两个具名字面量/具名模式**。若将来有人新起一个
   叫别的名字的 30 s 常量（如 `const SMOKE_TIMEOUT`），扫描**抓不到**。它挡的是
   「已知的那两种复制方式」，不是「一切复制」——**这是判据的边界，不是缺陷**。
2. `nai-upgrade-core/tests/{plan,migration,upgrade,gates}.rs` 里的
   `Instant::now() + Duration::from_secs(30)`（**自造 deadline**）**未改**：
   它们不是 `Operation::new` 的实参，不属本命题。
3. `apps/engine/src/pipeline.rs` 与 `nai-control-api/src/tests.rs:617/736` 的 5 s 是
   **在已打开的 store 上读**（逐次调用）⇒ 原样保留（W70 已裁定）。
4. 若将来 bring-up 预算需要按平台或场景分化，**MUST** 在 `nai-storage` 扩展，
   MUST NOT 在消费者处新起一份。
