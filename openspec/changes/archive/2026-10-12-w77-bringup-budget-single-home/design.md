# 设计（w77：bring-up 预算的单一归属点）

## 0. 核实结论先行

**问题**：`crates/nai-control-api/src/tests.rs::seed_approval_store`（`:349-353`）自造
`Operation::new(Duration::from_secs(10))` 去跑 `Store::open` —— 那是 **bring-up 工作量**
（全新数据根 + 全链迁移），**不是**逐次调用的工作量。W75 在 CI 上的失败正落在该文件 `:364`。

**要修它，先要回答「那份唯一的定义从哪儿拿」。** 核实五条事实：

| 事实 | 证据 |
| --- | --- |
| `nai-runtime` **依赖** `nai-control-api` | `crates/nai-runtime/Cargo.toml:17` |
| ⇒ 反向引用构成**依赖环**，「引 runtime」**不可行** | 同上 |
| `nai-control-api` 与 `nai-runtime` **都**依赖 `nai-storage` | 两者 `Cargo.toml` |
| `BRINGUP_OPERATION` 目前只定义在 `nai-runtime::composition` | `composition.rs:31` |
| `Operation` 的 30 s 上界是 `nai-security/src/lib.rs:36` 的**字面量** | grep |

**W70 立这条常量时的注释写的是**：

> Sized here once so the engine **cannot hold a second, drifting copy**.

⇒ 规则本身没错，**落点错了**：它被放在了**首个遇到问题的消费者**（runtime 的 bring-up 路径），
而 bring-up 的**执行者**是 `Store::open`，在 `nai-storage`。

**现存副本清单**（同一族）：

| 位置 | 当前写法 | 工作量 | 处置 |
| --- | --- | --- | --- |
| `nai-storage/src/tests.rs:10` | `from_secs(30)` 字面量 | bring-up | 改用常量（**定义所在的 crate 自己也有副本**） |
| `nai-storage/src/upgrade.rs:183` | `from_secs(30)` 字面量 | 迁移 | 改用常量 |
| `nai-control-api/src/tests.rs:351` | `from_secs(10)` 字面量 | bring-up | 改用常量（**预算与工作量不匹配**） |
| `nai-upgrade-core/tests/{plan,migration,lifecycle,gates}.rs` | `from_secs(30)` 字面量 | 演练 | 改用 `Operation::CEILING` |
| `nai-provider-core/examples/openai_smoke.rs:74,108` | `from_secs(30)` 字面量 | 冒烟 | 改用 `Operation::CEILING` |
| `nai-provider-core/examples/live_provider_smoke.rs:111` | **`const OPERATION_CEILING` 私有影子** | 冒烟 | **删影子**，4 处引用改指 `Operation::CEILING` |
| `xtask/examples/golden_eval.rs:52` | **`const OPERATION_CEILING` 私有影子** | 评测 | **删影子**，2 处引用改指 `Operation::CEILING` |

合计 **11 个位点 / 9 个文件**。**最后三行由实现期的全仓扫描发现**（设计初稿只列了前四类）——
它们各自**重新定义了一个同名常量**，正是 `composition.rs` 那句注释所禁止的
*"a second, drifting copy"* 的**活标本**。这只是清单的**扩展**，不改变上面任何一条裁定。

**注意区分**：`nai-upgrade-core` 的 `tests/plan.rs:115`、`migration.rs:27`、`upgrade.rs:22`、
`gates.rs:103` 里的 `Instant::now() + Duration::from_secs(30)` 是**自造 deadline**，不是
`Operation::new` 的实参 ⇒ 它们**不在**本轮守卫 G-C② 的命中范围内，也**不**强求改动
（改它们会扩大范围且不解决「预算定义点」这个命题）。

## 1. 决策：定义点归属「工作类别的执行者」

判据问题不是「谁最先遇到问题」，而是**「这份预算描述的是谁的工作」**：

| 工作类别 | 执行者 | 定义点 | 判定 |
| --- | --- | --- | --- |
| bring-up（全新数据根 + 全链迁移） | `nai_storage::Store::open` | **`nai-storage`** | ✓ 本轮下移 |
| `Operation` 的**上界**（拒绝阈值） | `nai_security::Operation::new` | **`nai-security`** | ✓ 本轮提取 |
| 逐次调用（`SHORT_OPERATION` 5 s） | 只有 `nai-runtime` 一个消费者 | **留原地** | ✓ 不预造公共定义 |

**为什么 `SHORT_OPERATION` 不上移**：它**没有第二个消费者**。为不存在的问题造公共定义，
正是 W73 记下的「没人消费的契约会变假契约」。**等出现第二个消费者再移。**

## 2. 决策：`Operation::CEILING` 让「取上界」可判定

原状：`BRINGUP_OPERATION = Duration::from_secs(30)`，而 `Operation::new` 拒绝 `> from_secs(30)`。
「bring-up 取 ADR 0004 上界」这件事**只写在注释里**，机器看不见 —— **数值恰好相等是巧合还是设计，
无法判定**。

改后：`Operation::CEILING`（public const）＝ 全仓唯一的 30 s 来源；`new()` 用它做拒绝阈值；
`BRINGUP_OPERATION = Operation::CEILING`。⇒ **「取上界」变成一个可判定的等式**。

**为什么不只把 `from_secs(30)` 写两遍**：那正是注释里那句承诺要禁止的「second, drifting copy」
—— 两个 30 改一个忘一个，注释里的承诺就失效了。

## 3. 两条 Rust 断言 + 四条仓库级扫描（非空洞）

| 守卫 | 形状 | 非空洞性（注入后必须失败） |
| --- | --- | --- |
| G-A `nai-security` | `CEILING == 30s`；`new(CEILING)` 成功；`new(CEILING + 1ms)` 失败 | 把 `CEILING` 改成 20 s ⇒ 第 1 条失败 |
| G-B `nai-storage` | `BRINGUP_OPERATION == Operation::CEILING` | 把值写成字面量 20 s ⇒ 失败 |
| G-C① Node 扫描 | `pub const BRINGUP_OPERATION` 全仓**恰 1 处**、在 `crates/nai-storage/src/lib.rs` | 加第二处 ⇒ 失败；删掉定义 ⇒ 失败 |
| G-C② Node 扫描 | `Operation::new(Duration::from_secs(30))` 全仓**恰 0 处** | 任一处改回字面量 ⇒ 失败 |
| G-C③ Node 扫描 | 定义行**含 `CEILING`** | 写成 `from_secs(30)` ⇒ 失败 |
| G-C④ Node 扫描 | `const OPERATION_CEILING`（私有影子）全仓**恰 0 处** | 任一处再起影子常量 ⇒ 失败 |

**G-A 的第一条为什么必须写死 30 s**：只用 `new(CEILING).is_ok()` 是**自指**的 —— `CEILING` 被改小后
它照样通过（拒绝阈值跟着变小）。外部锚点是 **ADR 0004 的 30 s**，这正是 W66 教训：「照抄某张表的
常量，必须另有一条把表逐项写死的断言」。

## 4. 扫描守卫的两条纪律（W73/W74 教训的直接应用）

- **去掉整行注释**：注释里出现一句 `BRINGUP_OPERATION` 不该算「定义」或「命中」。
  仅识别整行注释（`//` / `///` / `//!` 开头，以及块注释的续行）；**不**做行尾注释解析 —— 行尾
  注释仍算命中只是**多报**（方向安全），而误切会伤到 `https://` 这类内容。
- **本守卫不会读到自己**：判据在 `.mjs` 里，扫的是 `.rs` ⇒ 不存在自我满足（W73 的教训）。

## 5. 验证命令与环境边界

```
cargo fmt --all
cargo test -p nai-security -p nai-storage -p nai-runtime -p nai-control-api -p nai-upgrade-core
node --test tests/registry.test.mjs
cargo xtask check --profile full
```

- 纯静态改动 + 既有测试 ⇒ **不需要 protoc**；无平台分支 ⇒ Windows 与 Linux 行为一致。
- `cargo xtask schema --check` 本机仍跑不了（宿主对「node 自旋 node」的 `EBUSY`）⇒ 零漂移由
  「未改任何 schema 与生成器输入」＋ CI 的一个独立步骤兜底，**不写成「已通过」**。

## 6. 本设计明确不做

- **不改任何预算数值**（30 s 还是 30 s，5 s 还是 5 s）—— 这是**归属订正**，不是调参。
- **不改 `nai-control-api` 的 `:617`/`:736` 两处 5 s**：那是在**已打开的 store 上读**（逐次调用），
  5 s 正确（与 `apps/engine/src/pipeline.rs` 同型，W70 已裁定原样保留）。
- **不引依赖边**：`nai-control-api` 不新增 `nai-runtime`（会成环）；`nai-upgrade-core` 沿用既有依赖。
- **不上移 `SHORT_OPERATION`**：见 §1。
- **不改基线正文**；**不新增 xtask 步骤**（守卫并入既有 Node 测试文件）。
