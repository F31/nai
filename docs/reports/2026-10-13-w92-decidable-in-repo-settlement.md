# W92 轮次报告：清空 `decidable_in_repo`（A2A 首版边界落规格、画像验证订正）

- 日期：2026-10-13
- 变更：`w92-decidable-in-repo-settlement`（`openspec/changes/archive/2026-10-13-w92-decidable-in-repo-settlement/`）
- ADR：[0070](../adr/0070-decidable-in-repo-settlement.md)
- 基线：V3.11（`docs/直播数字人 + 智能体系统技术方案 V3.11.md`，**本轮一字不动**）

## 一、判断

上一轮结尾给出两个候选（结清 3 条待裁定项 / 演练级实操）。`AskUserQuestion` 的裁定是
**「结清 3 条待裁定项」**。本轮据此开工。

落点的**独立核实**（不依赖上一轮的结论）：`tests/pending-decisions.yaml` 的 34 条里，`decidable_in_repo`
（必填字段 = `recommendation`，即**"建议"**）还剩 **3 条**，且这 3 条正是 ADR 0066 遗留 1 与
ADR 0067 遗留 4 逐代传下来的同一批。⇒ 「结清」= **清空该态**，不是新设计。

## 二、核实（先读后判）

### 2.1 PD-22 / PD-24 的落点是**同一段活规格**

`openspec/specs/agent-harness/spec.md`（line 208–229）的 `A2A inbound is an external request;
outbound delegation is bounded`。逐条实测：

| 事实 | 出处 |
| --- | --- |
| 该条正文写「委派链深度 MUST 有上限（**初值 1**）」 | 活规格 line 214 |
| 基线：「委派边界：…**委派链深度上限初值 1（不得再委派）**」 | 基线 §46.3（line 2164） |
| 基线：「**有界**：…委派深度全部有界，**越界具名终止、不静默截断**」 | 基线 §40.4（line 1930） |
| 基线把「A2A：入站入口是否对第三方开放；…委派深度上限是否可配置」**登记为待决策** | 基线 §48.7（line 2362） |
| `grep`「委派链深度」在 Rust **零命中**（无 `DELEGATION_DEPTH`／`delegation_depth`／`max_delegation`） | 全仓 `.rs` |
| H13／H14 是 `capability_pack` + `pending`（未开始，W62） | `tests/registry.yaml` |

**规格原来漏掉的那半边**：原文只说「对端身份 MUST 由本地信任配置**派生**」——它约束**已到达的请求
如何取身份**，**没有**约束**谁可以到达**。前者是"身份从哪来"，后者是**可达集**。只写前者，实现完全可以
"接受任意第三方入站、再查它的身份、查不到就当匿名外部请求处理"——**规格允许这种读法**。

### 2.2 PD-34 的登记**与事实不符**

| 事实 | 出处 |
| --- | --- |
| 「### 54.2 **用户画像（初版，未经验证）**」 | 基线 §54.2（line 2494） |
| 「**用户画像的验证方式**：§54.2 初版未经访谈，**MUST 在 R1 实现期以真实使用者验证后才可作为档位依据**」 | 基线 §57.5（line 2647） |
| 「本节**未经用户访谈验证**，§18.3 的「未实测」声明同样适用于本节」 | 基线 §54.2（line 2502） |

⇒ **验证方式已被基线答出**（"以真实使用者验证"），未闭合的是**验证的执行**（真人访谈），
它必然在**仓外**。⇒ 登记为 `decidable_in_repo`（"可在仓库内决定"）是**不实分类**。

### 2.3 判据的形状（决定改动边界）

`scripts/contracts/pending-decisions.mjs`：六态封闭枚举 + `REQUIRED_FIELD` 一态一必填字段 +
`DISPOSITION_COUNTS` **逐项写死**（W66 教训：「总数对『被改类别』是瞎的」）。⇒ 改任何处置
**必须同步**这张表，否则判据红（这是想要的）。`CHAPTER_COUNTS`（各章新增条数）**不动**——条目数没变。

## 三、裁定

### 3.1 PD-22 → `decided_in_repo`，落到活规格

采纳建议并落到 A2A 那条 Requirement 的**新正文**：

> **R1 首版入站只响应本机与本地信任配置中已登记的受信对端**；对第三方开放 MUST 作为一次**显式的版本决策**，
> MUST NOT 因请求自称可信或自称已获授权而放行。

新增 Scenario `A third party asks to be admitted`：未登记对端发起入站 ⇒ **拒绝并具名**
（MUST NOT 以「无响应」代替拒绝——「无响应」与「拒绝」在类型上不同，前者不可区分于"服务没起来"）。

### 3.2 PD-24 → `decided_in_repo`，落到活规格

> **R1 首版委派链深度上限 MUST 为常量、MUST NOT 提供配置面**。

新增 Scenario `Delegation depth is asked to be configured`。**不把"配置化前置条件"写进规格**：
那两句（协议验证之后 + 引入时的两项要求）是**前瞻条件**，不是 R1 可判定的规范（"协议验证之后"
没有仓库内判据）；它们留在台账 `recommendation`（注解性留存）与 ADR 0070 里。

### 3.3 PD-34 → `needs_external`（**订正**）

`closure` = 「三个画像各产出一份**真实使用者访谈**记录（「画像-档位映射已验证」）；在此之前 §54.2 MUST NOT
被用作「Lite 该多 Lite」的论据」。`recommendation` **原文保留**（先例 PD-09）。

**排除另两个候选**：

- **不是 `decided_in_repo`**：该态要求「决定已作出**且已落地**」；PD-34 的闭合条件（访谈记录）在仓库里
  **不存在** ⇒ "已落地"不成立。
- **不是 `answered_by_baseline`**：§57.5 没给出 recommendation 的颗粒度（三个画像**各自**验证）；更关键，
  §54.2 的自我声明恰恰是**"未经验证"**——把一条"基线说它没验证"的项记成"基线回答了它"，是**不实声明**
  （W88 对 PD-09 排除了同一个陷阱）。

### 3.4 不越权（重要）

| 载体 | 句子 | 对象 |
| --- | --- | --- |
| 基线 §46.3 | 委派链深度**上限初值 1**（不得再委派） | 该协议的**一般描述**（"初值"= 起点值） |
| 规格（本轮） | **R1 首版**委派链深度上限 **MUST 为常量、MUST NOT 提供配置面** | **R1 首版**的具体化 |

两者**不矛盾**：后者是前者在**首版范围内的取值决定**，正是 PD-24 要裁的那个自由度。
⇒ **基线正文本轮一字不动**。PD-22 同理：§48.7 本就把它登记为待决策 ⇒ 规格补上是从"待决"变"已决"。

## 四、实现

| 文件 | 改动 |
| --- | --- |
| `tests/pending-decisions.yaml` | PD-22／PD-24 → `decided_in_repo` + `landing: ["openspec/specs/agent-harness/spec.md"]`（`recommendation` 保留为注解）；PD-34 → `needs_external` + `closure`（`recommendation` 保留） |
| `scripts/contracts/pending-decisions.mjs` | `DISPOSITION_COUNTS`：`decidable_in_repo` **3 → 0**、`decided_in_repo` **2 → 4**、`needs_external` **20 → 21**（总数仍 **34**）；注释补 W92 迁移段 |
| `tests/registry.test.mjs` | 两处注入的**样本定位由处置改为字段存在**；新增空态双向一致断言与本轮成果断言；import 补 `DISPOSITION_COUNTS` |
| `openspec/specs/agent-harness/spec.md` | 一条 Requirement 被 **MODIFIED** 替换（正文 455 字符 < 500；Scenario 3 → 5） |

**Rust 零改动、`cargo xtask` 零改动、`tests/registry.yaml` 零改动。**

### 4.1 「清空一个态」暴露的守卫隐含依赖（本轮的技术核心）

`decidable_in_repo` 计数改成 0 后，**两处测试用例失去了作用对象**——它们按**处置**定位样本：

```js
const item = stray.entries.find(entry => entry.disposition === disposition);
```

**实测**（改前/改后对照）：改成 0 后立刻 **2 条红** —— `#16 a pending decision cannot cite a section
the baseline does not have`、`#22 each disposition has to pay for itself with its own required field`。

**危险的修法**（MUST NOT 采用）：`if (!item) continue;`——把「该态没有实例」与「该字段的引用不再被扫」
**混为一谈**，是 W66「删字段照样绿」的同族形状。

**采用的修法**：样本定位改为**按字段存在**（`Object.hasOwn(entry, field)`）——用例断言的是"**这个字段**
里的 §NNN 被扫"，样本就该按"谁有这个字段"选；按处置选会让**一次合法的数据变更**（清空某态）
**改变判据的覆盖面**。这与 W80／W83／W88 三次确立的「**触发条件 MUST 只由「字段存在」决定**」同源。

空态**被显式断言**（计数 0 ⇒ 数据里 MUST 确实没有该态条目），不是"找不到就跳过"。

## 五、非空洞注入（F1–F8，全部真跑）

方法：**改真实台账 → 跑真实测试（`node --test tests/registry.test.mjs`）→ 抓具名 → 还原并校验 sha256**。

| 编号 | 注入 | 结果 | 具名错误串 |
| --- | --- | --- | --- |
| F1 | PD-22 的 `landing` → 不存在路径 | **PASS** | `PD-22: landing crates/nai-security/src/no-such-file.rs is not a file tracked by this repository` |
| F2 | PD-34 的 `closure` 尾接 `§99.9` | **PASS** | `PD-34: closure references §99.9, which does not exist in the baseline` |
| F3 | PD-24 的 `disposition` → 枚举外的值 | **PASS** | `invalid disposition`（具名） |
| F4 | 删掉 PD-24 的 `landing` | **PASS** | `PD-24: disposition decided_in_repo requires a non-empty landing` |
| F5 | PD-22 的 `landing` → `node_modules`（工作区有、**索引无**） | **PASS** | 具名（证明判据读的是 `.git/index`，不是工作区） |
| F6 | **反向**：合法 `landing` 挂到一条 `needs_external` 上 | **PASS（不红）** | — |
| F7 | **反向**：合法 `closure` 挂到一条 `decided_in_repo` 上 | **PASS（不红）** | — |
| F8 | **真跑**：清空态后跑全量台账测试 | **PASS（50/0）** | — |

- 每次注入后 **sha256 逐字还原**（`ALL-OK`）。
- F1–F5 各自点红 **4 个用例**（主判据 + 3 个含"期望 clean"前提的用例）——这是**前提共享**的结果
  （它们共用同一份 `pendingLedger` 夹具，台账一旦非法，"期望 clean"的断言都会红），**不是判据重叠**。
- **一次自我纠错**：抓取 F2 具名串的**临时脚本**一开始用 `count=1` 全局首次匹配，命中的是 **PD-01** 的
  `closure`（第一个 `closure:` 出现处）——正是 `tests/registry.test.mjs:61` 那条 `replaceOnce` 注释
  警告的形状（"注入的价值全在「我改的确实是我以为的那一处」"）。**按块重做**后得到上表的 PD-34 具名串。
  正式 harness（`.w92-inject.py`）的 F2 本来就是按块处理的，只有临时抓取脚本写错。

## 六、验证

### 6.1 分面

| 面 | 命令 | 结果 |
| --- | --- | --- |
| 台账判据 | `node --test tests/registry.test.mjs` | **50 → 50，全绿**（替换而非新增） |
| 变更校验 | `openspec validate w92-decidable-in-repo-settlement --strict` | **valid** |
| 归档前全量 | `openspec validate --all --strict` | **44 passed / 0 failed**（含活动变更；归档后见 §6.3） |

### 6.2 全量门控

`cargo xtask check --profile full` = **EXIT=0**：

| 指标 | 值 | 与 W91 对比 |
| --- | --- | --- |
| `test result` 行数（目标数） | **91** | 一致 |
| passed 合计 | **862** | 一致 |
| failed 合计 | **0** | 一致 |
| 预链接（`cargo test --workspace --all-targets --locked --no-run`） | **第 1 轮即收敛**，`LNK1104` 命中 **0** | W91 是第 5 轮 |
| `cargo fmt --all --check`（独立复跑） | **EXIT=0** | 一致 |
| frontend 步骤 | 「全部步骤通过（工程基础，非渲染/人工 UI 验收）」 | 一致 |

**Node 侧 5 个套件全绿**：`tests/registry.test.mjs` **50/50**（**本轮改动**）、
`tests/contracts/schema.test.mjs` 30/30、其余 22/5/52（`tests/registry.test.mjs` 在门控内 ——
`xtask/src/registry.rs` 直接 `node --test tests/registry.test.mjs`）。

`cargo xtask` 与 `tests/registry.yaml` **零改动**、**Rust 零改动** ⇒ 与 W91 同形（**91 目标 / 862 passed**，
两侧数字不变属预期，不是"门控没跑到新判据"——Node 侧 registry 的 50/50 就是本轮判据在门控里跑过的证据）。

### 6.3 归档与两条 validate

归档：`openspec archive w92-decidable-in-repo-settlement --yes` ⇒
`agent-harness: ~ 1 modified`（Totals `+ 0, ~ 1, - 0, → 0`），归档为
`2026-10-13-w92-decidable-in-repo-settlement`；活规格已含新句（`R1 首版入站只响应…`／`MUST NOT 提供配置面`／
`A third party asks to be admitted`）。

| 命令 | 结果 |
| --- | --- |
| `openspec validate --all --strict`（**归档前**） | **44 passed / 0 failed**（含活动变更） |
| `openspec validate --all --strict`（**归档后**） | **43 passed / 0 failed**（与 W91 的 44 → 43 同形） |
| `openspec validate --all --strict --archived`（归档后） | **90 passed / 0 failed**（W91 的 89 → 90） |

## 七、CI 结论

**观测三件齐**：唯一查询参数（`&cb=<time_ns>`）破坏代理缓存 + 请求头 `Cache-Control: no-cache`
+ 回读 `head_sha` 与推送 SHA 比对。**单次**查询
`https://github.com/F31/nai/actions/runs/38399560922`：

| 项 | 值 |
| --- | --- |
| run | **38399560922（#115）** |
| 回读 `head_sha` | `349fc1a15c468c5fa7a296faa9c1bdbc9df81b33` —— 与推送 SHA **MATCH** |
| 结论 | **completed / success** |
| 时间 | `2026-10-13T12:44:44Z` → `12:57:09Z`（约 **12 分 25 秒**；同 push 事件只触发这一个 run） |

两个 job 均 **completed / success**，各 **26** 个可视步骤：

| job | 结论 | 跳过项（均为条件跳过，非缺失） |
| --- | --- | --- |
| `foundation (ubuntu-24.04)` | **success** | **step 11**「Publish the gate failure summary as annotations」（`if: failure()`） |
| `foundation (windows-latest)` | **success** | **step 8**（Linux C++ runtime）、**step 11**（同上）、**step 20**（Linux-only 三语言契约套件） |

**本机绿与 CI 绿同源**（逐条比对 `.github/workflows/foundation.yml` 与 §6.2 的本地门控）：CI 的
**step 10**「Complete engineering foundation (portable steps + frontend workspace)」跑的命令是
`cargo xtask check --profile full` —— **与 §6.2 本机门控是同一条命令、同一个 profile**，两平台各跑一遍
且都 `success` ⇒ 本轮的判据变更（`tests/pending-decisions.yaml` + `tests/registry.test.mjs`，经
`xtask/src/registry.rs` 的 `node --test` 进入 `engineering_checks`）**在 CI 侧也真跑过**。
其余各步亦 `success`：step 9 `cargo xtask schema --check`、step 12–18 七个 `cargo xtask test --suite *`、
step 19 `node --test tests/contracts/schema.test.mjs tests/contracts/descriptor.test.mjs`、
step 21 `pnpm spec:validate`、step 22 `openspec validate --archived --no-interactive`。

**观测粒度的自限**：本结论为**作业/步骤级**（判 run 结论所需的三件已齐），**未**导出各步骤日志内的
逐目标计数（`GET /actions/jobs/{id}/logs` 要求仓库 admin 权限，无凭据不可读，且 step 11 的 annotation 通道
在无失败时本就跳过）⇒ **不断言**「CI 侧也是 91 目标 / 862 passed」，只断言
「**同一条 `check --profile full` 在 CI 两平台均 `success`，且其 `head_sha` 等于本轮推送 SHA**」。

## 八、遗留

1. **基线收录**：PD-22／PD-24 的裁定若要进基线正文，走下次升版（§46.3 的"初值 1"与规格的"首版常量"分层）。
2. **A2A 实现**：H13／H14 属 `capability_pack`（INV-33），R1 不装；实现时 MUST 兑现规格的「具名拒绝」
   与「上限为常量」。**落地是规范层的**——MUST NOT 被读作"A2A 已可用"（`delegation_depth` 在 Rust 仍零命中）。
3. **PD-34 的外部闭合**：三个画像的真实使用者访谈记录仍缺（`needs_external`）。
4. **W90 §八遗留 1／3／4／5** 与 W91 的跨语言（TS／生成面）键级消费**未动**。
5. `decidable_in_repo` 计数为 0 ⇒ 该态的**数据侧注入暂时无实例**（由 `shapeErrors()` 与常量表断言在
   **表格层**继续看守，与数据无关）。
6. ADR 0066 遗留 1 与 ADR 0067 遗留 4 的「余下 N 条 `decidable_in_repo`」**在本轮结清**。

## 九、方法论教训

1. **「清空一个枚举的某一态」是一次**压力测试**：任何按"态存在"定位样本的用例，都会在一个态被合法
   清空时**失去作用对象**。正确反应是**把定位条件换成被断言对象的属性**（字段存在），而不是补一句
   `continue`。**危险的修法比缺陷本身更危险**——它会把"没检查"伪装成"已通过"。
2. **注入的价值全在「我改的确实是我以为的那一处」**：临时抓取脚本用 `count=1` 全局首次匹配，
   把注入打到了 PD-01 而**输出看起来依然正确**（同样是"closure 引用坏章节"）。⇒ 「红」不等于
   「红在我以为的地方」；抓取具名串时须**按对象定位**（按块），并回读确认。
3. **「一般的基线描述」与「某一版的具体化」按对象性质分层**：PD-24 会让人以为在改正本，但
   §46.3 说的是协议的**一般规则**（"初值"）、规格说的是 **R1 首版**的取值 ⇒ 并存不矛盾。
   **判据是对象，不是章号**（W56 §56 的既有教训在此复现）。
4. **「登记与事实不符」是可以被核实抓到的**：PD-34 登记 `decidable_in_repo`，而它的闭合条件写明在
   §57.5 里是"真实使用者验证"——**同一条目的推荐文本自己就否证了它的处置**。⇒ 核实一条待裁定项时，
   MUST 读它**自己的** recommendation 与基线原文，不能只看处置字段。
