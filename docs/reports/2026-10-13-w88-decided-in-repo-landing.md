# W88：待裁定台账的第六态与落地点（PD-09 结清）

- 轮次：**W88**（V3.11 计划续编，不重排 W 编号）
- 变更：`openspec/changes/archive/2026-10-13-w88-decided-in-repo-landing/`
- 裁定：[ADR 0066](../adr/0066-decided-in-repo-landing.md)；ADR 0054 **追加勘误段**
- 上游登记：[W87 报告 §八第 1 条](2026-10-13-w87-baseline-v311-archive-boundary.md)、
  [ADR 0054](../adr/0054-pending-decisions-ledger.md) 遗留 3（**同一处**，W76 登记，W87 复述）

## 一、判断：缺口不在数据，在**分类**

W87 §八第 1 条与 ADR 0054 遗留 3 登记的是**同一件事**：

> **5 条 `decidable_in_repo` 的推荐尚未执行**：它们是推荐而非决定；采纳需要单独一轮
> （其中 **PD-09 会触碰平台密钥后端**、PD-22/PD-24 属 A2A 边界、PD-31 属 R3 前置、PD-34 需真实访谈）。

本轮采纳 **PD-09**（*静态数据加密所用的系统密钥保护机制*）。核实的第一个结论是**推荐早已落地**；
第二个结论更根本：**台账没有可以结清它的状态**。五态回答的都是「**何处能闭合**」——
基线答了（`answered_by_baseline`）／可在仓库内决定（`decidable_in_repo`）／等前置／等外部／不在范围。
**没有一态**表示「**仓库已经作出决定、并且已经落地**」⇒ 一个已结清的条目只能永远停在
「**可**决定」上，台账读起来会说「还等人决定」。

⇒ 这不是缺数据，是缺**一整个维度**：五态是「由谁闭合」，缺的是「**闭合了没有、落在哪里**」。
把「已落地」写成「可决定」的注解，是 W87 §九那条「一个载体承载两件事」的**同族**缺陷。

## 二、核实：先读后判

### 2.1 PD-09 的推荐，四条子句逐条比对

| 子句 | 仓库事实 | 落地点 |
| --- | --- | --- |
| 按目标平台各用**系统密钥库** | `NativeVault`（`keyring`） | `crates/nai-security/src/lib.rs` |
| **Windows 原生凭据**为首发实现 | `keyring` **只在** `[target.'cfg(windows)'.dependencies]` 下 | `crates/nai-security/Cargo.toml` |
| 缺该后端的平台如实登记 `explicit-unavailable` | `secret-protection` 行：`windows` = supported；`linux`/`macos` = `explicit-unavailable`（`not_ported_to_target_os`）；`ios`/`android` = `out-of-scope` | `schemas/platform-matrix/platforms.json` |
| MUST NOT 自造一套跨平台密钥库 | 不存在这样的东西 | —（能力行 note：「非 Windows 的 store/load/delete 一律**显式**返回 CapabilityNotReady——是失败关闭，不是静默降级」） |

行为有测试钉住：`production_backend_is_explicitly_unavailable`（`#[cfg(not(windows))]`）与
`native_windows_roundtrip_and_cleanup`（`#[cfg(windows)]`，由 CI 的 windows job 执行）。
⇒ **推荐已被采纳并落地**，不是「计划落地」。

### 2.2 一处**必须排除**的省事路径

基线 §11.5 的原文是：

> 静态数据：… 备份与录制提供可选加密，密钥由系统密钥保护机制封装（**具体机制待选型验证**）。

⇒ **基线没有回答它**，是**仓库**回答了它。故 **MUST NOT** 把它记作 `answered_by_baseline`——
该处置断言「基线正文回答了它」，写成那样就是一条**不实声明**。这条排除是**判据级**的（不是措辞级的）。

### 2.3 落地点**不能**用作业项编号

PD-09 的落地轮次是 **W55**（平台能力矩阵）。实测基线的工作项表**不含 W41–W55**：
§47.6 的编号说明写着「V3.7 §39.6 的计划止于 **W40**；**W41–W44、W46–W55 已被后续实际执行轮次占用**…
因此第四部分从 **W56** 起编号」。⇒ `W55` 无法作为 `W\d{2}` 解析，若把落地点写成作业项，
这条**必填字段就会恒假**。⇒ 改用**仓库路径**（成员集由 `.git/index` 派生，`.git/index` 是唯一判据输入）。

### 2.4 原实现里一处**写死的字段名**

必填字段的「清单型 / 文本型」判定原本写作：

```js
const ok = field === 'precondition' ? Array.isArray(value) && value.length > 0 : nonEmptyText(value);
```

新增的 `landing` 会**落进文本分支**。本轮实测（真跑这一行，见 §五 F4/F5）。

## 三、裁定（[ADR 0066](../adr/0066-decided-in-repo-landing.md)）

1. **新增第六态 `decided_in_repo`**：语义是「决定已在仓库内作出**且已落地**」。
   **MUST NOT** 写回 `decidable_in_repo`（会让台账说「还等人决定」）。
2. **它的必填字段是 `landing`（落地点），不是决定文本**——与 `answered_by_baseline` 用 `basis`
   （答在哪一节）而不是答案原文是**同一条纪律**。六个处置仍然**各一个**必填字段。
3. **`landing` 的每个元素 MUST 是仓库已跟踪的路径**，判据输入由 `.git/index` 派生（`git-index.mjs`，W75）。
4. **新增第三类检查类别 `REPO_PATH_FIELDS`**，与 `ID_LIST_FIELDS` **不合并**（解析器不同）。
5. **「清单型 / 文本型」由类别表决定**，MUST NOT 写死字段名。
6. **路径校验按「字段是否存在」触发**，MUST NOT 按处置触发（W80／W83 的纪律）。
7. **不引入 `tests/pending-decisions.schema.json`**——ADR 0054 遗留 5 预留的判定，本轮给出结论。

## 四、实现

- `scripts/contracts/pending-decisions.mjs`：
  - `DISPOSITIONS` 六态；`REQUIRED_FIELD` 加 `['decided_in_repo', 'landing']`；
  - 新增 `REPO_PATH_FIELDS = ['landing']`；`REFERENCE_FIELDS`（八 → **九**）、
    `LEDGER_FIELDS`（十 → **十一**）逐字加入 `landing`；
  - `shapeErrors()`：类别**三 → 四**，「必填字段被某检查覆盖」「并集恰等于 `LEDGER_FIELDS`」
    「引用域 == 类别之并」三条断言随之扩展；
  - `validatePendingDecisions`：新增必填依赖 **`repositoryFiles`**；引用扫描按类别**三路分派**
    （`§NNN` / 登记 id / **仓库成员路径**）；清单型判定改为 `idListFields ∪ repoPathFields`；
  - `DISPOSITION_COUNTS`：`decidable_in_repo` **5 → 4**、`decided_in_repo` **0 → 1**。
- `tests/pending-decisions.yaml`：`PD-09` 迁态 + `landing` 三处；`recommendation` **原文保留**（注解）。
- `tests/registry.test.mjs`：注入 `repositoryFiles`（并断言跟踪集非空）、四条既有断言纳入第四类、
  新增三条判据（**47 → 50**）。
- `docs/adr/0066-decided-in-repo-landing.md`（新建）；`docs/adr/0054-…md` **追加勘误段**；
  `AGENTS.md`、`docs/development-plan.md` 追加 W88 小节。

**不改**：基线正文、`tests/registry.yaml`、任何 Rust、依赖边、契约名；余下 4 条 `decidable_in_repo`
的处置一个字未动。

### 4.1 台账 diff 的净效果

`PD-09` 是**唯一**被改的条目；总数仍 **34**、分章计数（§18.2=10 / §39.7=11 / §48.7=10 / §57.5=3）不变，
只有 `DISPOSITION_COUNTS` 移动一格。⇒ 「总数对『被改类别』是瞎的」（W66）这条教训，
正是 `DISPOSITION_COUNTS` 存在的理由，而它在本轮**逮住了**这次迁移是否真的只动了一格。

## 五、非空洞注入（F0–F7）

| # | 注入 | 期望 | 实测 |
| --- | --- | --- | --- |
| F0 | 干净台账 | 零错误 | **零错误** ✓ |
| F1 | `landing` 加一个不存在的路径 | 具名（条目 + 字段 + 值） | `PD-09: landing crates/nai-security/src/nope.rs is not a file tracked by this repository` ✓ |
| F2 | `landing` 给 `node_modules`（**磁盘上存在、索引里没有**） | 具名 | `PD-09: landing node_modules is not a file tracked by this repository` ✓ |
| F3 | 把 `landing` 移出 `REFERENCE_FIELDS`，再跑 F1 | 同一个坏值**静默** | **(silent)** ✓ ⇒ 这条检查**承重**，不是被别的规则顺带挡住的 |
| F4 | 暂恢复改前那一行 `field === 'precondition'`，`landing` 给一个**合法跟踪路径的字符串** | 改前**放行** | **0 error** ✓ |
| F5 | 恢复类别表实现，同一个注入 | 具名 | `PD-09: disposition decided_in_repo requires a non-empty landing` ✓ |
| F6 | 可解析的 `landing` 挂在 `needs_external` 条目上 | **不红** | **不红** ✓（拒绝的是**不可解析**的引用，不是**放错位置**的引用） |
| F7 | 跟踪集口径 | 非空 | **1014** 条路径 ✓ |

**F2 的意义**：它把「用 `.git/index`」与「用 `readdir`」**分开**——后者会把未跟踪文件算成仓库成员，
那正是 W75 的 **F2 根因**。这条注入是它的**直接**回归守卫。
**F4/F5 的意义**：写死的字段名在**新增一个清单字段的那一刻**就说谎（W83 同族：清单的补集是盲区）。

## 六、验证

### 6.1 定向测试

```
node --test tests/registry.test.mjs                          →  # tests 50 / # pass 50 / # fail 0
node --test tests/*.test.mjs tests/contracts/*.test.mjs      →  # tests 81 / # pass 81 / # fail 0
openspec validate w88-decided-in-repo-landing --strict        →  valid
```

`tests/registry.test.mjs` **47 → 50**（新增：落地路径存在性 / 路径解析不按处置触发 / 清单型判定由类别表决定）；
全仓 Node 用例 **78 → 81**。变更内三条 Requirement 的正文长度 **493 / 406 / 245 字符**（均 < 500）；
`proposal.md` 的 `## Why` 段 **918 字符**（< 1000，无超长警告）。

### 6.2 门控

```
cargo xtask check --profile full   →  EXIT=0
```

实测 **91 个测试目标（73 `Running` + 18 `Doc-tests`）、860 passed / 0 failed**，
日志内 `error[` / `LNK1104` / `out of memory` 命中 **0** 次。
口径以 `grep -c '^test result:' <门控日志>` 定准（**每个目标恰好一行，没有自由度**）；
`^     Running ` **74** 行 = **1 包装行**（cargo 自己打印的
`Running \`E:/nai-build-target\debug\nai-xtask.exe check --profile full\``，**不是测试目标**）+ 73 真实目标。
本轮**零 Rust 改动**（改的只有 `.mjs`／`.yaml`／`.md`）⇒ `860 passed` 与 W86／W87 相同，
「数字不变」正是「未误改别处」的旁证。

门控内的 Node 用例（**不**计入 91/860，那组数只来自 `cargo test`）：
`tests/registry.test.mjs` **50/50**、`tests/contracts/schema.test.mjs` **29/29**、
平台矩阵 **22/22**、桌面 **5/5**、`tests/ui` **52/52**。新用例确实在门控内执行：

```
ok 15 - the pending-decision ledger classifies every item and every reference resolves
ok 25 - a landing must name a path the repository actually tracks
ok 26 - a repository-path reference must resolve wherever its field appears, not only under its own disposition
ok 27 - a landing must be a list, and that list must be the required field of exactly one disposition
```

### 6.3 两条 validate

| 命令 | 结果 |
| --- | --- |
| `openspec validate w88-decided-in-repo-landing --strict`（归档前） | **valid** |
| `openspec validate --all --strict` | **43 passed / 0 failed**（43 项 = **spec 目录数**，本轮只改既有 spec，故与 W87 同为 43） |
| `openspec validate --all --strict --archived` | **86 passed / 0 failed**（**只含已归档变更、不含 specs**，每归档一次 +1；W87 为 **85**，本轮的 +1 即 w88） |

归档应用结果：`spec-driven-delivery` **+1 added / ~2 modified / -0 / →0**；
live spec 的 Requirement 数 **11 → 12**，且 `validate spec-driven-delivery --type spec` = **valid**。
（W87 的教训在此被提前适用：**归档前**先跑 `validate <change> --strict`，
不让「MODIFIED 块漏抄既有 Scenario」这类问题在归档后才暴露。）

## 七、CI 复核

**CI_PLACEHOLDER**

## 八、遗留

1. **余下 4 条 `decidable_in_repo` 的推荐仍只登记**：`PD-22`／`PD-24`（A2A 边界）、`PD-31`（R3 前置）、
   `PD-34`（需真实访谈）——每条仍值得单独一轮（与 W76 §八第 3 条、W87 §八第 1 条一致）。
2. **本轮核实中的一处新观察（未裁定，登记为下一轮的输入）**：`PD-31` 的推荐写着「R1 内判为
   『**空集** + 默认关闭』」，而 W57 已交付的 `crates/nai-kernel/src/evaluate/memory.rs` 里
   `MemoryPolicy::default()` 是 `viewer_memory_enabled: false`（**默认关闭**已落地）但
   `allowed_viewer_fields` **非空**（`["preference", "answered_topic"]`）⇒ 推荐里的「空集」
   与既有实现**存在张力**。**本轮未裁定**（不是 PD-09 的范围），下一轮核实后裁定。
3. **「已落地」的语义判定仍有半边靠人**：脚本证「这些路径**真实存在于仓库**」，
   **不**证「它们真的就是这个决定的落地」——与 ADR 0054 遗留 2 是同一条刻意留给人读的边界。
   `landing` 只写**三处**落地点，**不穷举**（相关测试与 CI 步骤未列）。
4. **H18 / B07 仍 `pending`**（判定面已交付，接线与真实证据是 runtime／外部工作）；
   `CAPABILITY_PACK_IDS` 仍手写（ADR 0056 遗留 4）；`cargo xtask schema --check` 本机仍 `EBUSY`，
   该步**以 CI 结论为准**，**未写成「本机已通过」**。
5. §57.5 的待决策项与 §48.7 的全部事项继续有效；§18.2／§39.7 亦未变。

## 九、方法论教训

- **「盘点类任务」的第二次产出常常是「缺一个维度，不是缺一条数据」。**
  本轮找到推荐已落地，正确的下一步**不是**改一条状态串，而是承认**五态表达不了「已闭合」**。
  ⇒ 当「这条该记成什么」无法回答时，先问「**是不是分类本身少了一格**」。
- **「更省事的那条路」需要被具名排除，且排除理由必须是判据级的。**
  `answered_by_baseline` 会让这个条目**看起来**结清，但基线 §11.5 明写「待选型验证」——
  ⇒ 排除的理由不是「读起来别扭」，而是「**那条断言是假的**」。**能被判据否掉的方案，才叫被排除**。
- **必填字段要选「可判的那一半」。** 落地点选**路径**而不选作业项编号，理由是实测的：
  基线工作项表不含 W41–W55 ⇒ 作业项那一半**恒假**。⇒ **必填字段的取值域 MUST 由实测决定，
  MUST NOT 由「看起来更整齐」决定。**
- **写死的字段名是「清单的补集」的另一种形状。** `field === 'precondition'` 与 W81/W83 的
  手写清单同族：新成员一进来就落在分支外面。⇒ 分派 MUST 由**类别表**决定，MUST NOT 由字面量决定；
  并且**改前/改后都要真跑**（0 error → 1 error 的差，才是这条教训的证据）。
- **「登记 ≠ 收录」在方向上**也成立。** 前几轮修的是「登录的字段没被扫」；
  本轮修的是「**扫的类别少了一类**」——同一个「清单的盲区是它的补集」在**类别**这一层复现。
  ⇒ 每新增一个类别，`shapeErrors()` 的「并集相等 / 引用域双向相等」两条断言 MUST 立刻点名它。
- **预留的判决要在触发条件出现时**当场**判掉，MUST NOT 再往后推。**
  ADR 0054 遗留 5 说「是否需要 JSON Schema，等第二次结构变更再判」；本轮就是那次变更 ⇒
  结论**必须**给出（本轮给「不引入」，理由是第二处定义），否则这条遗留会像 W78 的边界那样
  **连轮复述**（W78–W86 八轮的教训）。
- **一条判据的输入决定它能断言什么。** `landing` 若用工作区清单，它能断言的只是「文件在磁盘上」；
  用 `.git/index`，它才能断言「**这是仓库成员**」。⇒ 选判据输入时，先问「我要断言的是哪一句话」。
