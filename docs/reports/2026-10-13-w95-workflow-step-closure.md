# W95 轮次报告：CI 工作流的每个 step 必须具名在册

- 日期：2026-10-13
- 变更：`openspec/changes/archive/2026-10-13-w95-workflow-step-closure/`
- ADR：[0073](../adr/0073-workflow-step-closure.md)

## 一、母题

判定的执行入口有两个：本机 `cargo xtask check --profile full`（门控）与
`.github/workflows/foundation.yml`（CI）。**后者多跑一批判定步，而这个集合没有任何在册台账。**

本轮补的是一层**尚未覆盖的对象**：workflow 的 **step**。它与 W84（测试文件的可达性）、
W86（OpenAPI 文档集）同族——同一手法，新的一层。

裁定过程：核实三个候选的「有没有东西可改」后，用户选定 **A：CI 步 × 本机门控对账**。

## 二、核实（先测后设计）

### 2.1 缝存在——两个探针（改真实 workflow，`git checkout --` 还原，`sha256` 逐字校验）

| 探针 | 改动 | 本机门控（schema 30 / platform 22 / registry 52） |
| --- | --- | --- |
| P-加法 | 在 `pnpm spec:validate` 前插一个 step（`run: node -e "…"`） | **全绿、零信号** |
| P-减法 | 删掉 `Check generated contracts …`（`cargo xtask schema --check`） | **同样全绿**——一个判定**静默消失** |

### 2.2 无替代守卫

- `grep -rn "foundation.yml" crates/ xtask/` **为空**——编译侧不读 workflow。
- `test-reachability.mjs` 只从 workflow 里读**一个 marker**（`tests/contracts/descriptor.test.mjs`），
  其余 20 个 step 完全不在受判范围。
- 本轮文件层的闭世界（新增一份 workflow 文件）此前同样无人管。

### 2.3 步的真实集合（YAML 解析得出，21 个）

| 类别 | 条数 | 步 |
| --- | --- | --- |
| `infra` | 7 | checkout / setup-node / pnpm-action / rustup / pnpm install / protoc / apt-get |
| `local` | 8 | `check --profile full`（1）+ 7 个 `test --suite`（storage／kernel／hosts／session／scheduler／provider／knowledge） |
| `ci-only` | 5 | `schema --check`／含 `descriptor.test.mjs` 的契约步／`--suite contracts`／`spec:validate`／`openspec validate --archived` |
| `diagnostic` | 1 | `Publish the gate failure summary as annotations` |

两个**需要判断**的地方，都有实测依据：

1. **7 个 suite 步被本机覆盖**：本机 `engineering_checks` 跑的是
   `cargo test --workspace --locked --exclude HARNESS`（**全 workspace**），这 7 个 suite 是它的**子集**。
2. **步 17 是混合步**：`schema.test.mjs` 本机跑、`descriptor.test.mjs` 只有 CI 跑 ⇒ 归 `ci-only`（保守方向）。

## 三、另两个候选为何没选（实测，不是推测）

- **B（配置面 `[B]` 类死约束）**：可判，但要新造一条读者派生；价值限于「揭示哪些 `const` 是装饰」。
- **C（`needs_external` 21 条审计）**：W94 已逐条抽样 `closure`，绝大多数确实闭合在仓外
  （平台书面答复／商用授权／第三方规范／真机实测／市场项目约定）⇒ 预计无数据订正，只剩形状加固（弱）。
- **另实测排除一条**：生成物 drift 的 `differences()` **确实比对内容**（`Vec<u8>`），没有 W93 那种
  「只验路径」的缝；而本机跑不了它（`EBUSY`）——那正是本轮 A 要登记的一格。

## 四、设计与四处取舍

### 4.1 `local` 必须付代价

`kind: 'local'` 的每条带 `{file, marker}`，marker MUST 在该门控源里**逐字存在**：
step 08 → `["check", "--profile", "full"]`；7 个 suite → `["test", "--workspace", "--locked", "--exclude", HARNESS]`。
⇒ 「本机也跑」从一句话变成一个**可判定**的引用（W75「已声明 ≠ 已执行」）。

### 4.2 `ci-only` 必须付代价，且**双向**

成因 ∈ 闭集四格：`host-subprocess`／`compiled-artifact`／`platform-toolchain`／`not-in-profile`。
域外成因 ⇒ 具名失败（理由与 `UNWIRED_CAUSES` 同形：一个未知成因是一个标签，不是一个理由）。
且 `ci-only` MUST NOT 同时挂 `localEvidence`——否则「它是不是本机也跑」没有单一回答。

### 4.3 `infra` MUST NOT 是绕过检查的后门

`infra` 的 `run` MUST NOT 命中 `JUDGE_COMMAND_SHAPES`（`cargo xtask`／`node --test`／
`pnpm exec`／`pnpm <非 install>`）。否则「把判定步骤标成环境准备」就能绕开整套检查（**自我满足**）。

### 4.4 `diagnostic` 由**结构**认出，不由名字认出

`diagnostic` 的 step 其 `if` MUST 含 `failure`。实测反例：`if: runner.os == 'Linux'` 是平台条件，
过不了这条。

**贯穿的一条**：分类是**静态声明**，MUST NOT 由运行时探测得出（铁律二）——`EBUSY` 是宿主事实，
换一台机器会对同一个仓库得出不同分类；且「能跑但没接进门控」与「根本跑不了」风险不同、成因不同。

## 五、与 `test-reachability` 的边界（实测）+ 一处既有清单的合法改动

两张表都以 `foundation.yml` 为 host，但**断言单元不同**。实测（改真实表 → 跑两个套件 → 还原）：

| 注入 | `workflow.test.mjs` | `registry.test.mjs` |
| --- | --- | --- |
| 删掉 `test-reachability` 的站点 | **0 红** | **2 红** |
| 删掉一条 `WORKFLOW_STEPS` 登记 | **5 红** | **0 红** |

⇒ 二者**互不顶替**：前者管「workflow 的 step」，后者管「测试文件的可达性」。

**本轮合法地改了既有清单**（该模式第三次出现）：`registry.test.mjs` 里那张**逐字写死的站点 id 列表**
（7 个）在新增测试文件后**如期变红**——这正是它的设计意图。处置是把 `xtask-engineering-workflow`
加进去（7 → 8），不是放宽断言。同一时刻 W84 的闭世界也报了
「site `xtask-engineering-workflow` matches no tracked file」（因为新测试文件尚未进 `.git/index`）
——两条断言**各自**独立地发现了同一件事，随后 `git add` 后同时转绿。

## 六、门控与计数

### 6.1 变更校验

| 项 | 结果 |
| --- | --- |
| `openspec validate w95-workflow-step-closure --strict` | **valid** |
| `openspec validate --all --strict`（归档前，含活动变更） | **44 passed / 0 failed**（44 items） |
| `openspec validate --all --strict`（归档后） | 见 §6.3 |

### 6.2 全量门控

| 项 | 结果 |
| --- | --- |
| `cargo xtask check --profile full` | **EXIT=0**（5 分 16 秒） |
| Rust 目标 / 用例 | **91 目标 / 862 passed / 0 failed** |
| `LNK1104`（宿主瞬态） | **0**；预链接**第 1 轮**即收敛（无重试痕迹） |
| `cargo fmt --all --check` | **EXIT=0** |
| Node 套件（6 个） | registry **52** / schema **30** / platform **22** / **workflow 13（本轮新增）** / desktop **5** / ui **52** —— 全绿 |

- Rust 目标数与用例数与 W94 **逐项相同**（91 / 862）：本轮 `xtask/src/lib.rs` 只加了**一行登记**
  （零行为改动），新增的 13 个用例住在 **Node 侧**，不进 Rust 的 `test result` 计数。
- Node 套件由 **5 个 → 6 个**（新增 `tests/contracts/workflow.test.mjs`）。

### 6.3 两条 validate

| 项 | 结果 |
| --- | --- |
| `openspec validate --all --strict`（归档后，不含活动变更） | **43 passed / 0 failed** |
| `openspec validate --all --strict --archived`（含已归档变更） | **93 passed / 0 failed**（W94 为 92） |
| 归档结果 | `engineering-foundation: + 1 added, ~ 0, - 0`；**Task status ✓ Complete** |

## 七、CI

<!-- CI_PLACEHOLDER -->

## 八、遗留

1. **不验 `ci-only` 的成因是否仍然成立**：`schema --check` 哪天在本机变得可跑，判据不会有信号。
2. **`infra` 的形状是启发式**：新判定命令形状（例如 `deno test`）若未被 `JUDGE_COMMAND_SHAPES`
   覆盖且被标成 `infra` ⇒ **假阴性方向**。正确修法是扩形状表，MUST NOT 把该步直接标成 `infra` 了事。
3. **步的版本号进身份**：`actions/checkout@v4` → `@v5` 会让判据红（要求显式处置）——有意，但需维护。
4. **四类都非空是被断言的**：某一类被合法清空会红 ⇒ 要求显式处置（W92 同族：空态 MUST 被显式断言）。
5. **W90 §八遗留 1／3／4／5、W91 的跨语言键级消费、W92 的基线收录、W93 的锚点唯一性、
   W94 的 `needs_external` 审计**均**未动**。

## 九、如实披露

1. **不验「CI 真的跑了某步」**——那要读运行结果，不是静态判据的事。本报告不断言 CI 侧的执行结果，
   除非 §七 有运行级证据。
2. **不验 `ci-only` 的成因是否成立**——语义，仍由人读（与 ADR 0054 决策 2 同族）。
3. **不验 `local` 的落点真的覆盖了这个 step**——验的是「那处落点存在」。把已覆盖的报成未覆盖是
   **假阳性**（安全方向）；反方向（假阴性）不可能由这条产生，因为 `local` 只是主张，主张错了要人改。
4. **`infra` 的形状检查验的是命令形状，不是语义**——把环境准备误报成判定会红，属安全方向。
5. **本轮不改 `foundation.yml` 一个字符**：补的是判据与登记，不动被审对象。
