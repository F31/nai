# W96 轮次报告：`cargo xtask` 的每一条命令必须具名在册

- 日期：2026-10-14
- 变更：`openspec/changes/2026-10-14-w96-xtask-command-surface/`
- ADR：[0074](../adr/0074-xtask-command-surface.md)

## 一、母题

W95 补的是判定的**第二个执行入口**（`.github/workflows/foundation.yml` 的 step 集合）。
本轮补的是**第一个执行入口的入口面**：`xtask/src/lib.rs` 的 `execute()` ——
一个 `match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice()` 的 **24 条臂**。

它比 W95 的对象更贴近门控本身：**它与 `check --profile full` 的门控闭包住在同一个文件里**，
相隔几十行，却没有任何在册台账。同一文件里的 `Usage: cargo xtask …` 字符串是命令集合的
**第二次书写**，两处没有任何机制要求一致。

同族先例：W84（测试文件的可达性）、W86（OpenAPI 文档集）、W95（CI workflow 的 step）——
同一手法，**新的一层对象**：`execute()` 的命令面。

裁定过程：核实三个候选的「有没有东西可改」后，用户选定 **「xtask 命令面台账」**。

## 二、核实（先测后设计）

### 2.1 缝存在——三个探针（改真实 `lib.rs`，`git checkout --` 还原，`sha256` 逐字校验）

| 探针 | 改动 | 本机 4 个 Node 套件（registry 52 / schema 30 / platform 22 / workflow 13） |
| --- | --- | --- |
| P1 加法 | 插一条臂 `["check", "--audit"] => Ok(())` | **全绿、零信号** |
| P2 减法 | 删掉 `["check", "--secrets"] => secrets::check(root)` | **同样全绿**——一条判定命令**静默消失** |
| P3 漂移 | usage 字符串追加 `\| check --audit` | **同样全绿**——usage 与 dispatch **分家** |

改前 `sha256('xtask/src/lib.rs') = bfdd6d2e0a92f29ab4939df9e8c6f32704b6758b5f9f9e3adbcdfcdcedaf7bec`
（三次复现一致）。

### 2.2 无替代守卫

- `grep -rn "Usage:" tests/` **为空**；没有任何测试读 match 臂或 usage 字符串。
- 命令面与 usage **只存在于代码里**，没有第二双眼睛。

### 2.3 两侧的真实集合（纯 JS 探针，括号计数法 + 臂模式正则 + 标识符锚展开）

```
dispatch (24)                     ← execute() 的臂
usage    (21)                     ← Usage: 字符串展开
usage \ dispatch = []
dispatch \ usage = ['help', '--help', '']
```

⇒ **双向对账成立且当前是干净的**：usage 不多说一条命令，dispatch 也不漏写一条
（三个 help 类别本就不该出现在 usage 里，因为它们就是「打印 usage」本身）。

### 2.4 门控区域（同一探针实测）

| 区域 | 起点 | 字符数 |
| --- | --- | --- |
| `engineering_checks` 的函数体 | `fn engineering_checks(` | 1898 |
| `check --profile portable\|windows-local` 的 arm 体 | 该 arm 的 `=>` 后的 `{}` | 403 |
| `check --profile full` 的 arm 体 | 同上 | 277 |

5 条 `gate` 候选落点**全部命中**区域；4 条 `standalone` 的执行体
（`golden::check(`／`secrets::check(`／`schema::generate(`／`schema::contracts(`）**全部 miss**。

### 2.5 命令的真实集合（24 条，逐条解析得出）

| 类别 | 条数 | 命令 |
| --- | --- | --- |
| `gate` | 5 | `check-deps`、`check --registry`、`check --profile portable`／`windows-local`／`full` |
| `standalone` | 13 | `check --golden`、`check --golden --dir`、`check --secrets`、`schema --check`、`test --suite contracts`、`test --suite ×8` |
| `artifact` | 3 | `sbom`、`sbom --out`、`schema` |
| `help` | 3 | `help`、`--help`、空串 |

三个**需要判断**的地方，都有实测依据：

1. **`check-deps` 与 `check --registry` 落在 `engineering_checks` 体内**（`deps::check(root)?`／
   `registry::check(root)?`）——它们既是独立入口，执行体又是门控的一步 ⇒ `gate`，且两条各有**独有**落点。
2. **`test --suite ×8` 不是 `gate`**：门控跑的是 `cargo test --workspace --locked --exclude HARNESS`，
   覆盖同一批包但**不走这个入口** ⇒ `standalone` + `superseded-by-workspace-run`（本轮**新增**的成因）。
3. **`schema`（生成）是 `artifact`，`schema --check` 是 `standalone`**：前者不产生判定结论，
   后者产生（且本机需派生子进程，`host-subprocess`）。

## 三、另两个候选为何没选（实测，不是推测）

- **非 config schema 键级消费**：实测不利——消费形态跨语言差异大（`plugins/avatar-2d/src/lib.rs:76`
  的 `struct Manifest`、`scripts/platform/matrix.mjs`、`crates/nai-contracts/src/generated/catalog.rs`），
  W90/W91 的「三分类形状」套不上，假阳性风险高。前端（`packages/`、`apps/`）对 `schemas/config` 键
  **零命中**（`grep "lease_renew_ms"` 与 `grep "schemas/config"` 皆空）⇒ 跨语言候选不成立。
- **`const` 钉死键与读取点的交叠**（W90 §八遗留 3）：实例只有 1 个
  （`session_defaults.allow_auto_without_platform_grant` 在 `const:false` 与
  `apps/engine/src/pipeline.rs` 同时命中）⇒ 判据太薄。

## 四、设计与取舍

### 4.1 判定单元是**命令签名**，不是命令名（W91）

签名 = **空格连接的顶层字面量序列**。`check --golden` 与 `check --golden --dir` 是**两条**命令；
`test --suite` 的 8 条共享前缀。匹配锚是**整条签名**，不是 `"--secrets"`——后者会在
`secrets::check` 这类**别的对象**上命中（W91 的 `nai-plugin-sdk` 同名事故的重演风险）。

### 4.2 usage 与 dispatch **对账**，而不是**消除第二处**

W93 的「同一件事 MUST NOT 两处定义 ⇒ 调用第一处」在这里做不到：usage 是给人看的自然语言，
不可能由 match 臂渲染。⇒ 采取**第三态**：承认两处、**断言一致**——
`usage ⊆ dispatch` 与 `dispatch \ usage ⊆ HELP_SIGNATURES`。
`HELP_SIGNATURES = ['help', '--help', '']` 是**具名例外**：少写一条判定命令是缺陷，少写 `help` 不是。

### 4.3 `gate` 的代价 MUST 是「落在门控闭包内」

`evidence.marker` MUST 在**门控区域**内逐字存在。只要求「在文件里逐字存在」的话，marker 就能填
**arm 自己**的文本（`["check", "--secrets"]`），把「本机门控真的跑它」偷换成「这条命令自己存在」
——**自我满足**的典型形态。区域由**解析**得出，MUST NOT 由调用方声明（否则等于把门槛交给被审者自己填）。

这是本项目「注入事实而非判定」这一族的**第五次出现**：
`repositoryFiles` → `readTrackedText` → W94 两张表 → W95 `gateSources` → 本轮 `gateRegions`。

### 4.4 `standalone` 的成因取闭集，且本轮**新增一格**

`host-subprocess`（`schema --check`）／`requires-compiled-artifact`（`test --suite contracts`）／
**`superseded-by-workspace-run`**（`test --suite ×8`）／`deliberate-on-demand`（`check --golden`、
`check --secrets`，依据 ADR 0043）。

第三格是本轮新增：它记录一个真实且容易误判的事实——这 8 条**不是「跑不了」**，
而是「门控跑的是它们的**超集**」。W95 的教训「看起来像 CI-only ≠ CI-only」在这里的对称形态是
「看起来像跑不了 ≠ 跑不了」。

### 4.5 不透支的两条纪律

- **分类是静态声明，MUST NOT 由运行时探测得出**（铁律二）：`EBUSY` 是宿主事实，
  换一台机器会对同一个仓库得出不同分类。
- **不可解析 ⇒ 具名失败**；dispatch／usage／某一类为空 ⇒ **显式报错**
  （空态 MUST 被显式断言，W92 同族）。

## 五、边界（实测）+ 一处既有清单的合法改动

三者共享宿主（`xtask/src/lib.rs` 与 `.github/workflows/foundation.yml` 互相调用），
但**断言单元不同**。实测（改真实表 → 跑两个套件 → 还原）：

| 注入 | `workflow.test.mjs` | `xtask-commands.test.mjs` | `registry.test.mjs` |
| --- | --- | --- | --- |
| 删掉一条 `WORKFLOW_STEPS` 登记 | **2 红** | **0 红** | — |
| 删掉一条 `XTASK_COMMANDS` 登记 | **0 红** | **4 红** | — |
| 删掉新增的 `test-reachability` 站点 | — | **0 红** | **2 红** |

⇒ 三者**互不顶替**：`workflow-steps` 管「CI workflow 的 step」，本模块管「`execute()` 的 CLI 命令」，
`test-reachability` 管「测试文件的可达性」。

**本轮合法地改了既有清单**（该模式第四次出现）：`registry.test.mjs` 里那张**逐字写死的站点 id 列表**
（8 个）在新增测试文件后**如期变红**——这正是它的设计意图。处置是把
`xtask-engineering-xtask-commands` 加进去（8 → 9），不是放宽断言。同一时刻 W84 的闭世界也会报
「site `xtask-engineering-xtask-commands` matches no tracked file」（因为新测试文件尚未进 `.git/index`）
——两条断言**各自**独立地发现了同一件事，随后 `git add` 后同时转绿。

### 5.1 非空洞注入 P1–P6（真实文件 + `sha256` 逐字还原 **ALL-OK**）

| 注入 | 具名错误（原文） |
| --- | --- |
| P1 未登记臂 | `dispatches the command "check --audit", which nobody declares — an unregistered command is not judged` + `is dispatched but never appears in the usage string — a command nobody can discover` |
| P2 删已登记命令 | `is declared to dispatch "check --secrets", but it does not — a registered command that vanished is a judgement that silently disappeared` + `the usage string advertises "check --secrets", which the dispatch does not implement — usage must not promise what does not exist` |
| P3 usage 漂移 | `the usage string advertises "check --audit", which the dispatch does not implement — usage must not promise what does not exist` |
| P4 落点移出门控闭包 | `"check-deps" cites xtask/src/lib.rs for "deps::check(root)?", which is not inside the local gate closure (engineering_checks, arm@6618, arm@7459) — the evidence is a claim, not a fact` |
| P5 删 arm 独有落点 | `"check --profile windows-local" cites xtask/src/lib.rs for ""windows-local requires a native Windows environment"", which is not inside the local gate closure (…) — the evidence is a claim, not a fact` |
| P6 usage 少写命令 | `the command "sbom" is dispatched but never appears in the usage string` + `"sbom --out"`（两条同时报） |

另有 **17 个用例内的 F 级注入**（未登记／消失／改名两侧／usage 两个方向／凭据不存在／
**凭据在文件里逐字存在却不在门控闭包内**／凭据指向未注入的文件／无凭据／成因域外／
standalone 挂凭据／artifact 挂凭据／help 挂成因／空 dispatch／duplicate 签名／不可解析）。
其中**「凭据在文件里逐字存在却不在门控闭包内」是本层独有的覆盖**：只验「逐字存在」的判据会放它过去。

## 六、门控与计数

### 6.1 变更校验

| 项 | 结果 |
| --- | --- |
| `openspec validate 2026-10-14-w96-xtask-command-surface --strict` | **valid** |
| `openspec validate --all --strict`（归档前，含活动变更） | **44 passed / 0 failed**（44 items） |

### 6.2 全量门控

| 项 | 结果 |
| --- | --- |
| `cargo xtask check --profile full` | **EXIT=0**（3 分 28 秒） |
| Rust 目标 / 用例 | **91 目标 / 862 passed / 0 failed** |
| `LNK1104`（宿主瞬态） | **0**；预链接**第 1 轮**即收敛（无重试痕迹） |
| `cargo fmt --all --check` | **EXIT=0**（门控内的一步） |
| Node 套件（7 个） | registry **52** / schema **30** / platform **22** / workflow **13** / **xtask-commands 17（本轮新增）** / desktop **5** / ui **52** —— 全绿 |

- Rust 目标数与用例数与 W95 **逐项相同（91 / 862）**：本轮 `xtask/src/lib.rs` 只加了**一行登记**
  （含注释共 3 行，零行为改动），新增的 17 个用例住在 **Node 侧**，不进 Rust 的 `test result` 计数。
  目标数 91 = 73 `Running` + 18 `Doc-tests`（`^     Running ` 行数为 74 = 真实目标 + 1 条 cargo 包装行）。
- Node 套件由 **6 个 → 7 个**（新增 `tests/contracts/xtask-commands.test.mjs`）。
- portable 侧合计 **134** passed（52 + 30 + 22 + 13 + 17）。

### 6.3 两条 validate

| 项 | 结果 |
| --- | --- |
| `openspec validate --all --strict`（归档后，不含活动变更） | **43 passed / 0 failed** |
| `openspec validate --all --strict --archived`（含已归档变更） | **94 passed / 0 failed**（W95 为 93） |
| 归档结果 | `engineering-foundation: + 1 added, ~ 0, - 0`；**Task status ✓ Complete** |

## 七、CI

（推送后的单次低频查询结果回填于此：唯一查询参数破坏缓存 + `Cache-Control: no-cache` +
回读 `head_sha` == 推送 SHA。）

## 八、遗留

1. **不验 `standalone` 的成因是否仍然成立**：`schema --check` 哪天在本机变得可跑，判据不会有信号。
2. **命令的签名不含值**：`check --golden --dir <x>` 与 `... <y>` 是同一条签名——有意
   （签名描述**入口的形状**，不描述取值）。
3. **`superseded-by-workspace-run` 依赖「全 workspace ⊇ 各 suite 的包集」**，而判据**不验**这个包含关系。
   若某个 suite 将来引入 workspace 之外的包，判据不会有信号。
4. **四类都非空是被断言的**：某一类被合法清空会红 ⇒ 要求显式处置（W92 同族）。
5. **W90 §八遗留 1／3／4／5、W91 的跨语言键级消费、W92 的基线收录、W93 的锚点唯一性、
   W94 的 `needs_external` 审计、W95 的 `ci-only` 成因是否仍成立**均**未动**。

## 九、如实披露

1. **不验「CI 真的跑了某条命令」**——那要读运行结果，不是静态判据的事。本报告不断言 CI 侧的执行结果，
   除非 §七 有运行级证据。
2. **不验 `standalone` 的成因是否成立**——语义，仍由人读（与 ADR 0054 决策 2 同族）。
3. **不验 `check --profile portable` 的证据专属于它**：`engineering_checks(root)?` 同时出现在
   `full` 的 arm 里；判据验「该调用点在门控区域内逐字存在」，**不**验「它只属于 portable」。
   这是**假阴性方向**的宽松（不会把问题掩盖成绿的）。五条里只有这一条是共享的。
4. **`braceBody` 用括号计数，不计字符串／注释里的花括号**——本文件里没有那种情形；将来若出现，
   定位会偏（下游断言会先响）。
5. **本轮不改 `execute()` 与 usage 一个字符**：补的是判据与登记，不动被审对象。
   （`engineering_checks` 里加了一行 `schema::node(...)` 登记与两句注释——它不在 `execute()` 内，
   也不影响 usage。）
