## Why

`ci-only`（CI workflow 步）与 `standalone`（`cargo xtask` 命令）两类条目，各自 MUST 给出一个**成因**，
且成因 MUST 取自一个闭集（`CI_ONLY_REASONS` / `STANDALONE_REASONS`，W95 / W96 建立）。
判据验的是「成因在闭集里」——**闭集之外**是一个标签，不是一个理由。

但闭集**之内**的每一个成因，今天是一句**无从查证的话**：它们只写在两处常量里，
判据（`workflowStepErrors` / `commandErrors`）对它们只做一次 `includes()`。于是有两处空位：

1. **成因本身没有任何仓库内的凭据。** `superseded-by-workspace-run` 断言「门控跑的是全 workspace
   测试，覆盖同一批包」——这句话若哪天不再成立（那条 `--workspace` 调用被换成显式包列表），
   成因格照旧成立、判据照旧全绿。
2. **成因格没有任何「至少有一个实例」的断言。** 两处测试都只宣布闭集**非空**：
   `assert.ok(CI_ONLY_REASONS.length > 0, 'the cause table must not be empty')`。
   某一格若在某轮被**合法清空**（它的条目改归别因），格子仍在常量里，它的规则对空集恒真。

这正是本仓反复出现的母题（W78 的规则表、W95 的 `CONDITION_SHAPES`）：**规则表没有实例 ⇒ 规则恒真**。

### 本轮的直接触发：一条被预言的失效刚刚发生

ADR 0073（W95）的「如实披露」第 2 条逐字写着：

> **不验 `ci-only` 的成因是否仍然成立**：例如 `schema --check` 哪天在本机变得可跑，判据不会有信号
> （`reason` 仍写着 `host-subprocess`）。要发现它得靠人重新实测宿主。

2026-10-14 实测：`cargo xtask schema --check` 在宿主机**跑通了**（退出码 `0`，
`Rust/TS/C++/JSON Schema/OpenAPI generation checked with no drift`），而注册表里那条
`schema --check` 的 `reason` **仍旧逐字写着 `host-subprocess`** ⇒ 判据**零信号**。

一条**会随仓库自身进展而过期**的声明，没有任何判据盯着它。这是**遗忘**的形状，不是一条待办。

### 但处置 MUST NOT 是「宿主现在能跑了，所以成因不成立」

「宿主能不能派生子进程」是**宿主/机器事实**。铁律二：**宿主瞬态 MUST NOT 改变被测系统的结论**——
换一台机器会对同一个仓库得出不同分类 ⇒ 判据不再可复现。ADR 0078 决策 5 已把 `host-subprocess`
明文归入「与 runner 的 OS 无关」那一侧，正是这个意思。

⇒ 正确的问题不是「它今天成立吗」，而是「**它能不能从仓库里自证**」。能 ⇒ 配凭据；
不能 ⇒ 它作为「在册成因里的一格」是一个**永久无法兑付的空格**，必须逐出。

## What Changes

- **新增成因兑付规格表**，两处各一张（成因闭集本就不同，不是同一件事的第二处定义）：
  - `scripts/contracts/xtask-commands.mjs`：`STANDALONE_REASON_PAYMENTS`
  - `scripts/contracts/workflow-steps.mjs`：`CI_ONLY_REASON_PAYMENTS`

  每格一条规格 `{evidence: [{file, marker, why}, …]}`，**每条凭据 MUST 逐字存在**。

- **新增判据（6 条）**，两个校验函数各自加载：
  1. **双向**：兑付表的键集 MUST **恰等于**成因闭集（两个方向各报一次：**欠付**＝有人声明没人付、
     **空规格**＝有人付没对象）。
  2. **凭据逐字存在**：每条凭据的 `marker` MUST 存在于 `file` 的内容里。
  3. **凭据是仓库成员**：`file` MUST 已被 git 跟踪（`target/` 下的产物、临时文件不算仓库事实）。
  4. **凭据不得自引**：`file` MUST NOT 落在被审注册表自身所在的目录（`scripts/contracts/`）。
  5. **格非空洞**：每个成因取值 MUST ≥1 在册条目。
  6. `why` MUST 非空：把「这条凭据回答了什么」写到明面上，供人审。

  ⚠️ **刻意不加**「逐条遍历在册条目的成因、必须有规格」这一条：既有断言已保证
  `entry.reason ∈ 成因闭集`，加上第 1 条的「键集 == 闭集」⇒ `entry.reason ∈ 键集` 是**纯集合论蕴含**。
  **能由结构蕴含的 MUST NOT 再具名一次**——多写一条不会多一个信号，只会多一处将来会漂移的定义。

- **逐出 `host-subprocess`**（两处闭集 4 → 3）。理由：**它不可能有凭据**——它是宿主/机器事实，
  而宿主瞬态 MUST NOT 成为判定输入（铁律二）⇒ 它是两处闭集里唯一一个**永久无法兑付的空格**。
  ⚠️ 逐出的理由**不引用任何发生过的瞬态**：本轮的实测只暴露了这个空格，MUST NOT 被写成理由。

- **重分类 `schema --check` 与它对应的 CI 步**（两条 `host-subprocess` 实例的去处）：
  - 命令层 `schema --check` ⇒ `requires-compiled-artifact`
  - step 层 `Check generated contracts without modifying consumers` ⇒ `compiled-artifact`

  理由：该命令依赖 **`protoc`**（`xtask/src/schema.rs` 在生成前**硬校验** `libprotoc 3.21.12`），
  超出 portable 门控「只需 Rust 工具链」的既有前提（ADR 0041 / ADR 0043）
  ⇒ 它 MUST NOT 并入 `engineering_checks`，与 `test --suite contracts` 同格。
  这个理由**不随宿主变化**：本机装了 protoc，不代表 portable 门控可以依赖 protoc。

- **MODIFIED 一条既有需求**：把 `ci-only` 成因与平台条件那一句里的**计数**去掉
  （「成因是其余**三因**之一」→「成因不是 `platform-toolchain`」）。
  计数是易漂移的，MUST NOT 进活文档；去计数之后，本起因闭集收缩而需要的订正**一次性**完成。

- **不新增**判据模块、**不改**任何 Rust 源码、**不改**任何契约生成物、**不改** `xtask/src/lib.rs`。

### 明确不做：判据验的是「凭据存在」，不是「凭据与成因语义一致」

凭据回答的是「这条成因凭什么成立」，那是**人的判断**。判据只能验「这条凭据在仓库里逐字存在」，
**MUST NOT** 被读作「凭据一旦被破坏，该成因就不再成立」的充分性证明。后者是三合一的
（仓库事实 + 跨仓库契约 + 触发方式），本轮的兑付面**只覆盖第一项**，如实披露在报告与 ADR 里。

## Modified Capabilities

- `engineering-foundation`：修改「Every registered workflow step's execution configuration is paid
  for by a parsed platform set」一条——去掉成因侧的平台计数，改为按**类别**（是不是
  `platform-toolchain`）分派。**不改**另外两条同族需求的行为：它们各自的「成因取自闭集」与
  「成因 MUST 兑付」由本轮新增的 ADDED 需求承担。

## Impact

- 判定面：`scripts/contracts/xtask-commands.mjs`、`scripts/contracts/workflow-steps.mjs`
  （闭集收缩 + 兑付表 + 核验逻辑 + 注释订正）。
- 注册表：上述两个 `.mjs` 各改**两条**条目的 `reason`（`schema --check`、
  `Check generated contracts without modifying consumers`）。**其余 43 条一字不改**。
- 测试：`tests/contracts/xtask-commands.test.mjs`（`:187` 的非空洞断言、`:304-309` 的逐字钉、
  以及全部调用点的输入面扩张）、`tests/contracts/workflow.test.mjs`（`:147` 的同族两处、调用点）。
  **既有用例逐项不变**（扩面 MUST 带回归断言）。
- 规格：`openspec/specs/engineering-foundation/spec.md` 一处 MODIFIED（1:1 行替换去计数）。
- 文档：新增 `docs/adr/0086-cause-payment.md`；ADR 0073 / 0078 **追加勘误段**（原文保留不改写）；
  `AGENTS.md` / `docs/development-plan.md` 里已被本轮改写的过期陈述判载体后就地订正；
  `docs/reports/2026-10-14-w108-cause-payment.md`。
- 门控：**不新增独立 Node 测试文件** ⇒ 无需在 `engineering_checks` 登记新站点；
  两条测试走既有 `cargo xtask check --profile full`。
- **明确不做**：不改任何 Rust 源码；不改 `xtask/src/lib.rs` 的 dispatch / usage / 门控闭包；
  不改 `schemas/**` 与任何生成物；不改 `migration::VERSION`；不改任何验收 `status`；
  不改 `step-config.mjs` 的分派（它已经按闭集 `includes()` 分派，闭集收缩**自动**正确）。
