# W74 报告：消费守卫的边界（扫描集、语言闭包、来源自负其责与单一生成清单）

- 日期：2026-10-12（W74）
- 相关：`docs/adr/0052-consumption-guard-boundaries.md`、ADR 0051（建立本守卫，其遗留 2 提出本轮的修正方向）、
  ADR 0050 决策 2
- 变更：`openspec/changes/2026-10-12-w74-consumption-guard-integrity/`（归档后移至 `archive/`）

## 一、判断与范围

出发点写的是 **W73 遗留第 2 条**（`generate.mjs` 用模板路径消费 10 份 schema；它的清单是 `AJV_SOURCES`
的子集；建议「若将来出现这种形态，把该清单也做成一条派生来源」）。**核实之后改了计划**：

- 那条遗留把风险写成「仅由模板消费的 schema 会被**误报为孤儿**」（安全的假阳性方向），
  而**真正的风险**是**两份清单可以无声漂移** ⇒ 这是 F4，性质与遗留里写的不同；
- 更靠前的问题在**守卫本身**：它有三条声称，**其中两条不成立**（F1、F2），另有一条边界由两个缺陷
  互相掩盖（F3）；
- 顺带订正一条**贯穿六轮报告**的归因（F5）。

**范围**：只动「消费守卫本身 + 它的两个输入面（扫描集与生成清单）」。
**不改基线正文、不改任何 schema 文件、不改任何 `acceptance` 条目、不改 Rust 行为、不新增 xtask 步骤。**

## 二、核实（五件事，全部先读后判；探针落在 `dist/`，用完删除）

### 2.1 F1：非空洞断言自我满足

W73 守卫的非空洞断言是：

```js
assert.ok(consumed.has('schemas/platform-matrix/platforms.schema.json') &&
  consumed.has('schemas/tests/registry.schema.json'),
  'the literal scan and the validator list must both contribute');
```

这两份 schema **都在 `AJV_SOURCES` 里** ⇒ 这句话**只由 Ajv 清单独自满足**。实测（探针）：

```
[A] 全开            = 13 份消费
[B] 关掉字面量扫描   = 13 份消费
[C] 关掉 Rust 扫描   = 12 份消费
只有字面量扫描能找到的 schema: （无）
只有 Rust 扫描能找到的 schema: schemas/config/upgrade-package.schema.json
```

**注入 V1**（把 `for (const pattern of [rust, literal])` 改成 `[rust]`）：守卫**照旧 13/13 / `fail 0`**。
⇒ 整条来源可以被删除，而**没有任何断言变红**。

### 2.2 F2：扫描的是工作区，不是仓库（最严重）

路径字面量来源扫「工作区里所有匹配扩展名的文件」。**注入 V2**：

```
（a）加未跟踪的 schemas/config/zzz-probe.schema.json
      ⇒ # fail 1（真孤儿被抓）✓
（b）再加未跟踪的 .tmp-fake-consumer.mjs（内含一句 schema 路径字面量）
      ⇒ # tests 13 / # pass 13 / # fail 0   ← 真孤儿被洗白
```

「有一份 schema 没人读」是本守卫**唯一**要抓的东西，而一句临时脚本就能让它失效。
这不只是「漏一次」，而是**判据的输入不等于仓库**。

### 2.3 F3：两个缺陷互相掩盖

```js
const CODE_EXTENSIONS = ['.rs', '.mjs', '.ts', '.tsx', '.yml'];            // 缺 .yaml / .js
const text = stripCommentLines(await readFile(file, 'utf8'), file.endsWith('.yml'));  // 只对 .yml 用 #
```

探针普查（全仓提到过 `.schema.json` 的 95 份文件）：`.md 79 / .rs 7 / .mjs 6 / .json 2 / **.yaml 1**`，
那唯一一份 `.yaml` 就是 **`tests/registry.yaml`** —— 也正是 W73 写下「注释里的路径不算消费」那句注释的地方。
它**因为扩展名叫 `.yaml` 而根本不在扫描集内** ⇒「不扫描的格式」与「误判注释语法的格式」各错一半，
且各自躲在对方的盲区里。

另：dot 目录（`.opencode` 9 / `.workbuddy` 7 / `.cargo` 1 / `.github` 1）都在扫描路径上，
其中只有 `.github/workflows/foundation.yml` 会被扫到。

### 2.4 F4：`schemas/` 有第二处清单

| 位置 | 条数 | 内容 |
|---|---|---|
| `scripts/contracts/validation.mjs` 的 `AJV_SOURCES` | 12 | 校验面 |
| `scripts/contracts/generate.mjs` 的 `jsonSources` | 10 | 生成面（前者的子集） |

两份清单**无任何交叉断言**；生成物 `packages/contracts-ts/src/generated/config/*.ts`（恰 10 份）**已入库**
⇒「生成面 ⊆ 校验面」这条**可以判定却无人判**。

### 2.5 F5：宿主禁止 node 派生**任何**子进程（并订正既有归因）

F2 的自然解法是「取仓库文件集」。实测**不可行**：本机 node 对 `git`（裸名与绝对路径）、
`process.execPath`、`cmd.exe`、`sh` **一律报 `EBUSY`**。

> **订正**：此前把 `cargo xtask schema --check` 本机跑不了的 `EBUSY` 归因为「主机对 **node 自旋 node** 的限制」。
> 正确表述是「**宿主禁止 node 派生任何子进程**」，比原归因**更宽**（原归因会把「node 生成 git」这类情形判为可用）。
> 处置与结论不变。**处置**：`AGENTS.md` 就地订正（1:1 行替换）；五份历史报告
> （`2026-10-11-w65`、`2026-10-12-w67`/`w68`/`w69`/`w71`）**保留原文 + 追加勘误段**。

⇒ F2 只能靠**结构性收敛**处理，残余风险如实登记（见 §八）。

### 2.6 扫描集事实（决策的实测底座）

```
跳 dot 段 + 具名排除  = 281 个代码文件（.rs 188 / .yaml 45 / .ts 27 / .mjs 20 / .tsx 1）
不跳 dot 段（仍排除 .git 与四个具名目录） = 282
唯一差异：.github/workflows/foundation.yml（.yml）
两侧的消费集合 delta：rust [] / literal []
```

关键词：**跳 dot 段与不跳 dot 段的消费集合完全相同** ⇒ 「排除无害」可以被**断言**，而不是被假设。

## 三、裁定（ADR 0052 的四条决策）

1. **扫描集是结构性的**：从仓库根递归、**跳过所有以 `.` 开头的目录**，再叠加**具名排除**
   （恰 4 条：`node_modules`/`target`/`graphify-out`/`dist`），**每条 MUST 被 `.gitignore` 忽略**——
   「它不是仓库的一部分」就是那条理由，`.gitignore` 就是证据。
   W73 的 `SKIP_DIRS` 里 `build`/`.venv` 两条**既不在 `.gitignore` 也不在磁盘上**（抄来的），本条移除。
2. **语言闭包**：`CODE_LANGUAGES`（扩展名 → 注释语言）**派生** `CODE_EXTENSIONS`
   ⇒「新增可扫描扩展名」与「声明它的注释语言」是**同一件事**；`.yml` 与 `.yaml` MUST 一起覆盖。
3. **来源自负其责**：来源收为**两条**，判据从「各自都被行使」换成**独有覆盖**：

   | 来源 | 独有找出的 schema | 条数 |
   |---|---|---|
   | Ajv 清单 | `config/binding-plan`、`catalog/catalog`、`plugin/ui`、`platform-matrix/platforms`、`tests/registry` | 5 |
   | Rust `include_str!` | `config/upgrade-package` | 1 |

   **路径字面量来源被移除**（零独有覆盖 + F2 的全部入口）。理由：按「假阳性与假阴性不对称，
   永远选安全方向」——保留它换假阴性（危险），移除它换假阳性（安全，加进 `AJV_SOURCES` 即可）。
4. **生成面与校验面共用一份清单**：`validation.mjs` 导出 `TS_GENERATED_SOURCES`，`generate.mjs`
   **删除硬编码**改为导入；断言 **生成面 ⊆ 校验面 + basename 互异 + 生成目录与清单双向相等**
   ＋防回退（生成器源码 MUST 引用该清单）。这**闭合了 ADR 0051 遗留 2** 自己提出的方向。

## 四、实现

| 文件 | 改动 |
|---|---|
| `scripts/contracts/validation.mjs` | 新增导出 `TS_GENERATED_SOURCES`（10 条，注明 MUST ⊆ `AJV_SOURCES`） |
| `scripts/contracts/generate.mjs` | 删除 `jsonSources` 字面量，改为 `import { TS_GENERATED_SOURCES }` |
| `tests/contracts/schema.test.mjs` | `SKIP_DIRS` → 结构性规则 + `EXCLUDED_DIRS`；`CODE_LANGUAGES` 派生 `CODE_EXTENSIONS`；`stripCommentLines(text, language)`；`consumedSchemas()` → `consumptionOf(files)`（纯函数，两条来源）；新增 `exclusiveTo` / `sourceErrors` / `exclusionErrors` / `generatorErrors`；新增两组测试 |

一个取舍要写明：**`consumptionOf` 改成接「文件列表」的纯函数**，是为了让「扫描集相等」这条断言
**可构造**（否则无法给两侧喂不同的输入去证明它敏感）。

## 五、非空洞注入（每组都必须被抓住，且逐项还原）

| # | 注入 | 预期 | 实测 |
|---|---|---|---|
| 3.1a | 清空 `AJV_SOURCES` | Ajv 零独有被指名 + 5 份独有变孤儿 | **被抓住**（恰 5 份：`catalog`/`binding-plan`/`platforms`/`plugin/ui`/`tests/registry`） |
| 3.1b | `RUST_INCLUDE` 改成永不匹配 | Rust 零独有 + `upgrade-package` 变孤儿 | **被抓住**（恰 `upgrade-package`） |
| 3.2 | 在一个 dot 目录里放含 `include_str!` 的 `.rs` | 「跳与不跳集合相同」变红 | **被抓住**（`deepStrictEqual` 差异直指那份 schema） |
| 3.3 | 给 `EXCLUDED_DIRS` 加 `'vendor'`（不在 `.gitignore`） | 排除未付理由 | **被抓住**（`vendor is excluded … .gitignore does not ignore it`） |
| 3.4 | 把 `.yaml` 换成不存在的扩展名 | 两处断言（`tests/registry.yaml` 收录 + `.yml`/`.yaml` 同语言） | **被抓住**（`not ok 13` 与 `not ok 14` 同时红） |
| 3.5 | `TS_GENERATED_SOURCES` 加一条 | 「生成但未校验」+「清单有而目录无」 | **被抓住**（两条同时开火） |

**外加一项：重现 W73 被洗绿的攻击**

```
真孤儿（未跟踪 schemas/config/w74-orphan.schema.json）
  + 未跟踪的伪造消费者 .tmp-fake-consumer.mjs
⇒ # fail 1：'…w74-orphan.schema.json is consumed by no execution path …'
```

⇒ W73 时**同一场景 13/13 全绿**，现在**仍被抓**。这是本轮**最关键的证据**。

**还原核验**：六组注入每次都以 `diff -q` 确认文件回到备份；`git status` 最终为
「3 个已改文件 + 1 个新变更目录」，无残留探针。

## 六、验证

- **消费守卫单测**：`node --test tests/contracts/schema.test.mjs` ⇒ **15 / 15 通过**（13 → 15，两条新增）。
- **门控 full**：`cargo xtask check --profile full` ⇒ **EXIT=0**；测试目标 **91（73 `Running` + 18 `Doc-tests`）**、
  **858 passed / 0 failed**（与 W73 逐项相同——本轮零 Rust 测试改动，两个增量对得上即「未误改别处」的旁证）。
  含前端门控与两条 Node 契约脚本。
- **`cargo xtask schema --check`**：本机**仍跑不了**（F5 的宿主约束）。本轮**未改动任何生成器输入**
  （`schemas/catalog/capabilities.json`、`schemas/{config,plugin}/*.schema.json` 均未动，生成物零漂移），
  零漂移由**结构性理由**取证，并由 **CI 的独立步骤**兜底。**不把「跑不了」写成「已通过」。**
- **两条 `openspec validate`**：`--all --strict` = **43**；`--all --strict --archived` = **72**（归档后 +1，W73 为 43 / 71）。

## 七、CI 复核（匿名观测）

| run | head_sha | 结论 |
|---|---|---|
| [`38233465696`](https://github.com/F31/nai/actions/runs/38233465696) | `7ad11c9`（与本轮推送 SHA 一致） | **双平台双绿**：`foundation (ubuntu-24.04)` **success**（**5m33s**）、`foundation (windows-latest)` **success**（**10m29s**）；run 总状态 **Success**，09:59:41Z → 10:10:15Z（**10m34s**）；两个 job 各 26 步 |

**顺带确认**：CI 的 job 步骤含 `pnpm exec openspec validate --archived --no-interactive`
（`.github/workflows/foundation.yml` 第 84 行）⇒ **已归档口径在 CI 上也跑**——W74 的 `tasks.md`
若留着那 6 个未勾项，CI 会与本机一样以 `N incomplete tasks` 失败；这条修正对 CI 是必需的，不是本机才需要。

## 八、未做与遗留（如实登记）

1. **未跟踪文件若落进「非 dot、非具名排除」的目录仍可能造出消费者**（例如 `scripts/` 下的一份手放脚本）。
   缓解：**CI 从干净 checkout 跑** ⇒ **CI 上「工作区 == 仓库」成立，该风险不存在**；它只存在于本机有人
   手工放置文件时。彻底消除需要「仓库文件集」这个输入，而取它需要派生 `git` 子进程（宿主禁止）。
2. **`.github` 被结构性排除**是一次**有意**取舍（代价：工作流里的 schema 引用不再算消费证据；
   实测损失为零）。若将来工作流成为某 schema 的唯一消费者，正确做法是**为它加一条具名排除的
   反向声明**（把它变成「正在被考虑」），而**不是**恢复枚举式扫描。
3. **判定覆盖从三条来源缩到两条**：将来若有 schema 只被某个脚本加载，会被报成**假阳性孤儿**
   （安全方向），处置是加进 `AJV_SOURCES`。
4. **语义类漂移仍只能靠人读**（沿用 ADR 0050 遗留 1 / ADR 0051 遗留 1）。
5. **`§56.2` 自身两处张力、`§57.5`/`§48.7` 待决策项**：沿用 W71/W72/W73 的登记，未动。
6. **`rag-golden/case-set` 的例外理由**仍需人维护（它的执行体 `golden.rs::parse_case_set` 未变）。

## 九、方法论教训

1. **守卫的「非空洞」断言必须逐条追问：删掉它，会不会有东西变红？**
   W73 那句「三条来源各自都被行使」写得很像在防空洞，实际只由**一条**来源满足。
   ⇒ **「哪条来源能单独满足这句话」是比「它有没有断言」更靠前的问题。**
2. **判据的输入必须等于被断言的对象。** 守卫要断言「仓库里没有孤儿」，而它读的是**工作区**——
   输入与对象之间的缺口就是 F2。**在没有版本控制查询能力时，这个缺口只能靠「结构性收敛」缩小，不能装作不存在。**
3. **两个缺陷可以互为对方的盲区。** `.yaml` 不在扫描集、`.yml` 才判 `#` 注释——各自看都像「漏了一点」，
   合起来却让**唯一能暴露它的文件**恰好落在两者之外。⇒ **修一个边界问题时，要问「另一个边界是否正好
   挡住了它的证据」。**
4. **「两份清单」不可能都不出错，只能互相牵制。** 交叉断言（生成面 ⊆ 校验面）比各自内部的完整性断言
   更有力——**各自的完整性检查对「另一份改了」是瞎的**。
5. **移走一条来源之前，先量它的独有覆盖。** 「保留它更安全」是直觉；**量出来**才发现它零独有覆盖，
   而且它是危险方向（假阴性）的唯一入口。**安全方向是量出来的，不是感觉出来的。**
6. **订正一个归因要全仓扫描。** 「node 自旋 node」这句话在**六轮报告**里被复制过——若不扫全仓就改，
   读者会在五份历史报告里继续读到已被订正的表述。**活文档就地订正、历史记录追加勘误段**。

## 勘误（W75，2026-10-12）

按 §56.2「历史记录保留原文 + 追加勘误段」，本报告**原文不改**，两处被订正：

**① §八遗留第 1 条的末句**（原文：「彻底消除需要『仓库文件集』这个输入，而取它需要派生 `git` 子进程
（宿主禁止）」）。**订正**：该输入**可以**取得，**不需要**任何子进程——`.git/index` 是 Git 的 DIRC
格式，**纯 JS 可读**（W75 实测 `magic=DIRC version=2 entries=922`、完整解析 922/922 条路径）。
⇒ **F2 已在 W75 根除**：判据改为「消费证据的来源 MUST ⊆ 已跟踪路径集」。该条遗留里
「CI 从干净 checkout 跑 ⇒ 该风险不存在」的说明**仍然正确**，但 W75 之后判据在 CI 与本机**同时**成立。

**② §九方法论教训第 2 条的末句**（原文：「**在没有版本控制查询能力时，这个缺口只能靠『结构性收敛』
缩小，不能装作不存在**」）。**订正**：前半句的前提**不成立**——查询能力**是有的**（读一个文件，
不是派生子进程）。**教训本身（判据的输入必须等于被断言的对象）依旧正确且更重要**；
要改的只是「没有能力」这个判断。⇒ **凡把某个缺口写成「只能如此」的地方，都要先问一次
「这个『只能』是实测的，还是我以为的」。**

**③ 补充（不是订正，是 W75 才测出来的事实）**：本报告 §五的重现只证明了「W73 的攻击形态」失效。
W74 之后 F2 的**残余**形态（用 `include_str!` 而非路径字面量）实际上被 `exclusiveTo(RUST)` 的
**具名钉死**挡住——那是**副作用而非设计**：任何人**合法地**让一份新 schema 只被 Rust 消费，都会让
那条钉死变红，而顺手的处置是**把它改松**（W75 的 V1c 实测了这一点，放宽后 `repositoryErrors`
独立兜住；V1b 两条都停用则 17/17 全绿）。见 [ADR 0053](../adr/0053-repository-scoped-evidence.md)。

