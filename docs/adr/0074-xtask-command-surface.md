# ADR 0074：`cargo xtask` 的每一条命令必须具名在册 —— usage 与 dispatch 的两处定义必须对账

- 日期：2026-10-14（W96）
- 状态：已接受
- 相关：ADR 0073（CI workflow 的 step 闭世界）、ADR 0062（测试文件可达性闭世界）、
  ADR 0064（OpenAPI 文档集闭世界）、ADR 0051（决策 1：重叠处逐字一致）、
  ADR 0054（决策 2：语义留给人读）、W91（判定单元 MUST 是身份，MUST NOT 是裸名字）、
  W93（同一件事 MUST NOT 两处定义 ⇒ 无法消除第二处时改为**断言一致**）

## 背景

`xtask/src/lib.rs` 的 `execute()` 是本机门控的**入口面**：一个
`match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice()` 的 **24 条臂**。
这个集合**没有任何在册台账**——而它与 `check --profile full` 的门控闭包**住在同一个文件里**，
相隔几十行。

同一文件里还有命令集合的**第二次书写**：`println!` 里的
`Usage: cargo xtask check-deps | check --registry | …` 字符串。两处**没有任何机制**要求一致。

同族先例：W84（测试文件的可达性）、W86（OpenAPI 文档集）、W95（CI workflow 的 step）。
本轮是同一手法下的**新的一层对象**：`execute()` 的命令面。

## 证据

三个探针（改真实 `lib.rs` → 跑本机 4 套件 → `git checkout --` 还原 → `sha256` 逐字校验）：

| 探针 | 改动 | 结果（registry 52 / schema 30 / platform 22 / workflow 13） |
| --- | --- | --- |
| P1 加法 | 插一条臂 `["check", "--audit"] => Ok(())` | **全绿、零信号** |
| P2 减法 | 删掉 `["check", "--secrets"] => secrets::check(root)` | **同样全绿** —— 一条判定命令**静默消失** |
| P3 漂移 | usage 字符串追加 `\| check --audit` | **同样全绿** —— usage 与 dispatch **分家** |

增补核实：

1. **无替代守卫**：`grep -rn "Usage:" tests/` 为空；没有任何测试读 match 臂。
2. **两侧的真实集合**（纯 JS 探针，括号计数法 + 臂模式正则 + 标识符锚展开）：
   `dispatch (24)`、`usage (21)`、`usage \ dispatch = []`、`dispatch \ usage = ['help','--help','']`。
3. **门控闭包的三块区域**（同一探针实测）：`engineering_checks` 的函数体（1898 字符）与
   `check --profile` 的两条 arm 体（403 / 277 字符）。
4. 5 条 `gate` 候选落点全部命中区域；4 条 `standalone` 的执行体（`golden::check(`／
   `secrets::check(`／`schema::generate(`／`schema::contracts(`）**全部 miss**。

## 决定

### 决定 1：判定单元是**命令签名**，不是命令名（W91）

签名 = **空格连接的顶层字面量序列**。`check --golden` 与 `check --golden --dir` 是**两条**命令；
`test --suite` 的 8 条共享前缀。匹配锚是**整条签名**，不是 `"--secrets"` 这类裸片段——
后者会在 `secrets::check` 这类**别的对象**上命中（W91 的 `nai-plugin-sdk` 同名事故的重演风险）。

### 决定 2：usage 与 dispatch **对账**，而不是**消除第二处**

W93 的「同一件事 MUST NOT 两处定义 ⇒ 调用第一处」在这里做不到：usage 是给人看的自然语言，
不可能由 match 臂渲染。⇒ 采取**第三态**：承认两处、**断言一致**——
`usage ⊆ dispatch`（不得声称不存在的命令）与 `dispatch \ usage ⊆ HELP_SIGNATURES`
（除三条具名豁免外，每条命令 MUST 被发现）。

`HELP_SIGNATURES = ['help', '--help', '']` 是**具名例外**：少写一条判定命令是缺陷，少写 `help`
不是。混进同一个「其余」里，「少写一条命令」就会溜过去。

### 决定 3：两层闭世界——命令面与 usage 面

- **命令面**：`execute()` 的每一条命令 MUST 有且只有一条登记；在册的每条 MUST 仍在 dispatch 里。
- **usage 面**：与 dispatch 双向对账（决定 2）。

⇒ 新增一条臂、删掉一条臂、改一个字面量、usage 说一句假话，都会**具名失败**。

### 决定 4：`gate` 的代价是**落在门控闭包内**，不是一个「逐字存在」的串

`gate` MUST 带 `evidence: {file, marker}`，marker MUST 在**门控区域**（`engineering_checks`
的函数体 ∪ 各 `check --profile` arm 体）内**逐字存在**。

**为什么必须有「区域」这一层**：只要求「marker 在文件里逐字存在」，marker 就可以填 **arm 自己**
的文本（`["check", "--secrets"]`），从而把「本机门控真的跑它」偷换成「这条命令自己存在」
——**自我满足**的典型形态。区域把判定域收紧到门控闭包内。

这是本项目「注入事实而非判定」这一族的**第五次出现**（`repositoryFiles` → `readTrackedText` →
W94 两张表 → W95 `gateSources` → 本轮 `gateRegions`）：区域由**解析**得出，MUST NOT 由调用方
声明，否则把区域交给被审者自己填。

### 决定 5：`standalone` 的成因取自**闭集**，域外 ⇒ 具名失败

| 成因 | 含义 | 实例 |
| --- | --- | --- |
| `host-subprocess` | 需要派生子进程，宿主对该步 `EBUSY` | `schema --check` |
| `requires-compiled-artifact` | 需要 protoc／编译产物 | `test --suite contracts` |
| `superseded-by-workspace-run` | 门控跑的是**全 workspace** 测试，覆盖同一批包但**不走这个入口** | `test --suite ×8` |
| `deliberate-on-demand` | 有意按需，**不并入**常规门控（ADR 0043） | `check --golden`、`check --secrets` |

`superseded-by-workspace-run` 是本轮**新增**的成因，记录一个真实且容易被误判的事实：
这 8 条不是「跑不了」，而是「**门控跑的是它们的超集**」。

### 决定 6：四类闭集，触发条件只由「字段存在」决定（W83）

- `gate` MUST 带 `evidence`；`standalone` MUST 带 `reason`、MUST NOT 带 `evidence`；
- `artifact`／`help` MUST NOT 带 `evidence` 与 `reason`（**不合并维度**）。

### 决定 7：不可解析 ⇒ **具名失败**，不静默降级

`dispatchSignatures`／`usageSignatures`／`gateRegions` 任一失败 ⇒ 一条**具名错误**。
dispatch 为空、usage 为空、某一类为空，都**显式报错**（空态 MUST 被显式断言，W92 同族）。

## 如实披露（本判据**不**验什么）

1. **不验「CI 真的跑了某条命令」**——那要读运行结果，不是静态判据的事。
2. **不验 `standalone` 的成因是否成立**——语义，仍由人读（ADR 0054 决策 2 同族）。
3. **不验 `check --profile portable` 的证据专属于它**——`engineering_checks(root)?` 同时出现在
   `full` 的 arm 里；判据验「该调用点在门控区域内逐字存在」，**不**验「它只属于 portable」。
   这是**假阴性方向**的宽松（不会把问题掩盖成绿的），与 W92「永远选安全方向」一致。
   （`windows-local` 取了它那条 arm **独有**的落点，所以五条里只有一条是共享的。）
4. **`braceBody` 用括号计数，不计字符串／注释里的花括号**——本文件里没有那种情形；
   将来若出现，定位会偏（下游断言会先响）。
5. **`matchBody` 依赖 `match args` 这个字面量**——它是被审对象的一部分，改名会被具名报出。

## 与 W95 及 `test-reachability` 的边界（同一件事 MUST NOT 两处定义）

三者共享宿主（`xtask/src/lib.rs` 与 `.github/workflows/foundation.yml` 互相调用），
但**断言单元不同**。实测（改真实表 → 跑两个套件 → 还原）：

| 注入 | `workflow.test.mjs` | `xtask-commands.test.mjs` | `registry.test.mjs` |
| --- | --- | --- | --- |
| 删掉一条 `WORKFLOW_STEPS` 登记 | **2 红** | **0 红** | — |
| 删掉一条 `XTASK_COMMANDS` 登记 | **0 红** | **4 红** | — |
| 删掉新增的 `test-reachability` 站点 | — | **0 红** | **2 红** |

⇒ 三者**互不顶替**：`workflow-steps` 管「CI workflow 的 step」，本模块管「`execute()` 的 CLI 命令」，
`test-reachability` 管「测试文件的可达性」。

## 后果

- 门控新增一个 Node 套件（`tests/contracts/xtask-commands.test.mjs`，**17 个用例**），已在
  `xtask/src/lib.rs` 的 `engineering_checks` 登记；**Rust 零行为改动**（只加一行登记，
  `execute()` 与 usage 一个字符未动）。**Node 套件由 6 个变 7 个**。
- `test-reachability` 站点 **8 → 9**；`tests/registry.test.mjs` 的逐字站点清单同步
  ⇒ 本轮新增站点时**两处同时红**（W95 已建立的耦合，本轮复用）。
- 规格：`engineering-foundation` 新增 1 条 Requirement（ADDED，正文 399 字符）。

## 遗留

1. **不验 `standalone` 的成因是否仍然成立**：例如 `schema --check` 哪天在本机变得可跑，
   判据不会有信号（`reason` 仍写着 `host-subprocess`）。
2. **命令的签名不含值**：`check --golden --dir <x>` 与 `check --golden --dir <y>` 是同一条签名
   ——这是有意的（签名描述**入口的形状**，不描述取值）。
3. **`superseded-by-workspace-run` 依赖「全 workspace ⊇ 各 suite 的包集」这一事实**，
   而判据**不验**这个包含关系。若将来某个 suite 引入 workspace 之外的包，判据不会有信号。
4. **四类都非空是被断言的**：某一类被**合法清空**（例如 `artifact` 命令全部移除）会让
   `the registry exercises no … command` 这条红 ⇒ 要求显式处置（W92 同族）。
5. **W90 §八遗留 1／3／4／5、W91 的跨语言键级消费、W92 的基线收录、W93 的锚点唯一性、
   W94 的 `needs_external` 审计、W95 的 `ci-only` 成因是否仍成立**均**未动**。
