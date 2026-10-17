# W134 — CI 上同一批测试被跑两遍

> 起点：上一轮梳理报告的 §5.2（「CI 分层」）。开工先量，量出来的不是「CI 慢」，而是**同一件事做了两遍**。

## 一、勘定

### 1.1 起因不是某条验收条目

`tests/registry.yaml` 本轮**未改**。W134 是一条工程债：CI 的 windows 腿 21–29 分钟，
每轮观测窗口被拖到 30 分钟以上。先怀疑「CI 太慢」，量完之后结论不同。

### 1.2 现测：门控步已经跑过全部 workspace 测试

```
xtask/src/lib.rs
  ["check", "--profile", "full"] => { engineering_checks(root)?; frontend::check(root)?; }
  fn engineering_checks(root) { …; run(root, &["test", "--workspace", "--locked", "--exclude", HARNESS])?; … }
  ["test", "--suite", "storage"] => run(root, &["test", "--locked", "--package", "nai-storage", "--package", "nai-security"])
  … 其余 6 条同形
```

7 条 `test --suite …` 的包集**全部**是 workspace 成员 ⇒ 上面那次 `--workspace` 已经把它们跑完了。

### 1.3 量出来的第二遍：517 秒/平台腿

本机暖缓存、**只计执行**（先由 `cargo build --workspace --all-targets` 预构建，再逐个计时）：

| 步 | 秒 |
|---|---|
| `test --suite storage` | 125 |
| `test --suite kernel` | 108 |
| `test --suite scheduler` | 89 |
| `test --suite session` | 77 |
| `test --suite hosts` | 43 |
| `test --suite knowledge` | 40 |
| `test --suite provider` | 35 |
| **合计** | **517** |

同一份日志里 `cargo test --workspace --locked --exclude nai-xtask` 计时 950s——那一次含 34 次重编译与
一次链接期占用（`LNK1104`，宿主瞬态），**不作为纯执行时间引用**；能当硬指标的只有上面 7 条（它们
在 workspace 运行之后，已无可编译项）。

### 1.4 为什么四层判据都没看见

| 判据 | 它管什么 | 对本缺陷 |
|---|---|---|
| W95 步集闭世界 | 这一步在不在册 | 沉默 |
| W97 覆盖闭包 | 门控与 suite 的包集合有没有**漏** | 沉默（漏才管，重不管） |
| W100 执行配置 | 会不会跑、能不能被吞 | 沉默 |
| W101 目标绑定 | 跑的是不是它自称的目标 | 沉默（目标确实对） |

四条都正确，四条都对**「两个目标覆盖的是不是同一批测试」**沉默 ⇒ 重复是它们**共同的盲区**。

### 1.5 一条拦住了「按平台分层」的既有规则

`step-config.mjs` 的 R3：`ci-only` 的成因若不是 `platform-toolchain`，该步 MUST NOT 被平台条件收窄
——否则它**静默跳过**另一个平台。本轮想做的「windows 只跑构建 + 平台测试」正好撞在这条上：
它不是配置问题，是**模型**问题（要改 R3 与 `tier-payment` 的 `ci` 面）。⇒ 本轮不做，具名登记。

## 二、交付

1. **删 7 条重复步**：`foundation.yml` 22 → 15 条步，`WORKFLOW_STEPS` 22 → 15 条（现测两侧相等）。
   保留 `contracts`（Linux C++ 夹具，门控**不**覆盖它）。
2. **新增判据** `scripts/contracts/step-overlap.mjs` + `tests/contracts/step-overlap.test.mjs`：
   同一 workflow 内两个判定步的**测试覆盖互相包含（含相等）** ⇒ 具名报出**两条步与各自包集**。
   包集 MUST 从 dispatch 臂与门控区域**算**（W93：同一件事 MUST NOT 两处定义）。
3. **覆盖类别闭集** `gate` / `suite` / `non-test-target`，域外取值具名失败（W90）。
   `suite` 类里「体是一条 `["test", …]`」与「体交给别的函数」MUST 分开：后者（`contracts`）返回
   `null` 而**不是空集**——空集会让包含关系对空集恒真。
4. **门控登记**：`engineering_checks` 加一行 `tests/contracts/step-overlap.test.mjs`。

**裁决口径**（门控覆盖是**并集**）：`engineering_checks` 有两条 `["test", …]`
（`--workspace --exclude <harness>` 与 `--package <harness> --lib`），只取第一条会**少报** harness
⇒ 一条「只跑 harness」的步会被判成不重复（假阴性，方向不安全）。

## 三、判据

### 3.1 判据清单（12 条）

基线干净 / 门控覆盖是并集 / 被门控覆盖的 suite 具名 / 门控不覆盖的不算重复 / 两条 suite 互不包含 /
域外类别具名 / 三类各有实例 / 缺输入具名 / 套件不可解析具名 / suite 包集本身 / 空判定集具名 /
门控区域没有 cargo test 具名。

**先写会红的判据**：对**未改动**的 workflow 跑 ⇒ **9 通过 / 1 红**，红的那条正是 7 条重复
（错误里逐条列出「某步的包集已被门控步覆盖」）。

### 3.2 注入验证（6 组，全部真红）

| # | 注入 | 结果 |
|---|---|---|
| D1 | 把 `test --suite storage` 步加回 workflow + 名册 | **1 红**，错误点名探针步 |
| D2 | `covers()` 恒假（重复检测失效） | **1 红**：`a suite whose packages the gate already runs is named` |
| D3 | 门控覆盖只取第一条命令（不取并集） | **1 红**：`the gate coverage is a union` |
| D4 | `coverageClassOf` 把所有目标归成 `non-test-target` | **7 红** |
| D5 | `suiteCoverageOf` 恒返回 `null` | **2 红** |
| D6 | 删掉「一条都没判到」的非空洞断言 | **1 红**：`an empty judged set is named` |

三文件 `sha256sum -c` **逐字节还原 OK**（注入基线是 fmt-clean 的最终字节）。

⚠️ **如实披露**：D2–D6 是**测试级**注入。D1 之外，模块级改动**无法**让门控整条变红——
**仓库里已经不存在重复**，「删掉重复检测」没有对象可变红。这是 W128 记过的**第二种成因**
（断言守的是由构造保证的事实），常驻覆盖落在单元用例上；MUST NOT 把它记作「门控会拦住」。

### 3.3 一个自造缺陷（当场修掉）

注释里写 `cargo` + `xtask` 连写，被 `gate-coverage.mjs` 的 `ciXtaskCommands` 当成一条真实 CI 命令
（它**不剥 YAML 注释**）⇒ 判据报「CI 调了不存在的命令」。改措辞避开；**该洞未修**，进具名登记。

## 四、门控

`cargo xtask check --profile full` **绿**（`EXIT=0`，6m48s，`LNK1104` **0** 次）：

- Rust **1085 / 0**（与 W133 相同——本轮**无** Rust 测试变动，逐项吻合）；
- Node **336 / 0**，14 个套件（W133 是 324 / 0、13 套件 ⇒ **+12 / +1**，正是新判据的 12 条）；
- `test result` 100 行 / `Running` 83 行 / `Doc-tests` 18 行（与 W133 同形；⚠️
  `100 < 83+18` 说明有一处目标未单独打印，是 cargo 既有形状 ⇒ **不作为逐轮对比指标**）；
- 错误类（`LNK1104` / `error[E…]` / `error:`）**0**；前端 7 步全通过。

### 4.1 前两次门控红——宿主瞬态，不是代码缺陷

| 次 | 失败点 | 形态 |
|---|---|---|
| 第 1 次 | `nai-runtime` (lib test) | `LNK1104` 打不开 `nai_runtime-*.exe` |
| 第 2 次 | `nai-control-api` (lib test) | `LNK1104` 打不开 `nai_control_api-*.exe` |

判据（四条，逐条现测）：①错误类同族且**全在链接期**；②失败目标**跨目标漂移**
（`nai-runtime` → `nai-control-api`）；③`git status crates/nai-runtime` 与 `crates/nai-control-api`
**为空**（本轮未改）；④单跑 `cargo test -p nai-runtime --lib` **2/2 通过**、`tasklist` 无残留测试进程。
⇒ 处置 = **串行预构建**（`cargo build --workspace --all-targets`，13m31s，`LNK1104` 0 次）后再跑门控
⇒ 一次通过。**未改任何代码与断言**。

## 五、具名登记（本轮**发现但不修**）

1. **windows 仍跑与 Linux 完全相同的门控**（前端 + fmt + clippy + 全量测试）。撤掉它要新增一个
   「平台无关 / 平台相关」分离的 profile，并同步改 `step-config.mjs` 的 R3 双向规则与 `tier-payment`
   的 `ci` 面——**模型变更**，MUST NOT 顺手做。这是 §5.2 里收益最大的一条，也是本轮**没有**落地的部分。
2. **归因粒度下降**：CI 上不再有「红色出现在哪个 suite 步」这一层，只能读门控日志
   （`ci-diagnose.mjs` 的注解仍给失败摘要）。如实登记，MUST NOT 说成「零损失」。
3. **`ciXtaskCommands` 不剥 YAML 注释**：注释里的 `cargo`+`xtask` 会变成一条「不存在的 CI 命令」。
   本轮只避开该形状，判据的洞未修。
4. **`cargo-suite` 类在名册里已无实例**：本轮之后所有 cargo 套件都不再有独立 CI 步
   （`upgrade` 套件本来就没有）。判据「两条步的覆盖不许互相包含」因此在本仓库上**只能靠单元注入
   变红**（见 §3.2 的如实披露）。
5. **`Running` 行数不可作逐轮对比指标**（`100 < 83+18`），硬指标只有 passed 总数与失败数。
6. **`target/` 已 18G**（超出常见 CI 缓存配额），故本轮**不加**编译缓存；缓存也不是「少做一遍」，
   与「重复」这一判据无关。
