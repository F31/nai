# ADR 0073：CI 工作流的每个 step 必须具名在册 —— 判定的消失不能没有信号

- 日期：2026-10-13（W95）
- 状态：已接受
- 相关：ADR 0063（OpenAPI 操作闭世界）、ADR 0064（OpenAPI 文档集闭世界）、W84（测试文件可达性闭世界）、
  ADR 0051（决策 1：重叠处逐字一致）、ADR 0054（决策 2：语义留给人读）

## 背景

判定的执行入口有两个：本机 `cargo xtask check --profile full`（门控）与
`.github/workflows/foundation.yml`（CI）。**后者多跑一批判定步**，而**这个集合没有任何在册台账**：

```
[07] cargo xtask schema --check                                       本机 EBUSY
[10] cargo xtask test --suite storage                                 含 Windows-only 的 native vault
[11] cargo xtask test --suite kernel
[12] cargo xtask test --suite hosts
[13] cargo xtask test --suite session
[14] cargo xtask test --suite scheduler
[15] cargo xtask test --suite provider
[16] cargo xtask test --suite knowledge
[17] node --test tests/contracts/schema.test.mjs tests/contracts/descriptor.test.mjs
                                                                      descriptor 只有 CI 跑（要编译产物）
[18] cargo xtask test --suite contracts   (if: runner.os == 'Linux')   需要 Linux C++ 工具链
[19] pnpm spec:validate
[20] pnpm exec openspec validate --archived --no-interactive
```

这份知识**只存在于注释里**：`tests/contracts/schema.test.mjs` 里那句
「**本地门控从不读它**（唯一读它的 `generate.mjs` 只在 CI-only 的 `cargo xtask schema --check` 里跑，
而本机对该步 `EBUSY`）」是一句**解释**，不是判据。

## 证据

两个探针（改真实 workflow → 跑本机 3 套件 → `git checkout --` 还原 → `sha256` 逐字校验）：

| 探针 | 改动 | 结果（schema 30 / platform 22 / registry 52） |
| --- | --- | --- |
| P-加法 | 在 `pnpm spec:validate` 前插一个 step（`run: node -e "…"`） | **全绿、零信号** |
| P-减法 | 删掉 `Check generated contracts …`（`cargo xtask schema --check`） | **同样全绿** —— 一个判定**静默消失** |

增补核实：

1. **无替代守卫**：`grep -rn "foundation.yml" crates/ xtask/` **为空**——编译侧不读 workflow。
2. **`test-reachability` 不覆盖它**：它只从 workflow 里读**一个 marker**
   （`tests/contracts/descriptor.test.mjs`），其余 step 完全不在受判范围。
3. **步的真实集合**：21 个 step（7 `infra` / 8 `local` / 5 `ci-only` / 1 `diagnostic`），
   逐条由 YAML 解析得出（有 `name` 用 `name`，无 `name` 用 `uses` 或 `run` 首行）。
4. **步 17 是混合步**：`schema.test.mjs` 本机跑，`descriptor.test.mjs` 只有 CI 跑。
   只要有**未被本机覆盖的成分**就归 `ci-only`——保守方向。
5. **7 个 suite 步其实被本机覆盖**：本机 `engineering_checks` 跑的是
   `cargo test --workspace --locked --exclude HARNESS`（**全 workspace**），
   这 7 个 suite 是它的**子集** ⇒ 它们是 `local`，不是 `ci-only`。
   （`--suite contracts` 例外：它跑 `descriptor.test.mjs`，要 protoc + C++ 工具链 ⇒ `ci-only`。）

## 决定

### 决定 1：两层闭世界——文件层与步层

- **文件层**：`.github/workflows/` 下**已跟踪**的 workflow 文件 MUST 逐份在册，在册的每份 MUST 已被跟踪。
- **步层**：每份 workflow 的**每一个 step** MUST 有且只有一条登记，步的**身份**由解析得出且 MUST 唯一。

⇒ 新增一个 step、删掉一个判定步、改一个名、加一份 workflow 文件，都会**具名失败**。

### 决定 2：分类是**静态声明**，MUST NOT 由运行时探测得出

「这一步本机能不能跑」不由判据试跑探测。理由：`EBUSY` 是**宿主事实**，换一台机器会对同一个仓库
得出不同分类 ⇒ 判据不再可复现（**铁律二**）。且「能跑但没接进门控」（`spec:validate`）与
「根本跑不了」（`schema --check`）**风险不同、成因不同**，运行时探测区分不出。

### 决定 3：`local` 与 `ci-only` 都必须付代价，且**双向**

- `local` MUST 带 `localEvidence: {file, marker}`，marker MUST 在该门控源里**逐字存在**——
  「本机也跑」从一句话变成一个**可判定**的引用（W75「已声明 ≠ 已执行」）。
- `ci-only` MUST 带 `reason`（∈ 闭集 `CI_ONLY_REASONS`），且 MUST NOT 带 `localEvidence`
  （否则「它是不是本机也跑」没有单一回答，W88 的「谁在管它」）。

成因闭集按**实测**分四格，理由与 `UNWIRED_CAUSES` 同形——一个未知成因是一个标签，不是一个理由：
`host-subprocess`／`compiled-artifact`／`platform-toolchain`／`not-in-profile`。

### 决定 4：`infra` MUST NOT 是绕过检查的后门

`infra` 的 `run` MUST NOT 命中 `JUDGE_COMMAND_SHAPES`（`cargo xtask`／`node --test`／
`pnpm exec`／`pnpm <非 install>`）。否则「把判定步骤标成环境准备」就能绕开整套检查（**自我满足**）。

### 决定 5：`diagnostic` 由**结构**认出，不由名字认出

`diagnostic` 的 step 其 `if` MUST 含 `failure`。`if: runner.os == 'Linux'` 是平台条件，
**不是**失败条件 ⇒ 必须被区分（实测：平台条件过不了这条）。

## 如实披露（本判据**不**验什么）

1. **不验「CI 真的跑了某步」**——那要读 CI 的运行结果，不是静态判据的事。
2. **不验 `ci-only` 的成因是否成立**——语义，仍由人读（与 ADR 0054 决策 2 同族的边界）。
   判据只能验「成因落在已识别的四格之一」。
3. **不验 `local` 的落点真的覆盖了这个 step**——验的是「那处落点存在」。
   把已覆盖的报成未覆盖是**假阳性**（安全方向）。
4. **`infra` 的形状检查验的是命令形状，不是语义**——把环境准备误报成判定会红，属安全方向。

## 与 `test-reachability` 的边界（同一件事 MUST NOT 两处定义）

两张表都以 `foundation.yml` 为 host，但**断言单元不同**。实测（改真实表 → 跑两个套件 → 还原）：

| 注入 | `workflow.test.mjs` | `registry.test.mjs` |
| --- | --- | --- |
| 删掉 `test-reachability` 的站点 | **0 红** | **2 红** |
| 删掉一条 `WORKFLOW_STEPS` 登记 | **5 红** | **0 红** |

⇒ 二者**互不顶替**，各有独有覆盖：前者管「workflow 的 step」，后者管「测试文件的可达性」。

## 后果

- 门控新增一个 Node 套件（`tests/contracts/workflow.test.mjs`，13 个用例），已在
  `xtask/src/lib.rs` 的 `engineering_checks` 登记；**Rust 零行为改动**（只加一行登记）。
- 新增一处**有意的耦合**：`test-reachability` 的站点表被 W84 的闭世界与 `registry.test.mjs` 的
  逐字清单同时钉住 ⇒ 本轮新增站点时**两处同时红**——这正是它该有的行为（新增测试文件必须被处置）。
- 规格：`engineering-foundation` 新增 1 条 Requirement（ADDED，正文 258 字符）。

## 遗留

1. **不验 `ci-only` 的成因是否仍然成立**：例如 `schema --check` 哪天在本机变得可跑，
   判据不会有信号（`reason` 仍写着 `host-subprocess`）。要发现它得靠人重新实测宿主。
2. **`infra` 的形状是启发式**：将来出现新的判定命令形状（例如 `deno test`、新工具链），
   若它不被 `JUDGE_COMMAND_SHAPES` 覆盖，且被标成 `infra`，判据会放行（**假阴性方向**）。
   真出现时的正确修法是**扩那张形状表**，MUST NOT 把该步直接标成 `infra` 了事。
3. **步的版本号进身份**：`actions/checkout@v4` 升到 `@v5` 会让判据红（要求显式处置）。
   这是**有意**的（infra 步的版本变更值得人看一眼），但它也确实是一处需要维护的地方。
4. **四类都非空是被断言的**：某一类被**合法清空**（例如诊断通道被移除）会让
   `the registry must exercise the … kind` 这条红 ⇒ 要求显式处置。**空态 MUST 被显式断言，
   MUST NOT 静默跳过**（W92 同族）。
5. **W90 §八遗留 1／3／4／5、W91 的跨语言键级消费、W92 的基线收录、W93 的锚点唯一性、
   W94 的 `needs_external` 审计**均**未动**。

## 勘误（W108）

**遗留 1 已关闭。** `ci-only` 与 `standalone` 的成因现在 MUST **付代价**：每条成因 MUST 给出一条
**可从仓库自证**的凭据（核验在 `scripts/contracts/cause-payment.mjs`；两处兑付表
`CI_ONLY_REASON_PAYMENTS` / `STANDALONE_REASON_PAYMENTS`）。上面那条遗留预言的失效**真的发生了**
——`cargo xtask schema --check` 在本机变得可跑（退出码 `0`），而注册表里的 `reason` 逐字未变，
全部判据零信号。正是它把一条「只能靠人重新实测宿主」的声明变成了一条**可判、且当时会红**的断言。

同轮成因闭集收缩 **4 → 3**：`host-subprocess` **逐出**。理由是**它不可能有凭据**——它的成因是
宿主/机器事实，而宿主瞬态 MUST NOT 成为判定输入（铁律二）⇒ 它是这条规则里一个**永久无法兑付的
空格**。它的那条 step 实例改归 `compiled-artifact`（该命令要 `protoc` 3.21.12，超出 portable 门控
「只需 Rust 工具链」的前提）。详见 [ADR 0086](0086-cause-payment.md)。

⚠️ 逐出的理由**不引用任何发生过的瞬态**：即使本机从未跑通那条命令，结论相同。上面那次实测只作
**触发**（把遗留变成可见），**不**作逐出的依据。

遗留 2（`infra` 的形状启发式）、3（步版本号进身份）、4（四类非空）、5 **未动**。
